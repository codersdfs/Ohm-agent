//! Subagent result — structured summary from run records.

use serde::{Deserialize, Serialize};

/// How the subagent run ended.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunOutcome {
    /// Subagent completed successfully (tool call returned success).
    Completed,
    /// Subagent hit max turns without completing.
    MaxTurns,
    /// Subagent ran out of token budget.
    BudgetExhausted,
    /// Subagent hit a tool error.
    ToolError,
}

/// Structured summary of a subagent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    /// Prose summary (≤ 3 sentences), only if completed.
    pub summary: Option<String>,
    /// How the run ended.
    pub outcome: RunOutcome,
    /// Gate score if run completed (0.0-1.0).
    pub gate_score: Option<f64>,
    /// Files changed by successfully completed write calls.
    pub files_changed: Vec<String>,
}

impl SubagentResult {
    pub fn completed(
        summary: impl Into<String>,
        gate_score: Option<f64>,
        files_changed: Vec<String>,
    ) -> Self {
        Self {
            summary: Some(summary.into()),
            outcome: RunOutcome::Completed,
            gate_score,
            files_changed,
        }
    }

    pub fn budget_exhausted() -> Self {
        Self {
            summary: None,
            outcome: RunOutcome::BudgetExhausted,
            gate_score: None,
            files_changed: Vec::new(),
        }
    }

    pub fn tool_error(_error: impl Into<String>) -> Self {
        Self {
            summary: None,
            outcome: RunOutcome::ToolError,
            gate_score: None,
            files_changed: Vec::new(),
        }
    }

    /// Format as structured line: `Outcome · Gate score · Files changed`
    pub fn structured_line(&self) -> String {
        let gate = match &self.gate_score {
            Some(score) => format!("Gate: {:.2}", score),
            None => "Gate: N/A".to_string(),
        };
        let files = if self.files_changed.is_empty() {
            "Files: none".to_string()
        } else {
            format!("Files: {}", self.files_changed.join(", "))
        };
        format!("{:?} · {} · {}", self.outcome, gate, files)
    }
}
