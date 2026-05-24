pub mod classifier;
pub mod parser;

pub use classifier::{classify_script_severity, suggest_exploit_vector};
pub use parser::parse_nmap_xml;
