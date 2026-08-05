//! # Omega Harness Library
//! 
//! Static analysis gate engine that runs code checks and produces structured results.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Represents a single violation or issue found during analysis
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    pub category: ViolationCategory,
    pub message: String,
    pub tool_hint: Option<String>,
    pub line: Option<u32>,
}

/// Enumeration of violation categories with associated penalty weights
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViolationCategory {
    #[serde(rename = "structural")]
    Structural,
    #[serde(rename = "taste")]
    Taste,
    #[serde(rename = "golden")]
    Golden,
    #[serde(rename = "repeated")]
    Repeated,
    #[serde(rename = "external")]
    External,
}

impl std::fmt::Display for ViolationCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViolationCategory::Structural => write!(f, "structural"),
            ViolationCategory::Taste => write!(f, "taste"),
            ViolationCategory::Golden => write!(f, "golden"),
            ViolationCategory::Repeated => write!(f, "repeated"),
            ViolationCategory::External => write!(f, "external"),
        }
    }
}

/// Result of a gate check (passes/fails based on violations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Final score (lower = more violations)
    pub score: u32,
    /// Whether the gate check passed
    pub passed: bool,
    /// All violations discovered during checking
    pub violations: Vec<Violation>,
}

impl GateResult {
    /// Create a successful result with no violations
    pub fn pass(score: u32) -> Self {
        Self { score, passed: true, violations: vec![] }
    }

    /// Create a gate result with a score and violations.
    /// The `passed` flag is determined by the score threshold (≥80 passes).
    pub fn evaluate(score: u32, violations: Vec<Violation>) -> Self {
        Self { score, passed: score >= 80, violations }
    }

    /// Check if this result represents a pass
    pub fn is_pass(&self) -> bool {
        self.violations.is_empty()
    }

    /// Check if this result represents a failure
    pub fn is_fail(&self) -> bool {
        !self.violations.is_empty()
    }
}
