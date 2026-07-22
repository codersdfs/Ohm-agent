// Entropy GC — Drift scanner, domain scorer, auto-GC PR generator
// Runs daily to detect structural drift and generate remediation PRs.

pub mod gc;
pub mod scanner;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntropyReport {
    pub domains: Vec<DomainScore>,
    pub generated_pr: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainScore {
    pub name: String,
    pub drift: f64,
    pub priority: u8,
}
