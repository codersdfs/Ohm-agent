// Core types for tool-harness

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Classification for tool errors
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorKind {
    NotFound,
    SchemaValidation,
    PermissionDenied,
    ExecutionFailed,
    Timeout,
    ProviderError,
    Aborted,
    BudgetExceeded,
    Internal,
}

/// Error type for tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolError {
    pub message: String,
    pub details: Option<String>,
    pub kind: ToolErrorKind,
    pub retryable: bool,
    pub source_tool: Option<String>,
}

impl ToolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: ToolErrorKind::Internal,
            message: message.into(),
            details: None,
            retryable: false,
            source_tool: None,
        }
    }

    pub fn with_details(message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            kind: ToolErrorKind::Internal,
            message: message.into(),
            details: Some(details.into()),
            retryable: false,
            source_tool: None,
        }
    }

    pub fn with_kind(kind: ToolErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            details: None,
            retryable: matches!(kind, ToolErrorKind::ProviderError | ToolErrorKind::Timeout),
            source_tool: None,
        }
    }

    pub fn with_kind_and_source(
        kind: ToolErrorKind,
        message: impl Into<String>,
        tool: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            details: None,
            retryable: matches!(kind, ToolErrorKind::ProviderError | ToolErrorKind::Timeout),
            source_tool: Some(tool.into()),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind, self.message)?;
        if let Some(ref details) = self.details {
            write!(f, " ({})", details)?;
        }
        Ok(())
    }
}

impl std::error::Error for ToolError {}

impl std::fmt::Display for ToolErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ToolErrorKind::NotFound => "not_found",
            ToolErrorKind::SchemaValidation => "schema_validation",
            ToolErrorKind::PermissionDenied => "permission_denied",
            ToolErrorKind::ExecutionFailed => "execution_failed",
            ToolErrorKind::Timeout => "timeout",
            ToolErrorKind::ProviderError => "provider_error",
            ToolErrorKind::Aborted => "aborted",
            ToolErrorKind::BudgetExceeded => "budget_exceeded",
            ToolErrorKind::Internal => "internal",
        };
        write!(f, "{}", s)
    }
}

/// Result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(msg.into()),
        }
    }
}

/// Input passed to a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    pub tool: String,
    pub args: serde_json::Value,
}

/// Tool request from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub tool: String,
    pub args: serde_json::Value,
}

impl ToolRequest {
    pub fn from_call(call: providers::ToolCall) -> Result<Self, String> {
        let args = serde_json::from_str(&call.function.arguments)
            .map_err(|e| format!("Failed to parse tool arguments: {}", e))?;
        Ok(Self {
            tool: call.function.name,
            args,
        })
    }

    pub fn into_input(self) -> ToolInput {
        ToolInput {
            tool: self.tool,
            args: self.args,
        }
    }
}

/// Outcome of tool execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutcome {
    Success(ToolResult),
    PermissionDenied(String),
    NotFound(String),
    Error(ToolError),
}

/// Result of permission check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionResult {
    Allow,
    Deny,
    Prompt(String), // Returns the prompt text for interactive mode
}

/// Result of budget check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetCheck {
    pub within_limit: bool,
    pub truncated: bool,
    pub persisted_path: Option<PathBuf>,
}

/// Gate violation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateViolationInfo {
    pub category: String,
    pub message: String,
    pub tool_hint: Option<String>,
    pub line: Option<u32>,
}

/// Gate check result for write/edit operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheckResult {
    pub passed: bool,
    pub score: u32,
    pub violations: Vec<GateViolationInfo>,
}

impl GateCheckResult {
    pub fn passed(score: u32) -> Self {
        Self {
            passed: true,
            score,
            violations: vec![],
        }
    }

    pub fn with_violations(violations: Vec<GateViolationInfo>) -> Self {
        let score = if violations.is_empty() {
            100
        } else {
            100usize.saturating_sub(violations.len() * 15) as u32
        };
        Self {
            passed: score >= 80,
            score,
            violations,
        }
    }
}
