// Glob tool implementation

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
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
    fn name(&self) -> &str {
        "glob"
    }
    fn description(&self) -> &str {
        "Find files matching a glob pattern. Use for listing files in a directory structure."
    }

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

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "glob".into(),
            label: "Find Files".into(),
            description: "Find files matching a glob pattern. Use for listing files in a directory structure.".into(),
            doc: Some("Lists all files matching the given glob pattern.
Use ** for recursive matching (e.g. '**/*.rs' finds all Rust files recursively).
Use ? for single-character wildcards.
The path parameter sets the base directory; the pattern is relative to it.
Returns one file path per line, sorted alphabetically.".into()),
            category: ToolCategory::SearchQuery,
            subcategory: Some("file-search".into()),
            tags: vec!["file".into(), "find".into(), "search".into(), "list".into(), "ls".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: true,
            concurrency_safe: true,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 50_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "invalid_pattern".into(),
                    description: "The glob pattern is invalid".into(),
                    recoverable: true,
                    retry_advice: Some("Check glob syntax — use ** for recursive, * for single-level".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Find all Rust files".into(),
                    description: "Recursively find all .rs files".into(),
                    arguments: serde_json::json!({
                        "pattern": "**/*.rs",
                        "path": "."
                    }),
                    expected_result: Some("src/main.rs\nsrc/lib.rs\n...".into()),
                },
                ToolExample {
                    title: "Find files in a specific directory".into(),
                    description: "Find all TypeScript files in src".into(),
                    arguments: serde_json::json!({
                        "pattern": "**/*.ts",
                        "path": "src"
                    }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 100, category: CostCategory::Cheap }),
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
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| {
                format!(
                    "{}/{}",
                    p.trim_end_matches('/').trim_end_matches('\\'),
                    input
                        .args
                        .get("pattern")
                        .and_then(|v| v.as_str())
                        .unwrap_or("*")
                )
            })
            .unwrap_or_else(|| {
                input
                    .args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("*")
                    .to_string()
            });

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
        tokio::fs::write(temp_dir.path().join("a.rs"), "")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.path().join("b.txt"), "")
            .await
            .unwrap();

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
