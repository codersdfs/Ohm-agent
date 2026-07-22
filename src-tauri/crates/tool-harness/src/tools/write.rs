// Write tool implementation

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;
use std::path::PathBuf;

pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }
    fn description(&self) -> &str {
        "Write content to a file, creating it if it doesn't exist. Overwrites existing content."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filePath": string_param("Absolute or relative path to the file"),
                "content": string_param("The full content to write")
            },
            "required": ["filePath", "content"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "write".into(),
            label: "Write File".into(),
            description: "Write content to a file, creating it if it doesn't exist. Overwrites existing content.".into(),
            doc: Some("Creates the file and any parent directories if they don't exist. 
Overwrites the entire file content — use edit for targeted replacements.
For new files, ensure parent directories exist or they will be created automatically.".into()),
            category: ToolCategory::FileOperations,
            subcategory: Some("write".into()),
            tags: vec!["file".into(), "create".into(), "save".into(), "overwrite".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: false,
            concurrency_safe: false,
            latency_hint: LatencyHint::Slow,
            supports_streaming: false,
            max_result_chars: 1_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "permission_denied".into(),
                    description: "Cannot write to the specified path due to OS permissions".into(),
                    recoverable: false,
                    retry_advice: Some("Check directory permissions or use a different path".into()),
                },
                ToolErrorSpec {
                    kind: "disk_full".into(),
                    description: "Insufficient disk space to write the file".into(),
                    recoverable: false,
                    retry_advice: None,
                },
                ToolErrorSpec {
                    kind: "invalid_path".into(),
                    description: "The specified path is not valid".into(),
                    recoverable: true,
                    retry_advice: Some("Check that the path contains valid characters".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Write a new file".into(),
                    description: "Create a file with content".into(),
                    arguments: serde_json::json!({
                        "filePath": "src/hello.rs",
                        "content": "fn main() { println!(\"Hello!\"); }"
                    }),
                    expected_result: Some("Wrote 35 bytes to src/hello.rs".into()),
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 30, category: CostCategory::Cheap }),
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    fn is_read_only(&self, _input: &ToolInput) -> bool {
        false
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let path = input
            .args
            .get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: filePath"))?;

        let content = input
            .args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: content"))?;

        // Create parent directories if needed
        if let Some(parent) = PathBuf::from(path).parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::new(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        tokio::fs::write(path, content)
            .await
            .map_err(|e| ToolError::new(format!("Failed to write {}: {}", path, e)))?;

        Ok(ToolResult::success(format!(
            "Wrote {} bytes to {}",
            content.len(),
            path
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_write_tool_creates_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = WriteTool::new();
        let input = ToolInput {
            tool: "write".into(),
            args: serde_json::json!({ "filePath": path.clone(), "content": "hello world" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Wrote 11 bytes"));

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_write_tool_creates_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir
            .path()
            .join("subdir")
            .join("file.txt")
            .to_str()
            .unwrap()
            .to_string();

        let tool = WriteTool::new();
        let input = ToolInput {
            tool: "write".into(),
            args: serde_json::json!({ "filePath": path.clone(), "content": "nested content" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "nested content");
    }
}
