// Edit tool implementation

use crate::{Tool, ToolInput, ToolResult, ToolError, ToolUseContext};
use crate::schema::{string_param, boolean_param};
use crate::metadata::{ToolMetadata, ToolCategory, LatencyHint, ToolErrorSpec, ToolExample, ToolSource, CostHint, CostCategory};
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

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "edit".into(),
            label: "Edit File".into(),
            description: "Edit a file by finding and replacing a specific string.".into(),
            doc: Some("Performs a targeted text replacement in a file. 
By default replaces only the first occurrence. Set replaceAll: true to replace all occurrences.
The oldString must match exactly — whitespace and all. Use read first to see the exact content.
For large files, make sure oldString is unique enough to avoid unintended replacements.".into()),
            category: ToolCategory::FileOperations,
            subcategory: Some("edit".into()),
            tags: vec!["file".into(), "replace".into(), "patch".into(), "modify".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: false,
            concurrency_safe: false,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 500,
            errors: vec![
                ToolErrorSpec {
                    kind: "not_found".into(),
                    description: "The oldString was not found in the file".into(),
                    recoverable: true,
                    retry_advice: Some("Use read to check the actual file content and make sure oldString matches exactly".into()),
                },
                ToolErrorSpec {
                    kind: "file_not_found".into(),
                    description: "The specified file does not exist".into(),
                    recoverable: true,
                    retry_advice: Some("Use glob to find the file or write to create it first".into()),
                },
                ToolErrorSpec {
                    kind: "permission_denied".into(),
                    description: "Cannot write to the file".into(),
                    recoverable: false,
                    retry_advice: None,
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Replace first occurrence".into(),
                    description: "Replace the first match of 'foo' with 'bar'".into(),
                    arguments: serde_json::json!({
                        "filePath": "src/main.rs",
                        "oldString": "foo",
                        "newString": "bar"
                    }),
                    expected_result: Some("Edited src/main.rs successfully".into()),
                },
                ToolExample {
                    title: "Replace all occurrences".into(),
                    description: "Replace every match of 'old_func' with 'new_func'".into(),
                    arguments: serde_json::json!({
                        "filePath": "src/lib.rs",
                        "oldString": "old_func",
                        "newString": "new_func",
                        "replaceAll": true
                    }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 40, category: CostCategory::Cheap }),
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
