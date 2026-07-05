// Tool trait definition

use async_trait::async_trait;
use crate::{ToolInput, ToolResult, ToolError, PermissionResult, ToolUseContext};

/// The core Tool trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the name of this tool
    fn name(&self) -> &str;

    /// Returns a description of what this tool does
    fn description(&self) -> &str;

    /// Returns the JSON schema for tool parameters
    fn parameters_schema(&self) -> serde_json::Value;

    /// Check if this tool is read-only (no file modifications)
    fn is_read_only(&self, _input: &ToolInput) -> bool {
        false
    }

    /// Check if this tool is safe for concurrent execution
    fn is_concurrency_safe(&self, _input: &ToolInput) -> bool {
        false
    }

    /// Maximum result size in characters before truncation/persistence
    fn max_result_size_chars(&self) -> usize {
        30_000
    }

    /// Check permissions for this tool execution
    fn check_permissions(&self, _input: &ToolInput, _ctx: &ToolUseContext) -> PermissionResult {
        PermissionResult::Allow
    }

    /// Execute the tool
    async fn call(&self, input: ToolInput, ctx: &ToolUseContext) -> Result<ToolResult, ToolError>;
}