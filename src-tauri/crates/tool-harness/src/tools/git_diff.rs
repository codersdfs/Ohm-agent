// Git diff tool — thin wrapper around git CLI

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;

use super::git_status::run_git;

pub struct GitDiffTool;

impl GitDiffTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitDiffTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }
    fn description(&self) -> &str {
        "Show changes between commits, commit and working tree, or staged changes"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "repoPath": string_param("Path to the git repository (default: current directory)"),
                "staged": {
                    "type": "boolean",
                    "description": "Show staged changes only (default: false, shows unstaged)",
                    "default": false
                },
                "target": string_param("Optional: commit/branch to diff against (default: HEAD for staged, working tree for unstaged)")
            },
            "required": []
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "git_diff".into(),
            label: "Git Diff".into(),
            description: "Show changes between commits, commit and working tree, or staged changes"
                .into(),
            doc: Some(
                "Runs `git diff` and returns the output.
- Default (no args): shows unstaged changes (working tree vs index)
- staged=true: shows staged changes (index vs HEAD)
- target: diff against a specific commit/branch"
                    .into(),
            ),
            category: ToolCategory::System,
            subcategory: Some("git".into()),
            tags: vec!["git".into(), "diff".into(), "scm".into(), "changes".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: true,
            concurrency_safe: true,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 50_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "not_a_repo".into(),
                    description: "The specified path is not a git repository".into(),
                    recoverable: true,
                    retry_advice: Some(
                        "Run in a git repository or provide a valid repoPath".into(),
                    ),
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
                    title: "Show unstaged changes".into(),
                    description: "Show changes not yet staged".into(),
                    arguments: serde_json::json!({}),
                    expected_result: Some("diff --git a/file.rs b/file.rs\n...".into()),
                },
                ToolExample {
                    title: "Show staged changes".into(),
                    description: "Show changes staged for commit".into(),
                    arguments: serde_json::json!({ "staged": true }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint {
                tokens_per_call: 50,
                category: CostCategory::Free,
            }),
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

        let staged = input
            .args
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let target = input.args.get("target").and_then(|v| v.as_str());

        let mut args: Vec<&str> = vec!["diff"];
        if staged {
            args.push("--cached");
        }
        if let Some(t) = target {
            args.push(t);
        }

        let output = run_git(repo_path, &args)
            .await
            .map_err(|e| ToolError::new(format!("git diff failed: {}", e)))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if stdout.trim().is_empty() {
                Ok(ToolResult::success("No changes".to_string()))
            } else {
                Ok(ToolResult::success(stdout.trim().to_string()))
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(ToolError::new(format!(
                "git diff failed: {}",
                stderr.trim()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_git_diff_not_a_repo() {
        let tool = GitDiffTool::new();
        let input = ToolInput {
            tool: "git_diff".into(),
            args: serde_json::json!({ "repoPath": "/nonexistent/path/12345" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }
}
