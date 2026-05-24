use anyhow::Result;
use mimikri::core::approval_gate::ApprovalGate;
use mimikri::core::capability_layer::ScanLayerPolicy;
use mimikri::core::orchestrator::dispatch::dispatch_scan;
use mimikri::models::{TargetHost, TargetStatus, TargetType};
use mimikri::plugins::get_all_scanners;
use mimikri::plugins::GlobalConfig;
use mimikri::utils::executor::GhostMode;
use mimikri::utils::memory_monitor::MemoryMonitor;
use std::fs;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting REAL Golden Scan Baseline Capture (V15 Architecture)...");

    // 1. Setup Environment Dependencies (Explicit GhostMode for baseline)
    let config = GlobalConfig::<GhostMode>::new();
    let plugins = Arc::new(get_all_scanners(config.clone()));

    // Using established presets from V15 core
    let layer_policy = ScanLayerPolicy::preset_authorized_red_team();
    let approval_gate = Arc::new(ApprovalGate::for_red_team());

    let memory_monitor = Arc::new(MemoryMonitor::new(1024, 2048)); // 1GB/2GB limits
    let memory_semaphore = Arc::new(Semaphore::new(1024)); // 1024 permits

    // 2. Define Controlled Targets (matches docker-compose.test.yml)
    let target_hosts = vec![
        "127.0.0.1:8081".to_string(), // DVWA
        "127.0.0.1:445".to_string(),  // Samba
    ];

    let mut all_findings = Vec::new();

    // 3. Run Scans via dispatch_scan
    for host in target_hosts {
        println!("🔍 Scanning: {}", host);
        let target = Arc::new(TargetHost {
            host: host.clone(),
            ip: Some(host.split(':').next().unwrap().to_string()),
            resolved_ip: None,
            target_type: TargetType::Host,
            file_path: None,
            user: None,
            status: TargetStatus::Pending,
            findings: Arc::new(Vec::new()),
            tool_suggestions: Arc::new(Vec::new()),
            tactical_context: Arc::new(serde_json::json!({})),
            extra_data: Arc::new(serde_json::json!({})),
            version: 1,
            skip_heavy_scan: false,
            scan_id: None,
            scope_id: "baseline".to_string(),
        });

        let concurrency_semaphore = Arc::new(Semaphore::new(10));
        let policy = config.policy.clone();
        let strict_scope = false;
        let approval_timeout_secs = None;

        let (findings, _error) = dispatch_scan(
            target.clone(),
            plugins.clone(),
            layer_policy,
            approval_gate.clone(),
            memory_semaphore.clone(),
            memory_monitor.clone(),
            concurrency_semaphore,
            policy,
            strict_scope,
            approval_timeout_secs,
        )
        .await;

        println!("📥 Captured {} findings from {}", findings.len(), host);
        all_findings.extend(findings);
    }

    println!(
        "📊 Capture complete. Total findings: {}",
        all_findings.len()
    );

    // 4. Persistence to tests/baselines/
    let json = serde_json::to_string_pretty(&all_findings)?;
    fs::create_dir_all("tests/baselines")?;
    let path = std::env::current_dir()?.join("tests/baselines/golden_baseline.json");
    fs::write(&path, json)?;

    println!("✅ Golden Scan Baseline saved to tests/baselines/golden_baseline.json.");
    Ok(())
}
