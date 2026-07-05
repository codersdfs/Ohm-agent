// Edit tool implementation

use crate::{Tool, ToolInput, ToolResult, ToolError, ToolUseContext};
use crate::schema::{string_param, boolean_param};
use async_trait::async_trait;

pub struct EditTool;

impl EditTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str { "edit" }
    fn description(&self) -> &str { "Edit a file by finding and replacing a specific string. For replacing substrings within a file." }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "filePath": string_param("Absolute or relative path to the file"),
                "oldString": string_param("The exact text to find and replace"),
                "newString": string_param("The replacement text"),
                "replaceAll": boolean_param("Replace all occurrences (default: false, replaces only the first)")
            },
            "required": ["filePath", "oldString", "newString"]
        })
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let path = input.args.get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: filePath"))?;

        let old_string = input.args.get("oldString")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: oldString"))?;

        let new_string = input.args.get("newString")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: newString"))?;

        let replace_all = input.args.get("replaceAll")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::new(format!("Failed to read {}: {}", path, e)))?;

        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        if new_content == content {
            return Err(ToolError::new(format!("oldString not found in {}", path)));
        }

        tokio::fs::write(path, &new_content)
            .await
            .map_err(|e| ToolError::new(format!("Failed to write {}: {}", path, e)))?;

        Ok(ToolResult::success(format!("Edited {} successfully", path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_edit_tool_replaces_first() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world hello").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let input = ToolInput {
            tool: "edit".into(),
            args: serde_json::json!({
                "filePath": path.clone(),
                "oldString": "hello",
                "newString": "hi"
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "hi world hello");
    }

    #[tokio::test]
    async fn test_edit_tool_replace_all() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world hello").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let input = ToolInput {
            tool: "edit".into(),
            args: serde_json::json!({
                "filePath": path.clone(),
                "oldString": "hello",
                "newString": "hi",
                "replaceAll": true
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "hi world hi");
    }

    #[tokio::test]
    async fn test_edit_tool_not_found() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let input = ToolInput {
            tool: "edit".into(),
            args: serde_json::json!({
                "filePath": path.clone(),
                "oldString": "notfound",
                "newString": "hi"
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("not found"));
    }
}