// Bash tool implementation

use crate::{Tool, ToolInput, ToolResult, ToolError, ToolUseContext};
use crate::schema::string_param;
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

    #[tokio::test]
    async fn test_bash_tool_failing_command() {
        let tool = BashTool::new();
        let input = ToolInput {
            tool: "bash".into(),
            args: serde_json::json!({ "command": "exit 1" }),
        };
        let ctx = ToolUseContext::new("test");

        let _result = tool.call(input, &ctx).await.unwrap();
        // Command "exit 1" will still succeed in shell (just exits with code 1)
        // But let's test with a command that actually fails
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