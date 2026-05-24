// Detection and evasion plugins
#[cfg(feature = "sovereign")]
pub mod donut;
pub mod jitter;
#[cfg(feature = "sovereign")]
pub mod scarecrow;
pub mod stealth_policy;
