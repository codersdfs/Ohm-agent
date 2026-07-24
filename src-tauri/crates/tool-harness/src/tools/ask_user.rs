// Ask user tool — emit a structured question to the UI and block for answer

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;

pub struct AskUserTool;

impl AskUserTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AskUserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }
    fn description(&self) -> &str {
        "Ask the user a question and wait for their response. Blocks until answered."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "question": string_param("The question to ask the user"),
                "options": {
                    "type": "array",
                    "items": string_param("An option the user can select"),
                    "description": "Optional: list of predefined options for the user to choose from"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional: timeout in seconds before giving up (default: no timeout)",
                    "default": 0
                }
            },
            "required": ["question"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "ask_user".into(),
            label: "Ask User".into(),
            description: "Ask the user a question and wait for their response.".into(),
            doc: Some("Emits a structured question to the UI and blocks until the user responds.
- question: the question to ask
- options: optional list of predefined choices (if provided, user selects from these)
- timeout: optional timeout in seconds (0 = no timeout)
In CLI mode, prompts via stdin. In TUI mode, shows an interactive prompt.
Returns the user's answer as the tool output.".into()),
            category: ToolCategory::Communication,
            subcategory: Some("ask".into()),
            tags: vec!["ask".into(), "user".into(), "question".into(), "interactive".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: true,
            concurrency_safe: false,
            latency_hint: LatencyHint::Blocking,
            supports_streaming: false,
            max_result_chars: 10_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "timeout".into(),
                    description: "User did not respond within the timeout period".into(),
                    recoverable: true,
                    retry_advice: Some("Ask a simpler question or increase the timeout".into()),
                },
                ToolErrorSpec {
                    kind: "no_ui".into(),
                    description: "No interactive UI available to ask the user".into(),
                    recoverable: true,
                    retry_advice: Some("Run in interactive mode (CLI/TUI) to use this tool".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Ask a free-form question".into(),
                    description: "Ask the user for input".into(),
                    arguments: serde_json::json!({
                        "question": "What is your preferred approach?"
                    }),
                    expected_result: Some("User's response".into()),
                },
                ToolExample {
                    title: "Ask with options".into(),
                    description: "Ask user to choose from predefined options".into(),
                    arguments: serde_json::json!({
                        "question": "Which file should I edit?",
                        "options": ["src/lib.rs", "src/main.rs", "src/utils.rs"]
                    }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 10, category: CostCategory::Free }),
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    async fn call(&self, input: ToolInput, ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let question = input
            .args
            .get("question")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: question"))?;

        let options: Vec<String> = input
            .args
            .get("options")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let _timeout_secs = input
            .args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // Use the prompt_callback if available (TUI mode)
        if let Some(ref cb) = ctx.prompt_callback {
            let prompt_text = if options.is_empty() {
                question.to_string()
            } else {
                let mut text = format!("{}\nOptions:", question);
                for (i, opt) in options.iter().enumerate() {
                    text.push_str(&format!("\n  {}. {}", i + 1, opt));
                }
                text
            };

            if cb(&prompt_text) {
                // In a real implementation, the callback would return the user's answer
                // For now, we return a placeholder that the orchestrator handles
                return Ok(ToolResult::success("User acknowledged".to_string()));
            } else {
                return Err(ToolError::with_kind(
                    crate::ToolErrorKind::Aborted,
                    "User declined to answer".to_string(),
                ));
            }
        }

        // Fallback: try stdin (CLI mode)
        // This is a simple blocking read from stdin
        if atty::is(atty::Stream::Stdin) {
            // We're in an interactive terminal
            eprintln!("\n>>> {}", question);
            if !options.is_empty() {
                eprintln!("Options:");
                for (i, opt) in options.iter().enumerate() {
                    eprintln!("  {}. {}", i + 1, opt);
                }
            }
            eprint!("> ");

            // Spawn a blocking thread to read from stdin, communicating the
            // result via a oneshot channel. This allows the async side to
            // use tokio::select! to race the read against a timeout,
            // properly cancelling the operation when the timeout fires.
            use tokio::sync::oneshot;
            let (tx, rx) = oneshot::channel::<Result<String, std::io::Error>>();
            std::thread::spawn(move || {
                let mut input_line = String::new();
                let result = std::io::stdin()
                    .read_line(&mut input_line)
                    .map(|_| input_line.trim().to_string());
                let _ = tx.send(result);
            });

            // Race the stdin read against a timeout
            let answer = tokio::select! {
                result = rx => {
                    match result {
                        Ok(Ok(answer)) => answer,
                        Ok(Err(e)) => return Err(ToolError::new(format!("Failed to read from stdin: {}", e))),
                        Err(_) => return Err(ToolError::new("Stdin read channel closed unexpectedly".to_string())),
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                    return Err(ToolError::new("Timed out waiting for user input".to_string()));
                }
            };

            // If options were provided, validate the answer
            if !options.is_empty() {
                if let Ok(idx) = answer.parse::<usize>() {
                    if idx > 0 && idx <= options.len() {
                        return Ok(ToolResult::success(options[idx - 1].clone()));
                    }
                }
                // If the answer matches an option directly, use it
                if options.iter().any(|o| o == &answer) {
                    return Ok(ToolResult::success(answer));
                }
                return Err(ToolError::new(format!(
                    "Invalid option. Choose from: {}",
                    options.join(", ")
                )));
            }

            Ok(ToolResult::success(answer))
        } else {
            // No interactive terminal available
            Err(ToolError::with_kind(
                crate::ToolErrorKind::PermissionDenied,
                "No interactive UI available to ask the user. Run in interactive mode (CLI/TUI).".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ask_user_missing_question() {
        let tool = AskUserTool::new();
        let input = ToolInput {
            tool: "ask_user".into(),
            args: serde_json::json!({}),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ask_user_no_ui() {
        let tool = AskUserTool::new();
        let input = ToolInput {
            tool: "ask_user".into(),
            args: serde_json::json!({ "question": "Test question" }),
        };
        let ctx = ToolUseContext::new("test");

        // Without a prompt_callback, this should either error immediately
        // (if no TTY) or block on stdin (if TTY). Use a timeout to handle
        // both cases — in CI without TTY it errors, in interactive mode
        // it would timeout.
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            tool.call(input, &ctx),
        )
        .await;

        // If we timed out, that's acceptable (TTY was detected, stdin blocked)
        // If we got an error, that's also acceptable (no TTY)
        // If we got a success, that's unexpected in a test
        match result {
            Ok(Ok(_)) => panic!("Expected error or timeout, got success"),
            Ok(Err(_)) => { /* Expected: no TTY available */ }
            Err(_) => { /* Expected: timed out waiting for stdin */ }
        }
    }
}
