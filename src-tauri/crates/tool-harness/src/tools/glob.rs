// Glob tool implementation

use crate::{Tool, ToolInput, ToolResult, ToolError, ToolUseContext};
use crate::schema::string_param;
use async_trait::async_trait;

pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str { "glob" }
    fn description(&self) -> &str { "Find files matching a glob pattern. Use for listing files in a directory structure." }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": string_param("The glob pattern (e.g. '**/*.rs', 'src/**/*.ts')"),
                "path": string_param("Base directory to search from")
            },
            "required": ["pattern"]
        })
    }

    fn is_read_only(&self, _input: &ToolInput) -> bool {
        true
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let pattern = input.args.get("path")
            .and_then(|v| v.as_str())
            .map(|p| format!("{}/{}", p.trim_end_matches('/').trim_end_matches('\\'), input.args.get("pattern").and_then(|v| v.as_str()).unwrap_or("*")))
            .unwrap_or_else(|| input.args.get("pattern").and_then(|v| v.as_str()).unwrap_or("*").to_string());

        let paths: Vec<String> = glob::glob(&pattern)
            .map_err(|e| ToolError::new(format!("Invalid glob pattern: {}", e)))?
            .filter_map(|e| e.ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        if paths.is_empty() {
            Ok(ToolResult::success("No matches found"))
        } else {
            Ok(ToolResult::success(paths.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_glob_tool_finds_files() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("a.rs"), "").await.unwrap();
        tokio::fs::write(temp_dir.path().join("b.txt"), "").await.unwrap();

        let tool = GlobTool::new();
        let input = ToolInput {
            tool: "glob".into(),
            args: serde_json::json!({
                "pattern": "*.rs",
                "path": temp_dir.path().to_str().unwrap()
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("a.rs"));
    }

    #[tokio::test]
    async fn test_glob_tool_no_matches() {
        let temp_dir = TempDir::new().unwrap();

        let tool = GlobTool::new();
        let input = ToolInput {
            tool: "glob".into(),
            args: serde_json::json!({
                "pattern": "*.nonexistent",
                "path": temp_dir.path().to_str().unwrap()
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "No matches found");
    }
}