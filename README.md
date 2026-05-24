# mimikri 🐙

[![CI](https://github.com/kripiman/mimikri/actions/workflows/ci.yml/badge.svg)](https://github.com/kripiman/mimikri/actions)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

**mimikri** is a high-performance, async-first red team assessment engine with 70+ integrated plugins. It orchestrates reconnaissance, enumeration, exploitation, and reporting pipelines with a focus on stealth, modularity, and autonomous operation.

> ⚠️ **Authorized Use Only**: mimikri is an offensive security tool. Use it only on systems you own or have explicit written permission to test.

---

## Install

### From source (Rust required)

```bash
git clone https://github.com/kripiman/mimikri.git
cd mimikri
cargo install --path .
```

### Optional: install `retire.js` for the `js_deep` plugin

```bash
npm install -g retire
```

---

## Quick Start

```bash
# Scan a target with default settings
mimikri --target example.com

# Autonomous swarm mode with dashboard
mimikri --target example.com --autonomous --swarm --dashboard 8080

# Worker mode (distributed)
mimikri --worker --postgres-url "postgres://user:pass@localhost:5432/osintdb" --node-id worker-1
```

---

## Usage

```
mimikri [OPTIONS] --target <TARGET>

Options:
  -t, --target <TARGET>          Target host or domain
      --autonomous               Enable autonomous decision-making
      --swarm                    Enable multi-agent swarm coordination
      --dashboard <PORT>         Start web dashboard on specified port
      --worker                   Run in distributed worker mode
      --postgres-url <URL>       PostgreSQL connection string
      --jsonl-output <PATH>      Output findings as JSONL
      --html-output <PATH>       Generate HTML report
  -h, --help                     Print help
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `MIMIKRI_DESTRUCTIVE` | Set to `1` to enable destructive plugins |
| `MIMIKRI_APPROVAL_TIMEOUT` | Seconds to wait for interactive approval (default: 300) |
| `MIMIKRI_AUTHORIZED_SCOPE` | Comma-separated authorized target scope |
| `DATABASE_URL` | PostgreSQL connection for persistence |

---

## Plugin Development

mimikri uses a trait-based plugin system. New scanners implement the `ScannerPlugin` trait:

```rust
use mimikri::plugins::ScannerPlugin;

pub struct MyScanner;

#[async_trait]
impl ScannerPlugin for MyScanner {
    fn name(&self) -> &'static str { "my_scanner" }
    async fn scan(&self, target: &TargetHost) -> Vec<Finding> { /* ... */ }
}
```

Register your plugin in `plugins/scanner_factory.rs`.

---

## Architecture

- **4-Stage Pipeline**: Discovery → Liveness → Scanning → Sink
- **Plugin Registry**: 70+ tools integrated via subprocess wrappers or native Rust
- **Swarm Intelligence**: Multi-agent coordination with budget constraints
- **Stealth Layer**: Proxy rotation, jitter, decoy traffic, and egress hardening
- **Dashboard**: Real-time WebSocket dashboard for mission monitoring

---

## License

This project is licensed under the [AGPL-3.0-or-later](LICENSE) license.

Because mimikri includes a network dashboard (AGPL §13), the source code is prominently linked in the dashboard footer:

> Source available at [github.com/kripiman/mimikri](https://github.com/kripiman/mimikri)

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Security

See [SECURITY.md](SECURITY.md) for responsible disclosure policy.

## Third-Party Attributions

See [THIRDPARTY.md](THIRDPARTY.md) for vendored tools and their licenses.
