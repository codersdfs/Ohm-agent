// Grep tool implementation

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;
use regex::Regex;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "Search for a regex pattern across files in a directory. Returns matching lines with line numbers."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": string_param("The regex pattern to search for"),
                "path": string_param("Directory to search in (default: current directory)"),
                "include": string_param("Optional file extension filter (e.g. '*.rs', '*.js')")
            },
            "required": ["pattern"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "grep".into(),
            label: "Search Text".into(),
            description: "Search for a regex pattern across files in a directory. Returns matching lines with line numbers.".into(),
            doc: Some("Searches file contents using regex patterns. Supports filtering by file extension.
Results are returned as file:line:content for each match.
Searches up to 10 directory levels deep. For very large directories, narrow the search path.
The pattern is a Rust regex — use standard regex syntax.".into()),
            category: ToolCategory::SearchQuery,
            subcategory: Some("pattern".into()),
            tags: vec!["search".into(), "text".into(), "regex".into(), "find".into(), "code".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: true,
            concurrency_safe: true,
            latency_hint: LatencyHint::Slow,
            supports_streaming: true,
            max_result_chars: 50_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "invalid_regex".into(),
                    description: "The pattern is not a valid regular expression".into(),
                    recoverable: true,
                    retry_advice: Some("Check your regex syntax — escape special characters".into()),
                },
                ToolErrorSpec {
                    kind: "path_not_found".into(),
                    description: "The search path does not exist".into(),
                    recoverable: true,
                    retry_advice: Some("Check the path with glob or ls".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Search for a function".into(),
                    description: "Find all uses of a function name".into(),
                    arguments: serde_json::json!({
                        "pattern": "fn main",
                        "path": "src",
                        "include": "*.rs"
                    }),
                    expected_result: Some("src/main.rs:1: fn main() {".into()),
                },
                ToolExample {
                    title: "Search all files".into(),
                    description: "Find a string in any file".into(),
                    arguments: serde_json::json!({
                        "pattern": "TODO",
                        "path": "."
                    }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 200, category: CostCategory::Moderate }),
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
        let pattern = input
            .args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: pattern"))?;

        let path = input
            .args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let include = input.args.get("include").and_then(|v| v.as_str());

        let re = Regex::new(pattern)
            .map_err(|e| ToolError::new(format!("Invalid regex pattern: {}", e)))?;

        let search_dir = PathBuf::from(path);
        if !search_dir.exists() {
            return Err(ToolError::new(format!("Path does not exist: {}", path)));
        }

        let mut results = Vec::new();

        for entry in WalkDir::new(&search_dir)
            .max_depth(10)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let file_path = entry.path();

            // Apply include filter if specified
            if let Some(ext) = include {
                let ext_stripped = ext.trim_start_matches('*');
                if !file_path.to_string_lossy().ends_with(ext_stripped) {
                    continue;
                }
            }

            if let Ok(content) = tokio::fs::read_to_string(file_path).await {
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        results.push(format!(
                            "{}:{}: {}",
                            file_path.display(),
                            i + 1,
                            line.trim()
                        ));
                    }
                }
            }
        }

        if results.is_empty() {
            Ok(ToolResult::success("No matches found"))
        } else {
            Ok(ToolResult::success(results.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_grep_tool_finds_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "hello\nworld\nhello again")
            .await
            .unwrap();

        let tool = GrepTool::new();
        let input = ToolInput {
            tool: "grep".into(),
            args: serde_json::json!({
                "pattern": "hello",
                "path": temp_dir.path().to_str().unwrap()
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_grep_tool_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "hello\nworld").await.unwrap();

        let tool = GrepTool::new();
        let input = ToolInput {
            tool: "grep".into(),
            args: serde_json::json!({
                "pattern": "goodbye",
                "path": temp_dir.path().to_str().unwrap()
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "No matches found");
    }

    #[tokio::test]
    async fn test_grep_tool_invalid_regex() {
        let tool = GrepTool::new();
        let input = ToolInput {
            tool: "grep".into(),
            args: serde_json::json!({
                "pattern": "[invalid("
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }
}
