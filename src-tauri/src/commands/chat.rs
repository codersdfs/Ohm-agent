use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::ChatEmitter;
use colored::Colorize;

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
    log::info!("send_message: agent={}, content={:?}", request.agent_type, request.content.chars().take(50).collect::<String>());

    let config = request.provider.unwrap_or_else(|| {
        let s = state.provider_config.lock().unwrap();
        s.clone()
    });

    let provider = providers::create_provider(&config)?;
    let tools = crate::commands::tools::tool_definitions();

    let mut messages = vec![
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
}

pub async fn stream_message<E: ChatEmitter>(
    state: &AppState,
    request: StreamMessageRequest,
    emitter: &E,
) -> Result<String, String> {
    log::info!("stream_message: agent={}", request.agent_type);

    let config = request.provider.unwrap_or_else(|| {
        let s = state.provider_config.lock().unwrap();
        s.clone()
    });

    let tools = crate::commands::tools::tool_definitions();

    let mut messages: Vec<providers::ChatMessage> = vec![];
    if let Some(system) = request.system_prompt {
        messages.push(providers::ChatMessage {
            role: "system".into(),
            content: system,
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
                    .template("{spinner:.purple} {msg}")
                    .unwrap(),
            );
            spinner.set_message("Thinking...");
            spinner.enable_steady_tick(std::time::Duration::from_millis(80));

            let mut streaming_text = false;
            let mut tool_call_deltas: Vec<(usize, String, String)> = vec![];

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

                messages = vec![providers::ChatMessage {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                }];
                for tc in &tool_calls {
                    eprintln!("  {} {} {}", "⚡".green(), tc.function.name.bold(), tc.function.arguments.dimmed());
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
                continue;
            }

            emitter.emit_done(&full_response)?;
            return Ok(full_response);
        } else {
            let provider = providers::create_provider(&config)?;

            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner:.purple} {msg}")
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
                messages = vec![providers::ChatMessage {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                }];
                for tc in &tool_calls {
                    eprintln!("  {} {} {}", "⚡".green(), tc.function.name.bold(), tc.function.arguments.dimmed());
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
                continue;
            }

            if !response.content.is_empty() {
                emitter.emit_token(&response.content)?;
                full_response.push_str(&response.content);
            }
            emitter.emit_done(&full_response)?;
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
