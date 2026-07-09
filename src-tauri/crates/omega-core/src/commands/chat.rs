use serde::{Deserialize, Serialize};
use crate::{AppState, MutexExt};
use crate::ChatEmitter;
use colored::Colorize;
use std::sync::atomic::{AtomicU64, Ordering};

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

static COST_INPUT: AtomicU64 = AtomicU64::new(0);
static COST_OUTPUT: AtomicU64 = AtomicU64::new(0);
static COST_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn cost_report() -> String {
    format!(
        "  {}cost: session total — {} in / {} out ({} messages){}",
        DIM,
        COST_INPUT.load(Ordering::Relaxed),
        COST_OUTPUT.load(Ordering::Relaxed),
        COST_COUNT.load(Ordering::Relaxed),
        RESET,
    )
}

fn record_cost(input: u32, output: u32) {
    COST_INPUT.fetch_add(input as u64, Ordering::Relaxed);
    COST_OUTPUT.fetch_add(output as u64, Ordering::Relaxed);
    COST_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Read the current session's cumulative token counts.
pub fn session_token_counts() -> (u64, u64) {
    (
        COST_INPUT.load(Ordering::Relaxed),
        COST_OUTPUT.load(Ordering::Relaxed),
    )
}

/// A no-op ChatEmitter used by send_message (non-interactive API call).
pub struct NoopEmitter;

impl ChatEmitter for NoopEmitter {
    fn emit_token(&self, _token: &str) -> Result<(), String> { Ok(()) }
    fn emit_done(&self, _full: &str) -> Result<(), String> { Ok(()) }
    fn emit_error(&self, _error: &str) -> Result<(), String> { Ok(()) }
}

enum Permission {
    Allow,
    Deny,
    Abort,
}

async fn check_permission(mode: &str, tool: &str, _args: &str) -> Permission {
    match mode {
        "strict" => {
            eprintln!("  {}{} denied (strict mode){}", DIM, tool, RESET);
            Permission::Deny
        }
        "on" => {
            use std::io::Write;
            use tokio::io::AsyncBufReadExt;
            let mut input = String::new();
            let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
            loop {
                eprint!("  Allow {}? (y/N/q): ", tool);
                std::io::stderr().flush().ok();
                input.clear();
                if reader.read_line(&mut input).await.is_err() {
                    return Permission::Deny;
                }
                match input.trim().to_lowercase().as_str() {
                    "y" | "yes" => return Permission::Allow,
                    "" | "n" | "no" => return Permission::Deny,
                    "q" | "quit" => return Permission::Abort,
                    _ => continue,
                }
            }
        }
        _ => Permission::Allow,
    }
}
fn show_diff(path: &str, old: &str, new: &str) {
    if old == new {
        return;
    }
    eprintln!("  {} {} {}", "──", path, "──");
    let diff = similar::TextDiff::from_lines(old, new);
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        let line = change.value().trim_end_matches('\n');
        if line.is_empty() {
            continue;
        }
        match change.tag() {
            similar::ChangeTag::Equal => {
                eprintln!("  {} {}{}{}", sign, DIM, line, RESET);
            }
            _ => {
                eprintln!("  {} {}", sign, line);
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    pub agent_type: String,
    pub provider: Option<providers::ProviderConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub content: String,
    pub agent_type: String,
}

pub async fn send_message(
    state: &AppState,
    request: SendMessageRequest,
) -> Result<SendMessageResponse, String> {
    log::debug!("send_message: agent={}, content={:?}", request.agent_type, request.content.chars().take(50).collect::<String>());

    let config = request.provider.unwrap_or_else(|| {
        let s = state.provider_config.lock_guard();
        s.clone()
    });

    let provider = providers::create_provider(&config)?;
    let tools = crate::commands::tools::tool_definitions();

    let mut messages = vec![
        providers::ChatMessage {
            role: "system".into(),
            content: crate::commands::tools::default_system_prompt(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        providers::ChatMessage {
            role: "user".into(),
            content: request.content.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];

    let mut max_loops = 10;

    loop {
        if max_loops == 0 {
            return Err("Tool call loop exceeded max iterations".into());
        }
        max_loops -= 1;

        let chat_request = providers::ChatRequest {
            messages: messages.clone(),
            config: config.clone(),
            stream: false,
            tools: Some(tools.clone()),
        };

        let response = provider.chat(chat_request).await?;

        if let Some(tool_calls) = response.tool_calls {
            // Use the shared handle_tool_calls which handles permission checking,
            // diff display, and emitter hooks — the inline loop was a partial duplicate.
            handle_tool_calls(state, &tool_calls, &mut messages, "off", &NoopEmitter).await?;
        } else {
            return Ok(SendMessageResponse {
                message_id: uuid::Uuid::new_v4().to_string(),
                content: response.content,
                agent_type: request.agent_type,
            });
        }
    }
}

async fn handle_tool_calls<E: ChatEmitter>(
    state: &AppState,
    tool_calls: &[providers::ToolCall],
    messages: &mut Vec<providers::ChatMessage>,
    permission_mode: &str,
    emitter: &E,
) -> Result<(), String> {
    messages.push(providers::ChatMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: Some(tool_calls.to_vec()),
        tool_call_id: None,
        name: None,
    });
    for tc in tool_calls {
        emitter.emit_tool_call(&tc.function.name, &tc.function.arguments)?;
        let args = match serde_json::from_str(&tc.function.arguments) {
            Ok(v) => v,
            Err(e) => {
                messages.push(providers::ChatMessage {
                    role: "tool".into(),
                    content: format!("Error parsing arguments for `{}`: {}.\nArguments received: {}", tc.function.name, e, tc.function.arguments),
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.function.name.clone()),
                });
                continue;
            }
        };
        let tool_request = crate::commands::tools::ToolRequest {
            tool: tc.function.name.clone(),
            args,
        };
        // Check permission FIRST — before any file I/O
        match check_permission(permission_mode, &tc.function.name, &tc.function.arguments).await {
            Permission::Allow => {}
            Permission::Deny => {
                messages.push(providers::ChatMessage {
                    role: "tool".into(),
                    content: format!("Tool `{}` was denied by permission mode", tc.function.name),
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.function.name.clone()),
                });
                continue;
            }
            Permission::Abort => return Err("Message aborted by user".into()),
        }

        // Read old file content for diff (permission already granted)
        let diff_path = if matches!(tc.function.name.as_str(), "write" | "edit") {
            tool_request.args.get("filePath").and_then(|v| v.as_str()).map(|p| p.to_string())
        } else {
            None
        };
        let old = diff_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();

        let result = match crate::commands::tools::execute_tool_inner(state, tool_request).await {
            Ok(r) => r,
            Err(e) => crate::commands::tools::ToolResult::err(e),
        };

        // Show diff after execution
        if let Some(ref path) = diff_path {
            let new = std::fs::read_to_string(path).unwrap_or_default();
            show_diff(path, &old, &new);
        }
        let output = if result.success { &result.output } else { result.error.as_deref().unwrap_or("") };
        emitter.emit_tool_result(&tc.function.name, result.success, output)?;
        messages.push(providers::ChatMessage {
            role: "tool".into(),
            content: output.to_string(),
            tool_calls: None,
            tool_call_id: Some(tc.id.clone()),
            name: Some(tc.function.name.clone()),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessageRequest {
    pub content: String,
    pub agent_type: String,
    pub provider: Option<providers::ProviderConfig>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub permission_mode: String,
    /// If true, show an indicatif progress spinner (CLI mode).
    /// Set to false in TUI mode where the spinner is in the status bar.
    #[serde(default = "default_true")]
    pub show_progress: bool,
}

fn default_true() -> bool { true }

pub async fn stream_message_with_history<E: ChatEmitter>(
    state: &AppState,
    request: StreamMessageRequest,
    emitter: &E,
    messages: &mut Vec<providers::ChatMessage>,
) -> Result<String, String> {
    log::debug!("stream_message: agent={}", request.agent_type);

    let config = request.provider.unwrap_or_else(|| {
        let s = state.provider_config.lock_guard();
        s.clone()
    });

    let tools = crate::commands::tools::tool_definitions();

    if messages.is_empty() {
        let sys_prompt = request.system_prompt
            .unwrap_or_else(crate::commands::tools::default_system_prompt);
        messages.push(providers::ChatMessage {
            role: "system".into(),
            content: sys_prompt,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
    }
    messages.push(providers::ChatMessage {
        role: "user".into(),
        content: request.content,
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    let provider = std::sync::Arc::new(providers::create_provider(&config)?);
    let mut full_response = String::new();
    let mut max_loops: u32 = 10;

    loop {
        if max_loops == 0 {
            return Err("Tool call loop exceeded max iterations".into());
        }
        max_loops -= 1;

        if config.kind.supports_streaming() {
            log::debug!("streaming: provider={:?} tools={}", config.kind, tools.len());
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            let chat_request = providers::ChatRequest {
                messages: messages.clone(),
                config: config.clone(),
                stream: true,
                tools: Some(tools.clone()),
            };

            let p = provider.clone();
            tokio::spawn(async move {
                let _ = p.chat_stream(chat_request, tx).await;
            });

            let spinner = if request.show_progress {
                let s = indicatif::ProgressBar::new_spinner();
                s.set_style(
                    indicatif::ProgressStyle::default_spinner()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                        .template("{spinner} {msg}")
                        .unwrap(),
                );
                s.set_message("Thinking...");
                s.enable_steady_tick(std::time::Duration::from_millis(80));
                Some(s)
            } else {
                None
            };

            let mut streaming_text = false;
            let mut tool_call_deltas: Vec<(usize, String, String, String)> = vec![];
            let mut last_usage: Option<providers::Usage> = None;

            while let Some(chunk) = rx.recv().await {
                // Emit thinking/reasoning tokens (model-internal reasoning)
                if !chunk.thinking.is_empty() {
                    emitter.emit_thinking(&chunk.thinking)?;
                }

                if !chunk.content.is_empty() {
                    if !streaming_text {
                        streaming_text = true;
                        if let Some(ref s) = spinner {
                            s.finish_and_clear();
                        }
                    }
                    emitter.emit_token(&chunk.content)?;
                    full_response.push_str(&chunk.content);
                }

                if let Some(ref deltas) = chunk.delta_tool_calls {
                    // Model is producing tool calls — clear spinner if still spinning
                    if !streaming_text {
                        streaming_text = true;
                        if let Some(ref s) = spinner {
                            s.finish_and_clear();
                        }
                    }
                    log::debug!("received {} tool call deltas", deltas.len());
                    for d in deltas {
                        let pos = tool_call_deltas.iter().position(|(idx, _, _, _)| *idx == d.index);
                        if let Some(p) = pos {
                            let entry = &mut tool_call_deltas[p];
                            if let Some(ref id_val) = d.id {
                                if entry.1.is_empty() {
                                    entry.1.push_str(id_val);
                                }
                            }
                            if let Some(ref name) = d.function.as_ref().and_then(|f| f.name.as_ref()) {
                                entry.2.push_str(name);
                            }
                            if let Some(ref args) = d.function.as_ref().and_then(|f| f.arguments.as_ref()) {
                                entry.3.push_str(args);
                            }
                        } else {
                            let mut id_buf = String::new();
                            let mut name_buf = String::new();
                            let mut args_buf = String::new();
                            if let Some(ref id_val) = d.id {
                                id_buf.push_str(id_val);
                            }
                            if let Some(ref n) = d.function.as_ref().and_then(|f| f.name.as_ref()) {
                                name_buf.push_str(n);
                            }
                            if let Some(ref a) = d.function.as_ref().and_then(|f| f.arguments.as_ref()) {
                                args_buf.push_str(a);
                            }
                            tool_call_deltas.push((d.index, id_buf, name_buf, args_buf));
                        }
                    }
                }

                if chunk.done {
                    last_usage = chunk.usage;
                    break;
                }
            }

            if let Some(ref s) = spinner {
                s.finish_and_clear();
            }

            if !tool_call_deltas.is_empty() {
                log::debug!("executing {} accumulated tool calls", tool_call_deltas.len());
                let tool_calls: Vec<providers::ToolCall> = tool_call_deltas.iter().map(|(_index, id, name, args)| {
                    providers::ToolCall {
                        id: if id.is_empty() { format!("call_{}", _index) } else { id.clone() },
                        tool_type: "function".into(),
                        function: providers::ToolCallFunction {
                            name: name.clone(),
                            arguments: args.clone(),
                        },
                    }
                }).collect();

                handle_tool_calls(state, &tool_calls, messages, &request.permission_mode, emitter).await?;
                continue;
            }

            emitter.emit_done(&full_response)?;
            if let Some(ref u) = last_usage {
                record_cost(u.input_tokens, u.output_tokens);
                eprintln!("  {}tokens: {} in / {} out{}", DIM, u.input_tokens, u.output_tokens, RESET);
            }
            return Ok(full_response);
        } else {
            let spinner = if request.show_progress {
                let s = indicatif::ProgressBar::new_spinner();
                s.set_style(
                    indicatif::ProgressStyle::default_spinner()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                        .template("{spinner} {msg}")
                        .unwrap(),
                );
                s.set_message("Thinking...");
                s.enable_steady_tick(std::time::Duration::from_millis(80));
                Some(s)
            } else {
                None
            };

            let chat_request = providers::ChatRequest {
                messages: messages.clone(),
                config: config.clone(),
                stream: false,
                tools: Some(tools.clone()),
            };

            let response = provider.chat(chat_request).await?;
            if let Some(ref s) = spinner {
                s.finish_and_clear();
            }

            if let Some(tool_calls) = response.tool_calls {
                handle_tool_calls(state, &tool_calls, messages, &request.permission_mode, emitter).await?;
                continue;
            }

            if !response.content.is_empty() {
                emitter.emit_token(&response.content)?;
                full_response.push_str(&response.content);
            }
            emitter.emit_done(&full_response)?;
            if let Some(ref u) = response.usage {
                record_cost(u.input_tokens, u.output_tokens);
                eprintln!("  {}tokens: {} in / {} out{}", DIM, u.input_tokens, u.output_tokens, RESET);
            }
            return Ok(full_response);
        }
    }
}

pub async fn stream_message<E: ChatEmitter>(
    state: &AppState,
    request: StreamMessageRequest,
    emitter: &E,
) -> Result<String, String> {
    let mut messages = Vec::new();
    stream_message_with_history(state, request, emitter, &mut messages).await
}

pub async fn list_models(config: &providers::ProviderConfig) -> Vec<String> {
    match providers::fetch_models(config).await {
        Ok(models) => models.into_iter().map(|m| m.id).collect(),
        Err(_) => {
            let fallback: &[&str] = match config.kind {
                providers::ProviderKind::OpenAI => &[
                    "gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-3.5-turbo",
                ],
                providers::ProviderKind::Anthropic => &[
                    "claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022",
                    "claude-opus-4-20250514",
                ],
                providers::ProviderKind::Groq => &[
                    "llama-3.3-70b-versatile", "mixtral-8x7b-32768",
                ],
                providers::ProviderKind::XAI => &[
                    "grok-3", "grok-3-mini",
                ],
                providers::ProviderKind::Local => &["ollama"],
                _ => &["unknown"],
            };
            fallback.iter().map(|s| s.to_string()).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChatEmitter;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct TestEmitter;

    impl ChatEmitter for TestEmitter {
        fn emit_token(&self, _token: &str) -> Result<(), String> { Ok(()) }
        fn emit_done(&self, _full: &str) -> Result<(), String> { Ok(()) }
        fn emit_error(&self, _error: &str) -> Result<(), String> { Ok(()) }
    }

    fn sse_line(value: &serde_json::Value) -> String {
        format!("data: {}\n\n", serde_json::to_string(value).unwrap())
    }

    fn build_sse_response(events: &[serde_json::Value]) -> Vec<u8> {
        let mut body = String::new();
        for event in events {
            body.push_str(&sse_line(event));
        }
        body.push_str("data: [DONE]\n\n");
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )
        .into_bytes()
    }

    fn tool_call_sse() -> Vec<u8> {
        build_sse_response(&[
            serde_json::json!({"choices":[{"index":0,"delta":{"content":""},"finish_reason":null}]}),
            serde_json::json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"glob","arguments":""}}]},"finish_reason":null}]}),
            serde_json::json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"pattern\":\"**/*.rs\"}"}}]},"finish_reason":null}]}),
            serde_json::json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
        ])
    }

    fn text_sse() -> Vec<u8> {
        build_sse_response(&[
            serde_json::json!({"choices":[{"index":0,"delta":{"content":"Done"},"finish_reason":null}]}),
            serde_json::json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
        ])
    }

    #[tokio::test]
    async fn test_stream_message_tool_calls_execute_and_push_results() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let request_count = Arc::new(AtomicUsize::new(0));
        let counter = request_count.clone();
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(());

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.changed() => break,
                    result = listener.accept() => {
                        match result {
                            Ok((mut stream, _)) => {
                                let mut buf = [0u8; 4096];
                                let _ = stream.read(&mut buf).await;
                                let idx = counter.fetch_add(1, Ordering::SeqCst);
                                let resp = if idx == 0 { tool_call_sse() } else { text_sse() };
                                let _ = stream.write_all(&resp).await;
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::task::yield_now().await;

        let cfg = providers::ProviderConfig {
            kind: providers::ProviderKind::Local,
            api_key: None,
            base_url: Some(format!("http://127.0.0.1:{}", port)),
            model: "mock".into(),
            max_tokens: 1024,
            temperature: 0.0,
        };
        let state = AppState::new_with_provider_config(":memory:", cfg.clone());

        let request = StreamMessageRequest {
            content: "list rust files".into(),
            agent_type: "chat".into(),
            provider: Some(cfg),
            system_prompt: None,
            permission_mode: "off".into(),
        };

        let emitter = TestEmitter;
        let mut messages = Vec::new();
        let result = stream_message_with_history(&state, request, &emitter, &mut messages).await;

        assert!(result.is_ok(), "stream_message_with_history failed: {:?}", result.err());
        assert!(!messages.is_empty(), "messages buffer should contain history");

        let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
        assert!(
            roles.contains(&"user"),
            "should have user message, got roles: {:?}",
            roles
        );
        assert!(
            roles.contains(&"tool"),
            "should have tool result message, got roles: {:?}",
            roles
        );
        assert!(
            roles.contains(&"assistant"),
            "should have assistant response, got roles: {:?}",
            roles
        );

        let first_assistant = messages.iter().find(|m| m.role == "assistant").unwrap();
        assert!(
            first_assistant.tool_calls.is_some(),
            "first assistant message should have tool_calls"
        );

        drop(cancel_tx);
        handle.await.ok();
    }

    #[tokio::test]
    async fn test_stream_message_preserves_history_across_calls() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let request_count = Arc::new(AtomicUsize::new(0));
        let counter = request_count.clone();
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(());

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.changed() => break,
                    result = listener.accept() => {
                        match result {
                            Ok((mut stream, _)) => {
                                let mut buf = [0u8; 4096];
                                let _ = stream.read(&mut buf).await;
                                let idx = counter.fetch_add(1, Ordering::SeqCst);
                                let resp = if idx == 0 { tool_call_sse() } else { text_sse() };
                                let _ = stream.write_all(&resp).await;
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::task::yield_now().await;

        let cfg = providers::ProviderConfig {
            kind: providers::ProviderKind::Local,
            api_key: None,
            base_url: Some(format!("http://127.0.0.1:{}", port)),
            model: "mock".into(),
            max_tokens: 1024,
            temperature: 0.0,
        };
        let state = AppState::new_with_provider_config(":memory:", cfg.clone());
        let emitter = TestEmitter;
        let mut messages = Vec::new();

        let r1 = stream_message_with_history(
            &state,
            StreamMessageRequest {
                content: "first call".into(),
                agent_type: "chat".into(),
                provider: Some(cfg.clone()),
                system_prompt: None,
                permission_mode: "off".into(),
            },
            &emitter,
            &mut messages,
        )
        .await;
        assert!(r1.is_ok());

        let r2 = stream_message_with_history(
            &state,
            StreamMessageRequest {
                content: "second call".into(),
                agent_type: "chat".into(),
                provider: Some(cfg.clone()),
                system_prompt: None,
                permission_mode: "off".into(),
            },
            &emitter,
            &mut messages,
        )
        .await;
        assert!(r2.is_ok());

        assert!(
            messages.len() > 4,
            "history should grow across calls, got {} messages",
            messages.len()
        );

        let msgs: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
        let user_positions: Vec<usize> = msgs.iter().enumerate()
            .filter(|(_, r)| **r == "user")
            .map(|(i, _)| i)
            .collect();
        assert!(
            user_positions.len() >= 2,
            "should have at least 2 user messages (1 per call), got {}",
            user_positions.len()
        );

        drop(cancel_tx);
        handle.await.ok();
    }

    #[tokio::test]
    async fn test_stream_message_handles_parse_error_tool_call() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let request_count = Arc::new(AtomicUsize::new(0));
        let counter = request_count.clone();
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(());

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_rx.changed() => break,
                    result = listener.accept() => {
                        match result {
                            Ok((mut stream, _)) => {
                                let mut buf = [0u8; 4096];
                                let _ = stream.read(&mut buf).await;
                                let idx = counter.fetch_add(1, Ordering::SeqCst);
                                let resp = if idx == 0 {
                                    // Tool call with invalid JSON arguments
                                    build_sse_response(&[
                                        serde_json::json!({"choices":[{"index":0,"delta":{"content":""},"finish_reason":null}]}),
                                        serde_json::json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_unparseable","type":"function","function":{"name":"glob","arguments":""}}]},"finish_reason":null}]}),
                                        serde_json::json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"NOT_VALID_JSON"}}]},"finish_reason":null}]}),
                                        serde_json::json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
                                    ])
                                } else {
                                    text_sse()
                                };
                                let _ = stream.write_all(&resp).await;
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::task::yield_now().await;

        let cfg = providers::ProviderConfig {
            kind: providers::ProviderKind::Local,
            api_key: None,
            base_url: Some(format!("http://127.0.0.1:{}", port)),
            model: "mock".into(),
            max_tokens: 1024,
            temperature: 0.0,
        };
        let state = AppState::new_with_provider_config(":memory:", cfg.clone());
        let emitter = TestEmitter;
        let mut messages = Vec::new();
        let result = stream_message_with_history(
            &state,
            StreamMessageRequest {
                content: "run broken tool".into(),
                agent_type: "chat".into(),
                provider: Some(cfg),
                system_prompt: None,
                permission_mode: "off".into(),
            },
            &emitter,
            &mut messages,
        )
        .await;

        assert!(result.is_ok(), "should not crash on parse error: {:?}", result.err());

        let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
        assert!(
            roles.contains(&"tool"),
            "should have a tool message even with parse error, got: {:?}",
            roles
        );

        drop(cancel_tx);
        handle.await.ok();
    }
}
