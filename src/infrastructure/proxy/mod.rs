pub mod builder;
pub mod fingerprint;
pub mod health;
pub mod kill_switch;
pub mod managed_exit;
pub mod manager;
pub mod types;
pub mod wrap_cmd;

pub use manager::ProxyManager;
pub use types::{ManagedExit, ProxyConfig};

// Re-export constants
pub use manager::MAX_LATENCY_SAMPLES;
