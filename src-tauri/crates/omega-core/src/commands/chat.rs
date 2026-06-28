use serde::{Deserialize, Serialize};
use crate::AppState;
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
        let s = state.provider_config.lock().unwrap();
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
            messages.push(providers::ChatMessage {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
                name: None,
            });

            for tc in &tool_calls {
                let tool_request = crate::commands::tools::ToolRequest {
                    tool: tc.function.name.clone(),
                    args: serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::Null),
                };
                let result = crate::commands::tools::execute_tool_inner(state, tool_request).await?;
                messages.push(providers::ChatMessage {
                    role: "tool".into(),
                    content: result.output,
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.function.name.clone()),
                });
            }
        } else {
            return Ok(SendMessageResponse {
                message_id: uuid::Uuid::new_v4().to_string(),
                content: response.content,
                agent_type: request.agent_type,
            });
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessageRequest {
    pub content: String,
    pub agent_type: String,
    pub provider: Option<providers::ProviderConfig>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub permission_mode: String,
}

pub async fn stream_message<E: ChatEmitter>(
    state: &AppState,
    request: StreamMessageRequest,
    emitter: &E,
) -> Result<String, String> {
    log::debug!("stream_message: agent={}", request.agent_type);

    let config = request.provider.unwrap_or_else(|| {
        let s = state.provider_config.lock().unwrap();
        s.clone()
    });

    let tools = crate::commands::tools::tool_definitions();

    let mut messages: Vec<providers::ChatMessage> = vec![];
    let sys_prompt = request.system_prompt
        .unwrap_or_else(crate::commands::tools::default_system_prompt);
    messages.push(providers::ChatMessage {
        role: "system".into(),
        content: sys_prompt,
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });
    messages.push(providers::ChatMessage {
        role: "user".into(),
        content: request.content,
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    let mut full_response = String::new();
    let mut max_loops: u32 = 10;

    loop {
        if max_loops == 0 {
            return Err("Tool call loop exceeded max iterations".into());
        }
        max_loops -= 1;

        if config.kind.supports_streaming() {
            let provider = providers::create_provider(&config)?;
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            let chat_request = providers::ChatRequest {
                messages: messages.clone(),
                config: config.clone(),
                stream: true,
                tools: Some(tools.clone()),
            };

            tokio::spawn(async move {
                let _ = provider.chat_stream(chat_request, tx).await;
            });

            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner} {msg}")
                    .unwrap(),
            );
            spinner.set_message("Thinking...");
            spinner.enable_steady_tick(std::time::Duration::from_millis(80));

            let mut streaming_text = false;
            let mut tool_call_deltas: Vec<(usize, String, String)> = vec![];
            let mut last_usage: Option<providers::Usage> = None;

            while let Some(chunk) = rx.recv().await {
                if !chunk.content.is_empty() {
                    if !streaming_text {
                        streaming_text = true;
                        spinner.finish_and_clear();
                    }
                    emitter.emit_token(&chunk.content)?;
                    full_response.push_str(&chunk.content);
                }

                if let Some(deltas) = chunk.delta_tool_calls {
                    for d in deltas {
                        let pos = tool_call_deltas.iter().position(|(idx, _, _)| *idx == d.index);
                        if let Some(p) = pos {
                            let entry = &mut tool_call_deltas[p];
                            if let Some(ref name) = d.function.as_ref().and_then(|f| f.name.as_ref()) {
                                entry.1.push_str(name);
                            }
                            if let Some(ref args) = d.function.as_ref().and_then(|f| f.arguments.as_ref()) {
                                entry.2.push_str(args);
                            }
                        } else {
                            let mut name_buf = String::new();
                            let mut args_buf = String::new();
                            if let Some(ref n) = d.function.as_ref().and_then(|f| f.name.as_ref()) {
                                name_buf.push_str(n);
                            }
                            if let Some(ref a) = d.function.as_ref().and_then(|f| f.arguments.as_ref()) {
                                args_buf.push_str(a);
                            }
                            tool_call_deltas.push((d.index, name_buf, args_buf));
                        }
                    }
                }

                if chunk.done {
                    last_usage = chunk.usage;
                    break;
                }
            }

            spinner.finish_and_clear();

            if !tool_call_deltas.is_empty() {
                let tool_calls: Vec<providers::ToolCall> = tool_call_deltas.iter().map(|(index, name, args)| {
                    providers::ToolCall {
                        id: format!("call_{}", index),
                        tool_type: "function".into(),
                        function: providers::ToolCallFunction {
                            name: name.clone(),
                            arguments: args.clone(),
                        },
                    }
                }).collect();

                messages.push(providers::ChatMessage {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });
                for tc in &tool_calls {
                    eprintln!("  {} {} {}", "▶", tc.function.name.bold(), tc.function.arguments.dimmed());
                    let tool_request = crate::commands::tools::ToolRequest {
                        tool: tc.function.name.clone(),
                        args: serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Null),
                    };
                    let diff_path = if matches!(tc.function.name.as_str(), "write" | "edit") {
                        tool_request.args.get("filePath").and_then(|v| v.as_str()).map(|p| p.to_string())
                    } else {
                        None
                    };
                    let old = diff_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
                    match check_permission(&request.permission_mode, &tc.function.name, &tc.function.arguments).await {
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
                    let result = crate::commands::tools::execute_tool_inner(state, tool_request).await?;
                    if let Some(ref path) = diff_path {
                        let new = std::fs::read_to_string(path).unwrap_or_default();
                        show_diff(path, &old, &new);
                    }
                    messages.push(providers::ChatMessage {
                        role: "tool".into(),
                        content: result.output,
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }
                continue;
            }

            emitter.emit_done(&full_response)?;
            if let Some(ref u) = last_usage {
                record_cost(u.input_tokens, u.output_tokens);
                eprintln!("  {}tokens: {} in / {} out{}", DIM, u.input_tokens, u.output_tokens, RESET);
            }
            return Ok(full_response);
        } else {
            let provider = providers::create_provider(&config)?;

            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner} {msg}")
                    .unwrap(),
            );
            spinner.set_message("Thinking...");
            spinner.enable_steady_tick(std::time::Duration::from_millis(80));

            let chat_request = providers::ChatRequest {
                messages: messages.clone(),
                config: config.clone(),
                stream: false,
                tools: Some(tools.clone()),
            };

            let response = provider.chat(chat_request).await?;
            spinner.finish_and_clear();

            if let Some(tool_calls) = response.tool_calls {
                messages.push(providers::ChatMessage {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });
                for tc in &tool_calls {
                    eprintln!("  {} {} {}", "▶", tc.function.name.bold(), tc.function.arguments.dimmed());
                    let tool_request = crate::commands::tools::ToolRequest {
                        tool: tc.function.name.clone(),
                        args: serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Null),
                    };
                    let diff_path = if matches!(tc.function.name.as_str(), "write" | "edit") {
                        tool_request.args.get("filePath").and_then(|v| v.as_str()).map(|p| p.to_string())
                    } else {
                        None
                    };
                    let old = diff_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
                    match check_permission(&request.permission_mode, &tc.function.name, &tc.function.arguments).await {
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
                    let result = crate::commands::tools::execute_tool_inner(state, tool_request).await?;
                    if let Some(ref path) = diff_path {
                        let new = std::fs::read_to_string(path).unwrap_or_default();
                        show_diff(path, &old, &new);
                    }
                    messages.push(providers::ChatMessage {
                        role: "tool".into(),
                        content: result.output,
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }
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

pub fn list_models(config: &providers::ProviderConfig) -> Result<Vec<String>, String> {
    log::info!("list_models for provider={:?}", config.kind);
    match config.kind {
        providers::ProviderKind::OpenAI => Ok(vec![
            "gpt-4o".into(), "gpt-4o-mini".into(), "gpt-4-turbo".into(), "gpt-3.5-turbo".into(),
        ]),
        providers::ProviderKind::Anthropic => Ok(vec![
            "claude-3-5-sonnet-20241022".into(), "claude-3-5-haiku-20241022".into(),
            "claude-opus-4-20250514".into(),
        ]),
        providers::ProviderKind::Groq => Ok(vec![
            "llama-3.3-70b-versatile".into(), "mixtral-8x7b-32768".into(),
        ]),
        providers::ProviderKind::XAI => Ok(vec![
            "grok-3".into(), "grok-3-mini".into(),
        ]),
        providers::ProviderKind::Local => Ok(vec!["ollama".into()]),
        _ => Ok(vec!["unknown".into()]),
    }
}
