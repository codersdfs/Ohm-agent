// Git commit tool — stages files and creates a commit

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::{boolean_param, string_param};
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;

use super::git_status::run_git;

pub struct GitCommitTool;

impl GitCommitTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitCommitTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }
    fn description(&self) -> &str {
        "Stage files and create a git commit"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "repoPath": string_param("Path to the git repository (default: current directory)"),
                "message": string_param("Commit message"),
                "files": {
                    "type": "array",
                    "items": string_param("File path to stage"),
                    "description": "List of file paths to stage. If empty, stages all changes."
                },
                "skipHooks": boolean_param("Skip git hooks (default: false). Only allowed in strict permission mode.")
            },
            "required": ["message"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "git_commit".into(),
            label: "Git Commit".into(),
            description: "Stage files and create a git commit".into(),
            doc: Some("Creates a git commit with the specified message.
- files: list of file paths to stage (if empty, stages all changes)
- skipHooks: skip git hooks (only allowed in strict permission mode)
- Returns the commit hash on success
Never uses --no-verify unless skipHooks=true AND permission mode allows it.".into()),
            category: ToolCategory::System,
            subcategory: Some("git".into()),
            tags: vec!["git".into(), "commit".into(), "scm".into(), "version-control".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: false,
            concurrency_safe: false,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 1_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "nothing_to_commit".into(),
                    description: "No changes to commit".into(),
                    recoverable: true,
                    retry_advice: Some("Make changes before committing".into()),
                },
                ToolErrorSpec {
                    kind: "not_a_repo".into(),
                    description: "The specified path is not a git repository".into(),
                    recoverable: true,
                    retry_advice: Some("Run in a git repository or provide a valid repoPath".into()),
                },
                ToolErrorSpec {
                    kind: "hooks_skipped".into(),
                    description: "skipHooks was requested but permission mode does not allow it".into(),
                    recoverable: true,
                    retry_advice: Some("Use default mode or get explicit permission to skip hooks".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Commit all changes".into(),
                    description: "Stage all changes and commit".into(),
                    arguments: serde_json::json!({
                        "message": "Fix bug in parser"
                    }),
                    expected_result: Some("Committed as abc1234".into()),
                },
                ToolExample {
                    title: "Commit specific files".into(),
                    description: "Stage and commit specific files".into(),
                    arguments: serde_json::json!({
                        "message": "Add new feature",
                        "files": ["src/lib.rs", "tests/test.rs"]
                    }),
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

        let message = input
            .args
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: message"))?;

        let files: Vec<String> = input
            .args
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let skip_hooks = input
            .args
            .get("skipHooks")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Stage files
        if files.is_empty() {
            run_git(repo_path, &["add", "-A"])
                .await
                .map_err(|e| ToolError::new(format!("git add failed: {}", e)))?;
        } else {
            let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
            let mut args = vec!["add"];
            args.extend(file_refs);
            run_git(repo_path, &args)
                .await
                .map_err(|e| ToolError::new(format!("git add failed: {}", e)))?;
        }

        // Commit
        let mut commit_args: Vec<&str> = vec!["commit", "-m", message];
        if skip_hooks {
            commit_args.push("--no-verify");
        }

        let output = run_git(repo_path, &commit_args)
            .await
            .map_err(|e| ToolError::new(format!("git commit failed: {}", e)))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            // Extract commit hash from output like: [main abc1234] message
            let commit_hash = extract_commit_hash(&stdout);
            Ok(ToolResult::success(format!(
                "Committed as {}",
                commit_hash.unwrap_or_else(|| "unknown".to_string())
            )))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("nothing to commit") || stderr.contains("no changes added") {
                Err(ToolError::with_kind(
                    crate::ToolErrorKind::ExecutionFailed,
                    "Nothing to commit — no staged changes".to_string(),
                ))
            } else {
                Err(ToolError::new(format!("git commit failed: {}", stderr.trim())))
            }
        }
    }
}

fn extract_commit_hash(output: &str) -> Option<String> {
    // Output format: [main abc1234] message
    for line in output.lines() {
        // Try bracket format first: [branch abc1234]
        if let Some(start) = line.find('[') {
            if let Some(end) = line[start..].find(']') {
                let inner = &line[start + 1..start + end];
                // Format: branch abc1234
                let parts: Vec<&str> = inner.split_whitespace().collect();
                if parts.len() >= 2 {
                    return Some(parts[1].to_string());
                }
            }
        }
        // Also try parenthesis format: (branch abc1234)
        if let Some(start) = line.find('(') {
            if let Some(end) = line[start..].find(')') {
                let inner = &line[start + 1..start + end];
                let parts: Vec<&str> = inner.split_whitespace().collect();
                if parts.len() >= 2 {
                    return Some(parts[1].to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_git_commit_not_a_repo() {
        let tool = GitCommitTool::new();
        let input = ToolInput {
            tool: "git_commit".into(),
            args: serde_json::json!({
                "repoPath": "/nonexistent/path/12345",
                "message": "test commit"
            }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_commit_hash() {
        let output = "[main abc1234] Fix bug\n 1 file changed, 2 insertions(+)";
        let hash = extract_commit_hash(output);
        assert_eq!(hash, Some("abc1234".to_string()));
    }

    #[test]
    fn test_extract_commit_hash_no_match() {
        let output = "nothing to commit";
        let hash = extract_commit_hash(output);
        assert_eq!(hash, None);
    }
}
