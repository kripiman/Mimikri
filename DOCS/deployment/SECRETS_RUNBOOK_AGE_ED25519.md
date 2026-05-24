# Secrets Management Runbook — age + dedicated ed25519 (no YubiKey)

**Status**: Operational runbook
**Author**: Auditor (Sprint 7.5 inter-sprint stabilization)
**Date**: 2026-05-21
**Substitutes**: YubiKey + PGP smartcard custody (unavailable due to budget)
**Threat model coverage**: At-rest secret protection, key-bound decryption, offline backup recovery. Does NOT protect against compromised root on running box (RAM extraction possible while service runs).
**Related**: `HYBRID_DEPLOYMENT_TOPOLOGY.md` §7 (cost model), `stealth_opsec.md`, `utils/config.rs`

---

## 1. Purpose

Provide a deterministic, zero-cost procedure for custody, rotation, and recovery of Mimikri RedTeam Core secrets (`DO_TOKEN`, `H1_API_KEY`, `NVD_API_KEY`, `C2_TOKEN`, `POSTGRES_PASSWORD`, `OTEL_ENDPOINT` credentials, Tailscale auth-keys) using a dedicated ed25519 keypair with `age` encryption.

This runbook replaces the YubiKey/PGP smartcard pattern referenced in earlier deployment notes (memory: `mimikri-hybrid-deployment.md`) for environments where hardware tokens are unavailable.

## 2. Prerequisites

