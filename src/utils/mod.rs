pub mod activity_log;
pub mod bounty_exporter;
pub mod common;
pub mod config;
pub mod cve_cache;
pub mod cvss;
pub mod deduplication;
pub mod downloader;
pub mod executor;
pub mod ja4;
pub mod jitter;
pub mod liveness;
pub mod memory_monitor;
pub mod output_filter;
pub mod payload_server;
pub mod poc_generator;
pub mod process_guard;
pub mod program_config;
pub mod proxy; // Redirection to infrastructure/proxy.rs
pub mod report_gen;
pub mod security;
pub mod stealth_detect;
pub mod stealth_http;
pub mod telemetry;
pub mod tone;
pub mod tool_detection;
pub mod transport;

pub use executor::StealthExecutor;
pub use payload_server::PayloadServer;
pub use security::{is_ssrf_safe_host, validate_target};

pub use crate::infrastructure::proxy::ProxyManager;
pub use jitter::JitterSleep;
pub use liveness::LivenessChecker;
pub use memory_monitor::MemoryMonitor;
pub use process_guard::ExternalToolGuard;
pub use report_gen::generate_report;
pub use telemetry::{init_telemetry, shutdown_telemetry};
pub use tool_detection::{
    check_tool_availability, detect_tool, detect_tool_system, verify_tool_version,
};
pub mod hardware_detection;
pub use hardware_detection::{detect_infrastructure, HardwareInfo, InfrastructureType};
pub mod api_budget;
pub mod api_cache;
pub mod shodan_keyring;
