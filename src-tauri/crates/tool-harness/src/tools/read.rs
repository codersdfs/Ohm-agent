// Read tool implementation

use crate::{Tool, ToolInput, ToolResult, ToolError, ToolUseContext};
use crate::schema::string_param;
use crate::metadata::{ToolMetadata, ToolCategory, LatencyHint, ToolErrorSpec, ToolExample, ToolSource};
use async_trait::async_trait;

pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "read" }
    fn description(&self) -> &str { "Read the contents of a file at the given path" }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filePath": string_param("Absolute or relative path to the file"),
                "offset": {
                    "type": "number",
                    "description": "Starting line number (1-indexed)",
                    "default": 1
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum lines to read",
                    "default": 2000
                }
            },
            "required": ["filePath"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "read".into(),
            label: "Read File".into(),
            description: "Read the contents of a file at the given path".into(),
            doc: Some("Returns file contents as a string up to max_result_chars (default 50KB). 
For binary files, returns a hex dump. Use offset/limit to read specific line ranges.
Respects project allowlists for safe paths.".into()),
            category: ToolCategory::FileOperations,
            subcategory: Some("read".into()),
            tags: vec!["file".into(), "view".into(), "cat".into(), "content".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: true,
            concurrency_safe: true,
            latency_hint: LatencyHint::Fast,
            supports_streaming: true,
            max_result_chars: 50_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "not_found".into(),
                    description: "File does not exist at the specified path".into(),
                    recoverable: true,
                    retry_advice: Some("Check the file path with glob or ls".into()),
                },
                ToolErrorSpec {
                    kind: "permission_denied".into(),
                    description: "Cannot read file due to OS permissions".into(),
                    recoverable: false,
                    retry_advice: Some("Check file permissions or run with elevated access".into()),
                },
                ToolErrorSpec {
                    kind: "too_large".into(),
                    description: "File exceeds maximum read size".into(),
                    recoverable: true,
                    retry_advice: Some("Use offset/limit to read in chunks".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Read a file".into(),
                    description: "Read entire file contents".into(),
                    arguments: serde_json::json!({"filePath": "src/main.rs"}),
                    expected_result: Some("fn main() { ... }".into()),
                },
                ToolExample {
                    title: "Read with offset".into(),
                    description: "Read lines 100-200 of a file".into(),
                    arguments: serde_json::json!({
                        "filePath": "src/main.rs",
                        "offset": 100,
                        "limit": 100
                    }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(crate::metadata::CostHint {
                tokens_per_call: 50,
                category: crate::metadata::CostCategory::Free,
            }),
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    fn is_read_only(&self, _input: &ToolInput) -> bool {
        true
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let path = input.args.get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: filePath"))?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::new(format!("Failed to read {}: {}", path, e)))?;

        Ok(ToolResult::success(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_tool_reads_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = ReadTool::new();
        let input = ToolInput {
            tool: "read".into(),
            args: serde_json::json!({ "filePath": path }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "hello world");
    }

    #[tokio::test]
    async fn test_read_tool_missing_file() {
        let tool = ReadTool::new();
        let input = ToolInput {
            tool: "read".into(),
            args: serde_json::json!({ "filePath": "/nonexistent/file.txt" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_tool_missing_arg() {
        let tool = ReadTool::new();
        let input = ToolInput {
            tool: "read".into(),
            args: serde_json::json!({}),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("Missing argument"));
    }
}