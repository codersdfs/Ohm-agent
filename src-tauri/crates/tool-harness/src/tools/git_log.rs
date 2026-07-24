// Git log tool — thin wrapper around git CLI

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;

use super::git_status::run_git;

pub struct GitLogTool;

impl GitLogTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitLogTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }
    fn description(&self) -> &str {
        "Show commit history"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "repoPath": string_param("Path to the git repository (default: current directory)"),
                "limit": {
                    "type": "number",
                    "description": "Maximum number of commits to show (default: 10)",
                    "default": 10
                },
                "oneline": {
                    "type": "boolean",
                    "description": "Show one-line summary per commit (default: false)",
                    "default": false
                }
            },
            "required": []
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "git_log".into(),
            label: "Git Log".into(),
            description: "Show commit history".into(),
            doc: Some("Runs `git log` and returns the commit history.
- limit: max number of commits (default 10)
- oneline: compact one-line-per-commit format".into()),
            category: ToolCategory::System,
            subcategory: Some("git".into()),
            tags: vec!["git".into(), "log".into(), "history".into(), "commits".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: true,
            concurrency_safe: true,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 20_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "not_a_repo".into(),
                    description: "The specified path is not a git repository".into(),
                    recoverable: true,
                    retry_advice: Some("Run in a git repository or provide a valid repoPath".into()),
                },
                ToolErrorSpec {
                    kind: "git_not_found".into(),
                    description: "Git is not installed or not in PATH".into(),
                    recoverable: false,
                    retry_advice: Some("Install git and ensure it is in your PATH".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Show recent commits".into(),
                    description: "Show last 5 commits".into(),
                    arguments: serde_json::json!({ "limit": 5 }),
                    expected_result: None,
                },
                ToolExample {
                    title: "Show oneline history".into(),
                    description: "Compact one-line-per-commit view".into(),
                    arguments: serde_json::json!({ "oneline": true }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 50, category: CostCategory::Free }),
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let repo_path = input
            .args
            .get("repoPath")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let limit = input
            .args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let oneline = input
            .args
            .get("oneline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let limit_str = format!("-{}", limit);
        let mut args: Vec<&str> = vec!["log", &limit_str];
        if oneline {
            args.push("--oneline");
        }

        let output = run_git(repo_path, &args)
            .await
            .map_err(|e| ToolError::new(format!("git log failed: {}", e)))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if stdout.trim().is_empty() {
                Ok(ToolResult::success("No commits found".to_string()))
            } else {
                Ok(ToolResult::success(stdout.trim().to_string()))
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(ToolError::new(format!(
                "git log failed: {}",
                stderr.trim()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_git_log_not_a_repo() {
        let tool = GitLogTool::new();
        let input = ToolInput {
            tool: "git_log".into(),
            args: serde_json::json!({ "repoPath": "/nonexistent/path/12345" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }
}
