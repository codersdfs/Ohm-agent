// Tool trait definition

use async_trait::async_trait;
use crate::{ToolInput, ToolResult, ToolError, PermissionResult, ToolUseContext};
use crate::metadata::{ToolMetadata, ToolCategory, LatencyHint};

/// The core Tool trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the name of this tool
    fn name(&self) -> &str;

    /// Returns a description of what this tool does
    fn description(&self) -> &str;

    /// Returns the JSON schema for tool parameters
    fn parameters_schema(&self) -> serde_json::Value;

    /// Returns full metadata for the Tool Calling Box.
    ///
    /// The default implementation builds a stub from [`name()`], [`description()`],
    /// and [`parameters_schema()`]. Tools should override this to provide
    /// richer metadata (category, tags, examples, error specs, etc.).
    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        let name = self.name().to_string();
        ToolMetadata {
            name: name.clone(),
            label: name,
            description: self.description().to_string(),
            doc: None,
            category: ToolCategory::HelpDocs,
            subcategory: None,
            tags: vec![],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: false,
            concurrency_safe: false,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 30_000,
            errors: vec![],
            examples: vec![],
            cost_hint: None,
            version: "1.0.0".into(),
            deprecation: None,
            source: crate::metadata::ToolSource::Builtin,
            source_name: None,
        }
    }

    /// Check if this tool is read-only (no file modifications)
    fn is_read_only(&self, _input: &ToolInput) -> bool {
        self.metadata().read_only
    }

    /// Check if this tool is safe for concurrent execution
    fn is_concurrency_safe(&self, _input: &ToolInput) -> bool {
        self.metadata().concurrency_safe
    }

    /// Maximum result size in characters before truncation/persistence
    fn max_result_size_chars(&self) -> usize {
        self.metadata().max_result_chars
    }

    /// Check permissions for this tool execution
    fn check_permissions(&self, _input: &ToolInput, _ctx: &ToolUseContext) -> PermissionResult {
        PermissionResult::Allow
    }

    /// Execute the tool
    async fn call(&self, input: ToolInput, ctx: &ToolUseContext) -> Result<ToolResult, ToolError>;
}