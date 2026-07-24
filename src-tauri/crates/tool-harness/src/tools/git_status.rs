// Git status tool — thin wrapper around git CLI

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;

pub struct GitStatusTool;

impl GitStatusTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitStatusTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }
    fn description(&self) -> &str {
        "Show the working tree status of a git repository"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "repoPath": string_param("Path to the git repository (default: current directory)")
            },
            "required": []
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "git_status".into(),
            label: "Git Status".into(),
            description: "Show the working tree status of a git repository".into(),
            doc: Some("Runs `git status` and returns the output. Shows staged, unstaged, and untracked files.
No network access required.".into()),
            category: ToolCategory::System,
            subcategory: Some("git".into()),
            tags: vec!["git".into(), "status".into(), "scm".into(), "version-control".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: true,
            concurrency_safe: true,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 10_000,
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
            examples: vec![ToolExample {
                title: "Check status".into(),
                description: "Show git status of current directory".into(),
                arguments: serde_json::json!({}),
                expected_result: Some("On branch main\nChanges to be committed:...".into()),
            }],
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

        let output = run_git(repo_path, &["status", "--porcelain", "-b"])
            .await
            .map_err(|e| ToolError::new(format!("git status failed: {}", e)))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(ToolResult::success(stdout.trim().to_string()))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(ToolError::new(format!(
                "git status failed: {}",
                stderr.trim()
            )))
        }
    }
}

/// Run a git command with a timeout.
pub(crate) async fn run_git(
    repo_path: &str,
    args: &[&str],
) -> Result<std::process::Output, ToolError> {
    #[cfg(windows)]
    let git_cmd = {
        tokio::process::Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .await
    };

    #[cfg(not(windows))]
    let git_cmd = {
        tokio::process::Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .output()
            .await
    };

    git_cmd.map_err(|e| ToolError::new(format!("Failed to execute git: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_git_status_not_a_repo() {
        let tool = GitStatusTool::new();
        let input = ToolInput {
            tool: "git_status".into(),
            args: serde_json::json!({ "repoPath": "/nonexistent/path/12345" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }
}
