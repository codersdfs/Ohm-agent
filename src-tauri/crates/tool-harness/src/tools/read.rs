// Read tool implementation

use crate::{Tool, ToolInput, ToolResult, ToolError, ToolUseContext};
use crate::schema::string_param;
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
                "filePath": string_param("Absolute or relative path to the file")
            },
            "required": ["filePath"]
        })
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