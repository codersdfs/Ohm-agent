// Bash tool implementation

use crate::{Tool, ToolInput, ToolResult, ToolError, ToolUseContext};
use crate::schema::string_param;
use crate::metadata::{ToolMetadata, ToolCategory, LatencyHint, ToolErrorSpec, ToolExample, ToolSource, CostHint, CostCategory};
use async_trait::async_trait;

pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }
    fn description(&self) -> &str { "Execute a shell command on the system. Use for running scripts, installing packages, building projects, etc." }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": string_param("The shell command to execute")
            },
            "required": ["command"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "bash".into(),
            label: "Run Shell Command".into(),
            description: "Execute a shell command on the system. Use for running scripts, building, testing, and automation.".into(),
            doc: Some("Executes a command via the system shell (PowerShell on Windows, sh on Unix).
The command runs in the current working directory.
Output is captured from stdout; stderr is included on failure.
Commands run with the same permissions as the agent process.
Warning: Destructive commands (rm, del, write to system paths) should be used carefully.
For long-running commands, consider using timeout mechanisms.".into()),
            category: ToolCategory::CodeExecution,
            subcategory: Some("shell".into()),
            tags: vec!["shell".into(), "command".into(), "run".into(), "terminal".into(), "execute".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: false, // Depends on command — checked dynamically in is_read_only
            concurrency_safe: false,
            latency_hint: LatencyHint::Blocking,
            supports_streaming: true,
            max_result_chars: 50_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "command_not_found".into(),
                    description: "The specified command was not found on the system".into(),
                    recoverable: true,
                    retry_advice: Some("Check that the command is installed and available in PATH".into()),
                },
                ToolErrorSpec {
                    kind: "non_zero_exit".into(),
                    description: "Command exited with a non-zero status code".into(),
                    recoverable: true,
                    retry_advice: Some("Check stderr output for error details".into()),
                },
                ToolErrorSpec {
                    kind: "timeout".into(),
                    description: "Command exceeded execution timeout".into(),
                    recoverable: true,
                    retry_advice: Some("Simplify the command or increase timeout".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "List directory contents".into(),
                    description: "List files in the current directory".into(),
                    arguments: serde_json::json!({ "command": "ls -la" }),
                    expected_result: Some("total 42\n-rw-r--r-- ...".into()),
                },
                ToolExample {
                    title: "Run tests".into(),
                    description: "Run cargo tests".into(),
                    arguments: serde_json::json!({ "command": "cargo test" }),
                    expected_result: None,
                },
                ToolExample {
                    title: "Install a package".into(),
                    description: "Install npm dependencies".into(),
                    arguments: serde_json::json!({ "command": "npm install" }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 500, category: CostCategory::Moderate }),
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    fn is_read_only(&self, input: &ToolInput) -> bool {
        let cmd = input.args.get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // Basic read-only detection
        let read_only_patterns = ["ls", "cat", "echo", "pwd", "whoami", "date"];
        read_only_patterns.iter().any(|p| cmd.starts_with(p))
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let command = input.args.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: command"))?;

        #[cfg(windows)]
        let output = {
            tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", command])
                .output()
                .await
        };

        #[cfg(not(windows))]
        let output = {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
                .await
        };

        let output = output.map_err(|e| ToolError::new(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            let combined = if stderr.is_empty() {
                stdout.clone()
            } else {
                format!("{}\n{}", stdout, stderr)
            };
            return Ok(ToolResult::error(combined.trim().to_string()));
        }

        Ok(ToolResult::success(stdout.trim().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bash_tool_echo() {
        let tool = BashTool::new();
        let input = ToolInput {
            tool: "bash".into(),
            args: serde_json::json!({ "command": "echo hello" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("hello"));
    }

    #[test]
    fn test_read_only_detection() {
        let tool = BashTool::new();
        assert!(tool.is_read_only(&ToolInput {
            tool: "bash".into(),
            args: serde_json::json!({ "command": "ls" }),
        }));
        assert!(tool.is_read_only(&ToolInput {
            tool: "bash".into(),
            args: serde_json::json!({ "command": "cat file.txt" }),
        }));
        assert!(!tool.is_read_only(&ToolInput {
            tool: "bash".into(),
            args: serde_json::json!({ "command": "rm file.txt" }),
        }));
    }
}
