use crate::context::{compact, ContextPacker};
use crate::Error;

/// Stream message with history and optional cancel support.
pub async fn stream_message_with_history_cancel<E: ChatEmitter>(
    state: &AppState,
    request: StreamMessageRequest,
    emitter: &E,
    messages: &mut Vec<providers::ChatMessage>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<String, String> {
    log::debug!("stream_message: agent={}", request.agent_type);

    if cancelled(cancel.as_ref()) {
        return Err("cancelled".into());
    }

    let config = request.provider.unwrap_or_else(|| {
        let s = state.provider_config.lock_guard();
        s.clone()
    });

    // Build tool defs once per turn.
    let tools = crate::commands::tools::tool_definitions();
    let max_tool_loops = request.max_tool_loops.unwrap_or(DEFAULT_MAX_TOOL_LOOPS);

    if messages.is_empty() {
        let sys_prompt = request
            .system_prompt
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
    // Durably record the user turn before any provider call so a mid-stream
    // kill still leaves the prompt on disk.
    flush_session(state, messages);

    // Compact messages if needed (70% of model window threshold)
    let packer = ContextPacker::new();
    packer.compact_if_needed(state, messages, emitter, config);

    let provider = std::sync::Arc::new(providers::create_provider(&config)?);
    let mut full_response = String::new();
    let mut max_loops = max_tool_loops;

    loop {
        if cancelled(cancel.as_ref()) {
            let _ = emitter.emit_error("cancelled");
            return Err("cancelled".into());
        }
        if max_loops == 0 {
            return Err(format!(
                "Tool call loop exceeded max iterations ({})",
                max_tool_loops
            ));
        }
        max_loops -= 1;

        if config.kind.supports_streaming() {
            log::debug!(
                "streaming: provider={:?} tools={}",
                config.kind,
                tools.len()
            );
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
                s.set_message("Cooking…");
                s.enable_steady_tick(std::time::Duration::from_millis(80));
                Some(s)
            } else {
                None
            };

            let mut streaming_text = false;
            let mut tool_call_deltas: Vec<(usize, String, String, String)> = vec![];
            let mut last_usage: Option<providers::Usage> = None;

            while let Some(chunk) = rx.recv().await {
                if cancelled(cancel.as_ref()) {
                    if let Some(ref s) = spinner {
                        s.finish_and_clear();
                    }
                    let _ = emitter.emit_error("cancelled");
                    return Err("cancelled".into());
                }
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
                        let pos = tool_call_deltas
                            .iter()
                            .position(|(idx, _, _, _)| *idx == d.index);
                        if let Some(p) = pos {
                            let entry = &mut tool_call_deltas[p];
                            if let Some(ref id_val) = d.id {
                                if entry.1.is_empty() {
                                    entry.1.push_str(id_val);
                                }
                            }
                            if let Some(ref name) =
                                d.function.as_ref().and_then(|f| f.name.as_ref())
                            {
                                if entry.2.is_empty() {
                                    entry.2.push_str(name);
                                }
                            }
                            if let Some(ref args) =
                                d.function.as_ref().and_then(|f| f.arguments.as_ref())
                            {
                                if entry.3.is_empty() {
                                    entry.3.push_str(args);
                                }
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
                            if let Some(ref a) =
                                d.function.as_ref().and_then(|f| f.arguments.as_ref())
                            {
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

            if cancelled(cancel.as_ref()) {
                let _ = emitter.emit_error("cancelled");
                return Err("cancelled".into());
            }

            if !tool_call_deltas.is_empty() {
                log::debug!(
                    "executing {} accumulated tool calls",
                    tool_call_deltas.len()
                );
                let tool_calls: Vec<providers::ToolCall> = tool_call_deltas
                    .iter()
                    .map(|(_index, id, name, args)| providers::ToolCall {
                        id: if id.is_empty() {
                            format!("call_{}", _index)
                        } else {
                            id.clone()
                        },
                        tool_type: "function".into(),
                        function: providers::ToolCallFunction {
                            name: name.clone(),
                            arguments: args.clone(),
                        },
                    })
                    .collect();

                handle_tool_calls(
                    state,
                    &tool_calls,
                    messages,
                    &request.permission_mode,
                    emitter,
                    cancel.as_ref(),
                )
                .await?;
                // Reset text accumulator between tool rounds so final answer is clean.
                full_response.clear();
                continue;
            }

            // Persist final assistant text into conversation history so multi-turn
            // and session resume keep the LLM context complete.
            if !full_response.is_empty() {
                messages.push(providers::ChatMessage {
                    role: "assistant".into(),
                    content: full_response.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
            emitter.emit_done(&full_response)?;
            if let Some(ref u) = last_usage {
                record_cost(u.input_tokens, u.output_tokens);
                if emitter.allows_direct_terminal_output() {
                    eprintln!(
                        "  {}tokens: {} in / {} out{}",
                        DIM, u.input_tokens, u.output_tokens, RESET
                    );
                }
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
                s.set_message("Cooking…");
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

            if cancelled(cancel.as_ref()) {
                let _ = emitter.emit_error("cancelled");
                return Err("cancelled".into());
            }

            if let Some(tool_calls) = response.tool_calls {
                handle_tool_calls(
                    state,
                    &tool_calls,
                    messages,
                    &request.permission_mode,
                    emitter,
                    cancel.as_ref(),
                )
                .await?;
                full_response.clear();
                continue;
            }

            if !response.content.is_empty() {
                emitter.emit_token(&response.content)?;
                full_response.push_str(&response.content);
            }
            if !full_response.is_empty() {
                messages.push(providers::ChatMessage {
                    role: "assistant".into(),
                    content: full_response.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
            emitter.emit_done(&full_response)?;
            if let Some(ref u) = response.usage {
                record_cost(u.input_tokens, u.output_tokens);
                if emitter.allows_direct_terminal_output() {
                    eprintln!(
                        "  {}tokens: {} in / {} out{}",
                        DIM, u.input_tokens, u.output_tokens, RESET
                    );
                }
            }
            return Ok(full_response);
        }
    }
}

impl ContextPacker {
    pub fn new() -> Self {
        Self {
            compacted_this_turn: AtomicBool::new(false),
        }
    }

    pub fn compact_if_needed<E: ChatEmitter>(
        &self,
        state: &AppState,
        messages: &mut Vec<providers::ChatMessage>,
        emitter: &E,
        config: Arc<ProviderConfig>,
    ) {
        // Check if already compacted this turn
        if self.compacted_this_turn.load(Ordering::SeqCst) {
            return;
        }

        let token_count = crate::commands::chat::estimate_tokens(messages);
        let threshold = (config.kind.context_window() as f64 * 0.7) as usize;

        if token_count <= threshold {
            self.compacted_this_turn.store(true, Ordering::SeqCst);
            return;
        }

        let (new_messages, summary) = compact(messages.clone(), 6, config.kind.context_window());
        if !summary.is_empty() {
            // Mark entry as compacted boundary
            if let Some(emitter) = state.emit_notice_emitter() {
                emitter.emit_notice();
            }
            // Add summary as system message
            messages.push(providers::ChatMessage {
                role: "system".into(),
                content: summary.into(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }
        self.compacted_this_turn.store(true, Ordering::SeqCst);
    }
}