### Software
- `age` ≥ 1.1.0 (`apt install age` on Ubuntu/Debian, `pacman -S age` on Arch, or download from https://github.com/FiloSottile/age/releases)
- `openssh-client` (ssh-keygen)
- `paperkey` (optional, for paper backup): `apt install paperkey`
- `ssss` (optional, Shamir secret split): `apt install ssss`
- `gocryptfs` or `cryptsetup` (optional, for encrypted USB backup)

### Verify install
```bash
age --version          # expect: v1.1.x or higher
ssh-keygen -E sha256 -l -f /dev/null 2>&1 || true   # verify ssh-keygen exists
```

## 3. One-Time Setup

### 3.1 Generate dedicated ed25519 keypair

```bash
# DO NOT reuse SSH login key. Generate fresh keypair scoped to secrets only.
KEY_PATH="$HOME/.ssh/mimikri_age"
ssh-keygen -t ed25519 -f "$KEY_PATH" -C "mimikri-secrets@$(hostname)" -N ""

# Verify
ls -la "$KEY_PATH"*
# -rw-------  $KEY_PATH       (private, 600)
# -rw-r--r--  $KEY_PATH.pub   (public, 644)

# Compute fingerprint for audit trail
ssh-keygen -E sha256 -lf "${KEY_PATH}.pub"
# Example: 256 SHA256:abc123... mimikri-secrets@box1 (ED25519)
```

**Record the fingerprint in a private location** (encrypted note, password manager, paper). Used later to verify recovery is correct key.

### 3.2 Lock down private key

```bash
chmod 600 "$KEY_PATH"

# Linux only — make immutable, prevents accidental delete/overwrite by root
sudo chattr +i "$KEY_PATH"

# Remove with: sudo chattr -i "$KEY_PATH" (only when intentional)
```

### 3.3 Create secrets vault directory

```bash
sudo mkdir -p /opt/mimikri/secrets
sudo chown $USER:$USER /opt/mimikri/secrets
chmod 700 /opt/mimikri/secrets
```

### 3.4 Encrypt initial secrets file

```bash
# Compose plaintext (NEVER save unencrypted to disk; pipe directly)
cat <<'EOF' | age -R "$HOME/.ssh/mimikri_age.pub" -o /opt/mimikri/secrets/env.age
DO_TOKEN=dop_v1_REPLACE_WITH_REAL_TOKEN
H1_API_KEY=REPLACE_WITH_HACKERONE_KEY
NVD_API_KEY=REPLACE_WITH_NVD_KEY
C2_TOKEN=REPLACE_WITH_C2_TOKEN
POSTGRES_PASSWORD=REPLACE_WITH_PG_PASSWORD
TAILSCALE_AUTHKEY=tskey-auth-REPLACE
CERTSTREAM_KEYWORDS=acme.com,target.io
MIMIKRI_AUTHORIZED_SCOPE=acme-2026
EOF

# Verify encrypted
file /opt/mimikri/secrets/env.age   # expect: age encrypted file
ls -la /opt/mimikri/secrets/env.age
```

### 3.5 Test decryption round-trip

```bash
age -i "$HOME/.ssh/mimikri_age" -d /opt/mimikri/secrets/env.age
# Expect plaintext key=value lines back. If garbled or error → keypair mismatch, abort.
```

## 4. Daily Operations

### 4.1 Boot-time decryption (Box1 coordinator)

Add to systemd unit `mimikri-coordinator.service`:

```ini
[Unit]
Description=Mimikri RedTeam Coordinator
After=network-online.target tailscaled.service
Wants=network-online.target

[Service]
Type=simple
User=mimikri
WorkingDirectory=/opt/mimikri
ExecStartPre=/usr/local/bin/mimikri-load-secrets.sh
ExecStart=/opt/mimikri/redteam_rust_core --target-file /etc/mimikri/targets.txt --dashboard 8080 --postgres-url postgres://localhost:5432/mimikri
EnvironmentFile=/run/mimikri/env
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Wrapper script `/usr/local/bin/mimikri-load-secrets.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

KEY="$HOME/.ssh/mimikri_age"
VAULT="/opt/mimikri/secrets/env.age"
RUNDIR="/run/mimikri"
ENVFILE="$RUNDIR/env"

# tmpfs only — never persist plaintext to disk
mkdir -p "$RUNDIR"
mount | grep -q "$RUNDIR" || mount -t tmpfs -o size=1M,mode=700,uid=$(id -u mimikri) tmpfs "$RUNDIR"

# Decrypt to tmpfs
age -i "$KEY" -d "$VAULT" > "$ENVFILE"
chmod 600 "$ENVFILE"
chown mimikri:mimikri "$ENVFILE"

# Validate non-empty + has required keys
for required in DO_TOKEN POSTGRES_PASSWORD; do
  grep -q "^${required}=" "$ENVFILE" || { echo "FATAL: missing $required"; exit 1; }
done
```

Install:
```bash
sudo install -m 0750 mimikri-load-secrets.sh /usr/local/bin/
sudo systemctl daemon-reload
sudo systemctl enable --now mimikri-coordinator.service
```

**Critical**: `/run/mimikri` is tmpfs → wiped on reboot, plaintext never touches disk.

### 4.2 Add new secret

```bash
KEY="$HOME/.ssh/mimikri_age"
VAULT="/opt/mimikri/secrets/env.age"

# Decrypt to memory, append, re-encrypt
{
  age -i "$KEY" -d "$VAULT"
  echo "NEW_SECRET_KEY=new_value_here"
} | age -R "${KEY}.pub" -o "${VAULT}.new"

# Atomic replace
mv "${VAULT}.new" "$VAULT"
chmod 600 "$VAULT"

# Reload coordinator to pick up new env
sudo systemctl restart mimikri-coordinator.service
```

### 4.3 Rotate existing secret

```bash
KEY="$HOME/.ssh/mimikri_age"
VAULT="/opt/mimikri/secrets/env.age"
TARGET_KEY="DO_TOKEN"   # variable to rotate
NEW_VALUE="dop_v1_NEW_VALUE"

age -i "$KEY" -d "$VAULT" | \
  sed "s|^${TARGET_KEY}=.*|${TARGET_KEY}=${NEW_VALUE}|" | \
  age -R "${KEY}.pub" -o "${VAULT}.new"

mv "${VAULT}.new" "$VAULT"
chmod 600 "$VAULT"
sudo systemctl restart mimikri-coordinator.service
```

### 4.4 Remove secret

```bash
KEY="$HOME/.ssh/mimikri_age"
VAULT="/opt/mimikri/secrets/env.age"
TARGET_KEY="OLD_API_KEY"

age -i "$KEY" -d "$VAULT" | \
  grep -v "^${TARGET_KEY}=" | \
  age -R "${KEY}.pub" -o "${VAULT}.new"

mv "${VAULT}.new" "$VAULT"
sudo systemctl restart mimikri-coordinator.service
```

### 4.5 Audit secret list (without revealing values)

```bash
age -i "$HOME/.ssh/mimikri_age" -d /opt/mimikri/secrets/env.age | cut -d= -f1
```

## 5. Backup Procedure

### 5.1 Encrypted USB backup (primary)

```bash
# Format USB with LUKS (one-time, destroys data on USB)
DEVICE=/dev/sdX   # IDENTIFY CORRECTLY, this destroys the device
sudo cryptsetup luksFormat "$DEVICE"
sudo cryptsetup open "$DEVICE" mimikri-backup
sudo mkfs.ext4 /dev/mapper/mimikri-backup
sudo mount /dev/mapper/mimikri-backup /mnt/backup

# Copy keypair + vault + fingerprint
sudo cp "$HOME/.ssh/mimikri_age" /mnt/backup/
sudo cp "$HOME/.ssh/mimikri_age.pub" /mnt/backup/
sudo cp /opt/mimikri/secrets/env.age /mnt/backup/
ssh-keygen -E sha256 -lf "$HOME/.ssh/mimikri_age.pub" | sudo tee /mnt/backup/FINGERPRINT.txt
date -u +"%Y-%m-%dT%H:%M:%SZ" | sudo tee /mnt/backup/BACKUP_DATE.txt

sync
sudo umount /mnt/backup
sudo cryptsetup close mimikri-backup
```

**Storage**: Physically separate location from primary box (different building if possible). Label only with non-descriptive code (e.g., `MK-B1-2026-05`). Never label "mimikri secrets backup".

### 5.2 Paper backup (paperkey, secondary)

```bash
# Generate human-readable hex dump of private key
ssh-keygen -p -m PEM -f "$HOME/.ssh/mimikri_age" -N "" -P ""   # ensure PEM format
paperkey --secret-key "$HOME/.ssh/mimikri_age" --output mimikri_age_paper.txt

# Print on archival paper (acid-free), store in fireproof location
lpr mimikri_age_paper.txt

# Securely delete tmp file
shred -u mimikri_age_paper.txt
```

**Recovery**: `paperkey --pubring=mimikri_age.pub --secrets=scanned_text.txt --output=mimikri_age_restored`

### 5.3 Shamir 2-of-3 split (high-paranoia option)

```bash
# Split private key into 3 shares, any 2 reconstruct
ssss-split -t 2 -n 3 -w mimikri-2026 < "$HOME/.ssh/mimikri_age" > shares.txt

# Distribute shares to 3 physically separate locations
# Reconstruct: ssss-combine -t 2 -w mimikri-2026
```

## 6. Disaster Recovery

### 6.1 Lost private key (vault intact)

If `$HOME/.ssh/mimikri_age` lost AND no backup:
- **Vault unrecoverable**. age has no recovery backdoor.
- Rotate ALL underlying secrets at source (revoke `DO_TOKEN`, regenerate `H1_API_KEY`, etc.)
- Regenerate keypair per §3.1, re-encrypt fresh secrets

### 6.2 Lost private key (backup available)

```bash
# From encrypted USB
sudo cryptsetup open /dev/sdX mimikri-backup
sudo mount /dev/mapper/mimikri-backup /mnt/backup
cp /mnt/backup/mimikri_age "$HOME/.ssh/"
cp /mnt/backup/mimikri_age.pub "$HOME/.ssh/"
chmod 600 "$HOME/.ssh/mimikri_age"

# Verify fingerprint matches recorded value
ssh-keygen -E sha256 -lf "$HOME/.ssh/mimikri_age.pub"
cat /mnt/backup/FINGERPRINT.txt
# MUST match. If not: wrong backup or tampered.

sudo umount /mnt/backup
sudo cryptsetup close mimikri-backup

# Test decryption
age -i "$HOME/.ssh/mimikri_age" -d /opt/mimikri/secrets/env.age | head -1
```

### 6.3 Compromised private key (suspected exfiltration)

Treat as worst case:

1. **Immediately**: revoke all secrets at their sources
   - DigitalOcean: regenerate `DO_TOKEN` in DO Console, destroy any droplets that used compromised token
   - HackerOne: regenerate `H1_API_KEY`
   - NVD: regenerate `NVD_API_KEY`
   - Postgres: `ALTER USER mimikri WITH PASSWORD 'new_strong_password'`
   - Tailscale: revoke auth-key in admin console, kick affected devices
   - Sliver/C2: regenerate token, rotate listener certs
2. Generate fresh keypair per §3.1
3. Re-encrypt vault with new pubkey using rotated secrets
4. Destroy compromised private key: `shred -u $HOME/.ssh/mimikri_age_compromised`
5. Update all backups (USB, paper, Shamir shares — old ones now useless)
6. Post-mortem: how did key leak? Add detection (auditctl, file integrity monitoring)

## 7. Annual Rotation Schedule

Suggested calendar:

| Month | Action |
|---|---|
| Jan | Rotate `H1_API_KEY`, `NVD_API_KEY` |
| Apr | Rotate `DO_TOKEN`, refresh Tailscale auth-key |
| Jul | Rotate keypair entirely (§7.1), refresh backups |
| Oct | Rotate `POSTGRES_PASSWORD`, `C2_TOKEN` |
| Annual | Verify paper backup readability, verify USB backup mount |

### 7.1 Keypair rotation procedure

```bash
KEY_OLD="$HOME/.ssh/mimikri_age"
KEY_NEW="$HOME/.ssh/mimikri_age_new"
VAULT="/opt/mimikri/secrets/env.age"

# Generate new keypair
ssh-keygen -t ed25519 -f "$KEY_NEW" -C "mimikri-secrets@$(hostname)-$(date +%Y%m)" -N ""
chmod 600 "$KEY_NEW"

# Re-encrypt vault with new pubkey
age -i "$KEY_OLD" -d "$VAULT" | age -R "${KEY_NEW}.pub" -o "${VAULT}.rotated"

# Verify new decryption works BEFORE replacing
age -i "$KEY_NEW" -d "${VAULT}.rotated" | grep -q "DO_TOKEN=" || { echo "FATAL: rotation failed"; exit 1; }

# Atomic swap
mv "${VAULT}.rotated" "$VAULT"
sudo chattr -i "$KEY_OLD"
mv "$KEY_OLD" "${KEY_OLD}.retired"
mv "${KEY_OLD}.pub" "${KEY_OLD}.pub.retired"
mv "$KEY_NEW" "$KEY_OLD"
mv "${KEY_NEW}.pub" "${KEY_OLD}.pub"
sudo chattr +i "$KEY_OLD"

# Record new fingerprint
ssh-keygen -E sha256 -lf "${KEY_OLD}.pub"

# Refresh backups per §5

# Shred retired key after 7-day overlap window
shred -u "${KEY_OLD}.retired"
```

## 8. Integration with Mimikri Code

### 8.1 Current state (`utils/config.rs`)

Engine reads secrets from environment variables. The systemd unit's `EnvironmentFile=/run/mimikri/env` injects decrypted vault contents transparently. No code change required in `utils/config.rs`.

### 8.2 Future enhancement (proposed Sprint 8.C)

Add native `age` decryption in `utils/config.rs` as fallback when running outside systemd:

```rust
// Pseudocode — actual implementation TBD in Sprint 8.C
pub fn load_from_age_vault(vault_path: &Path, identity_path: &Path) -> Result<HashMap<String, String>> {
    let identity = age::ssh::Identity::from_buffer(BufReader::new(File::open(identity_path)?), None)?;
    let decryptor = match age::Decryptor::new(BufReader::new(File::open(vault_path)?))? {
        age::Decryptor::Recipients(d) => d,
        _ => bail!("vault not recipient-encrypted"),
    };
    let mut reader = decryptor.decrypt(iter::once(&identity as &dyn age::Identity))?;
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    parse_env_format(&content)
}
```

Crate dependency: `age = { version = "0.10", features = ["ssh"] }`

Selection precedence:
1. CLI arg `--age-vault /opt/mimikri/secrets/env.age --age-identity ~/.ssh/mimikri_age`
2. Env vars `MIMIKRI_AGE_VAULT` + `MIMIKRI_AGE_IDENTITY`
3. Standard env vars (`DO_TOKEN`, etc.) directly
4. Empty defaults

## 9. Audit and Verification

### 9.1 Daily checks (cron)

```bash
# /etc/cron.daily/mimikri-secrets-audit
#!/usr/bin/env bash
set -euo pipefail

KEY="/home/mimikri/.ssh/mimikri_age"
VAULT="/opt/mimikri/secrets/env.age"
LOG="/var/log/mimikri/secrets-audit.log"

mkdir -p "$(dirname "$LOG")"

{
  echo "=== $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
  echo "Key fingerprint: $(ssh-keygen -E sha256 -lf "${KEY}.pub")"
  echo "Key perms: $(stat -c '%a %U:%G' "$KEY")"
  echo "Key immutable: $(lsattr "$KEY" | awk '{print $1}')"
  echo "Vault size: $(stat -c '%s bytes, modified %y' "$VAULT")"
  echo "Vault decrypts: $(age -i "$KEY" -d "$VAULT" >/dev/null 2>&1 && echo OK || echo FAIL)"
  echo "Secret count: $(age -i "$KEY" -d "$VAULT" | wc -l)"
} >> "$LOG"

# Alert if decrypt fails
age -i "$KEY" -d "$VAULT" >/dev/null 2>&1 || \
  curl -sf -X POST "$DISCORD_ALERT_WEBHOOK" -d "{\"content\":\"⚠️ Mimikri secrets vault decryption FAILED on $(hostname)\"}"
```

### 9.2 Linux audit subsystem

```bash
# Watch private key file for any access
sudo auditctl -w /home/mimikri/.ssh/mimikri_age -p rwxa -k mimikri_secrets

# Inspect access log
sudo ausearch -k mimikri_secrets -ts today
```

### 9.3 File integrity baseline (AIDE)

```bash
sudo aide --init
sudo cp /var/lib/aide/aide.db.new /var/lib/aide/aide.db
# Schedule check
echo "0 3 * * * root /usr/bin/aide --check | mail -s 'AIDE report' root" | sudo tee /etc/cron.d/aide-check
```

## 10. Threat Model Coverage

### Protects against
- **Disk theft / offline analysis**: vault encrypted, useless without keypair
- **Accidental git commit of plaintext**: secrets never on disk in plaintext
- **Backup tape compromise**: same as disk theft
- **Cloud snapshot leak**: vault encrypted at rest
- **Shoulder-surfing console**: no secrets typed manually after setup
- **Root accident on key file**: `chattr +i` prevents `rm -rf` mishap

### Does NOT protect against
- **Compromised root with running mimikri**: plaintext in `/run/mimikri/env` (tmpfs) readable, RAM extraction via `/proc/$PID/mem`
- **Kernel rootkit**: any in-memory secret recoverable
- **Compromised Tailscale or SSH**: if attacker gets shell access as `mimikri` user, can read decrypted env directly
- **Hardware coercion**: no PIN/biometric gate, possession of key = full access
- **Side-channel on age decryption**: theoretical, not in current threat budget

### Mitigations for residual risk
- **Run mimikri as dedicated unprivileged user**: limits compromise blast radius
- **systemd `PrivateTmp=yes`, `ProtectSystem=strict`**: harden service unit
- **Tailscale ACLs**: limit SSH ingress to admin device only
- **2FA on Tailscale account**: TOTP via authenticator app (free)
- **fail2ban on SSH**: rate-limit brute force
- **Log all sudo to remote box3 Loki**: detect post-compromise lateral movement

## 11. Migration Path to YubiKey (when budget allows)

When affording a YubiKey 5 / OnlyKey / Nitrokey 3 / Token2 PIN+:

1. Install `age-plugin-yubikey` (or equivalent for chosen token)
2. Generate identity bound to YubiKey slot:
   ```bash
   age-plugin-yubikey --generate
   # Records identity file referencing hardware token
   ```
3. Re-encrypt vault adding YubiKey as recipient alongside ed25519:
   ```bash
   age -i "$HOME/.ssh/mimikri_age" -d "$VAULT" | \
     age -R "${HOME}/.ssh/mimikri_age.pub" \
         -r "age1yubikey1q..." \
         -o "${VAULT}.dual"
   mv "${VAULT}.dual" "$VAULT"
   ```
4. Both keys can decrypt during transition window
5. After validation: remove ed25519 recipient, retire keypair per §7.1
6. New backup: YubiKey itself (PIN-protected, hardware-bound) — software backup keys retired

## 12. Quick Reference Card

```bash
# Decrypt to stdout
age -i ~/.ssh/mimikri_age -d /opt/mimikri/secrets/env.age

# Add/modify secret (atomic)
{ age -i ~/.ssh/mimikri_age -d /opt/mimikri/secrets/env.age | \
  grep -v "^FOO="; echo "FOO=bar"; } | \
  age -R ~/.ssh/mimikri_age.pub -o /opt/mimikri/secrets/env.age.new && \
  mv /opt/mimikri/secrets/env.age.new /opt/mimikri/secrets/env.age

# Verify decryption works
age -i ~/.ssh/mimikri_age -d /opt/mimikri/secrets/env.age >/dev/null && echo OK

# List secret names only
age -i ~/.ssh/mimikri_age -d /opt/mimikri/secrets/env.age | cut -d= -f1

# Reload coordinator after secret change
sudo systemctl restart mimikri-coordinator.service

# Fingerprint of current keypair
ssh-keygen -E sha256 -lf ~/.ssh/mimikri_age.pub
```

## 13. Cost

**Total: $0 USD**

| Item | Cost | Replaces |
|---|---|---|
| `age` software | $0 (BSD-licensed) | OCI Vault (~$24/yr or free tier) |
| ed25519 keypair | $0 (crypto-generated) | YubiKey 5 (~$55 USD) |
| USB drive for backup | reuse existing | dedicated HSM (~$300+) |
| Paper printout | ~$0.01 | offsite secure storage |
| Audit cron | $0 (built-in) | SIEM subscription |

Frees the original $24/yr OCI Vault budget line in `HYBRID_DEPLOYMENT_TOPOLOGY.md` §7 → reallocate to Object Storage overage buffer or egress.

## 14. References

- age project: https://age-encryption.org/
- ssh-rsa/ed25519 recipients spec: https://github.com/FiloSottile/age/blob/main/doc/age-encryption.org.txt
- paperkey: https://www.jabberwocky.com/software/paperkey/
- Shamir Secret Sharing (`ssss`): http://point-at-infinity.org/ssss/
- OWASP Secret Management Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html
- `HYBRID_DEPLOYMENT_TOPOLOGY.md` §7 (cost model integration)
- `stealth_opsec.md` (broader OPSEC context)
