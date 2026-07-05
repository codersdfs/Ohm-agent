// Write tool implementation

use crate::{Tool, ToolInput, ToolResult, ToolError, ToolUseContext};
use crate::schema::string_param;
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
    fn name(&self) -> &str { "write" }
    fn description(&self) -> &str { "Write content to a file, creating it if it doesn't exist. Overwrites existing content." }
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

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let path = input.args.get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: filePath"))?;

        let content = input.args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: content"))?;

        // Create parent directories if needed
        if let Some(parent) = PathBuf::from(path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::new(format!("Failed to create directory {}: {}", parent.display(), e)))?;
        }

        tokio::fs::write(path, content)
            .await
            .map_err(|e| ToolError::new(format!("Failed to write {}: {}", path, e)))?;

        Ok(ToolResult::success(format!("Wrote {} bytes to {}", content.len(), path)))
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
        let path = temp_dir.path().join("subdir").join("file.txt").to_str().unwrap().to_string();

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