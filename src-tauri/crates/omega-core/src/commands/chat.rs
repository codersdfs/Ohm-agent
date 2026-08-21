use crate::ChatEmitter;
use crate::{AppState, MutexExt};
use serde::{Deserialize, Serialize};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::cost_tracker;
use super::diff_display::show_diff;
use super::permission_prompt::{check_permission, NoopEmitter, Permission};

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

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

/// Default max tool-loop iterations for a single user turn.
pub const DEFAULT_MAX_TOOL_LOOPS: u32 = 25;

pub async fn send_message(
    state: &AppState,
    request: SendMessageRequest,
) -> Result<SendMessageResponse, String> {
    log::debug!(
        "send_message: agent={}, content={:?}",
        request.agent_type,
        request.content.chars().take(50).collect::<String>()
    );

    let config = request.provider.unwrap_or_else(|| {
        let s = state.provider_config.lock_guard();
        s.clone()
    });

    let provider = providers::create_provider(&config)?;
    // Build tool defs once per turn (not per loop).
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

    let mut max_loops = DEFAULT_MAX_TOOL_LOOPS;

    loop {
        if max_loops == 0 {
            return Err(format!(
                "Tool call loop exceeded max iterations ({})",
                DEFAULT_MAX_TOOL_LOOPS
            ));
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
            // Shared handle_tool_calls: permission, diff, emitter hooks.
            handle_tool_calls(state, &tool_calls, &mut messages, "off", &NoopEmitter, None, provider.as_ref(), &tools).await?;
        } else {
            return Ok(SendMessageResponse {
                message_id: uuid::Uuid::new_v4().to_string(),
                content: response.content,
                agent_type: request.agent_type,
            });
        }
    }
}

fn cancelled(flag: Option<&Arc<AtomicBool>>) -> bool {
    flag.map(|f| f.load(Ordering::SeqCst)).unwrap_or(false)
}

/// Persist conversation snapshot if AppState has a session store attached.
/// Failures are logged but never abort the turn (disk errors shouldn't kill chat).
fn flush_session(state: &AppState, messages: &[providers::ChatMessage]) {
    if let Err(e) = state.persist_session(messages) {
        log::warn!("session persist failed: {e}");
    }

}

/// Handle a `spawn_subagent` tool call by forking the parent's conversation
/// context, running an isolated subagent loop, and returning the structured
/// summary.
///
/// This intercepts the tool before `execute_tool_inner` since the subagent
/// needs access to the provider and current conversation state.
async fn handle_spawn_subagent<E: ChatEmitter>(
    state: &AppState,
    args_json: &str,
    messages: &Vec<providers::ChatMessage>,
    provider: &dyn providers::LlmProvider,
    tools: &[providers::ToolDefinition],
    emitter: &E,
) -> Result<String, String> {
    use crate::subagent::{SubagentConfig, spawn_subagent};
    use serde_json::Value;

    let args: Value = serde_json::from_str(args_json)
        .map_err(|e| format!("Failed to parse spawn_subagent arguments: {}", e))?;

    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required argument: task".to_string())?;

    let token_budget = args
        .get("token_budget")
        .and_then(|v| v.as_u64())
        .unwrap_or(30_000);

    let max_turns = args
        .get("max_turns")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as u32;

    let tool_whitelist: Vec<String> = args
        .get("tool_whitelist")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let deliverable = args
        .get("deliverable")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "condensed summary".to_string());

    // Build the subagent config with Full fork mode (per ticket 01 decision)
    let config = SubagentConfig {
        context_mode: crate::subagent::ContextForkMode::Full,
        token_budget,
        max_turns,
        tool_whitelist,
        deliverable,
        task: task.to_string(),
    };
    let provider_config = state.provider_config.lock_guard().clone();
    let _ = emitter.emit_token(&format!("  Subagent running task: {}…\n", task));

    // Fork parent context and run the subagent loop
    let parent_id = state.session_id().unwrap_or_else(|| "unknown".to_string());
    let parent_session = parent_id.clone();

    let result = spawn_subagent(
        config,
        &messages,
        0, // parent_token_count: defer precise count to ContextManager
        0, // parent_generation: not yet instrumented, pass 0
        &parent_id,
        &parent_session,
        state,
        provider,
        &provider_config,
        tools.to_vec(),
        emitter,
    )
    .await?;

    // Build the two-part summary: prose + structured line
    let line = result.structured_line();
    let summary = result.summary.unwrap_or_default();
    let output = if summary.is_empty() {
        line.clone()
    } else {
        format!("{}\n\n{}", summary, line)
    };

    Ok(output)
}

async fn handle_tool_calls<E: ChatEmitter>(
    state: &AppState,
    tool_calls: &[providers::ToolCall],
    messages: &mut Vec<providers::ChatMessage>,
    permission_mode: &str,
    emitter: &E,
    cancel: Option<&Arc<AtomicBool>>,
    provider: &dyn providers::LlmProvider,
    tools: &[providers::ToolDefinition],
) -> Result<(), String> {
    messages.push(providers::ChatMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: Some(tool_calls.to_vec()),
        tool_call_id: None,
        name: None,
    });

    for tc in tool_calls {
        if cancelled(cancel) {
            let msg = "Cancelled by user before tool execution".to_string();
            emitter.emit_tool_result(&tc.function.name, false, &msg)?;
            messages.push(providers::ChatMessage {
                role: "tool".into(),
                content: msg,
                tool_calls: None,
                tool_call_id: Some(tc.id.clone()),
                name: Some(tc.function.name.clone()),
            });
            // Mark remaining tools as skipped so the message list stays consistent.
            continue;
        }

        emitter.emit_tool_call(&tc.function.name, &tc.function.arguments)?;
        let args = match serde_json::from_str(&tc.function.arguments) {
            Ok(v) => v,
            Err(e) => {
                let err_msg = format!(
                    "Error parsing arguments for `{}`: {}.\nArguments received: {}",
                    tc.function.name, e, tc.function.arguments
                );
                emitter.emit_tool_result(&tc.function.name, false, &err_msg)?;
                messages.push(providers::ChatMessage {
                    role: "tool".into(),
                    content: err_msg,
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
        match check_permission(
            permission_mode,
            &tc.function.name,
            &tc.function.arguments,
            emitter,
        )
        .await
        {
            Permission::Allow => {}
            Permission::Deny => {
                emitter.emit_tool_result(&tc.function.name, false, "denied")?;
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

        // isolated subagent loop. Other tools go through the normal pipeline.
        if tc.function.name == "spawn_subagent" {
            let result = handle_spawn_subagent(
                state,
                &tc.function.arguments,
                messages,
                provider,
                tools,
                emitter,
            )
            .await;
            match result {
                Ok(output) => {
                    emitter.emit_tool_result(&tc.function.name, true, &output)?;
                    messages.push(providers::ChatMessage {
                        role: "tool".into(),
                        content: output,
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }
                Err(e) => {
                    emitter.emit_tool_result(&tc.function.name, false, &e)?;
                    messages.push(providers::ChatMessage {
                        role: "tool".into(),
                        content: format!("spawn_subagent failed: {}", e),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }
            }
            continue;
        }
        // Read old file content for diff (permission already granted)
        let diff_path = if matches!(tc.function.name.as_str(), "write" | "edit") {
            tool_request
                .args
                .get("filePath")
                .and_then(|v| v.as_str())
                .map(|p| p.to_string())
        } else {
            None
        };
        let old = diff_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();

        let result = match crate::commands::tools::execute_tool_inner(state, tool_request).await {
            Ok(r) => r,
            Err(e) => crate::commands::tools::ToolResult::err(e),
        };

        // Show diff after execution (terminal CLI only; TUI shows the bounded
        // diff preview inside ToolExecutionComponent).
        if let Some(ref path) = diff_path {
            let new = std::fs::read_to_string(path).unwrap_or_default();
            show_diff(path, &old, &new, emitter);
        }
        let output = if result.success {
            result.output.clone()
        } else {
            result.error.unwrap_or_default()
        };
        emitter.emit_tool_result(&tc.function.name, result.success, &output)?;
        messages.push(providers::ChatMessage {
            role: "tool".into(),
            content: output,
            tool_calls: None,
            tool_call_id: Some(tc.id.clone()),
            name: Some(tc.function.name.clone()),
        });
    }

    if cancelled(cancel) {
        return Err("cancelled".into());
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
    /// Optional max tool-loop iterations (default: DEFAULT_MAX_TOOL_LOOPS).
    #[serde(default)]
    pub max_tool_loops: Option<u32>,
}

fn default_true() -> bool {
    true
}

/// Canonical interactive agent loop (no cancel flag).
pub async fn stream_message_with_history<E: ChatEmitter>(
    state: &AppState,
    request: StreamMessageRequest,
    emitter: &E,
    messages: &mut Vec<providers::ChatMessage>,
) -> Result<String, String> {
    stream_message_with_history_cancel(state, request, emitter, messages, None).await
}

/// Canonical interactive agent loop with optional cancel flag.
///
/// `cancel` is checked before each provider call, after each stream chunk,
/// and before each tool execution. When set, returns Err("cancelled").
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
    let user_content = request.content.clone();
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

    // P1: assemble context — graph-ranked repo-map + JIT Hermes memory +
    // structured compaction, budgeted per provider window. Replaces the old
    // chars/4 `estimate_tokens` + keep-last-N `compact` block.
    let window = config.kind.context_window();
    {
        let assembled = state
            .assemble_context(
                std::env::current_dir().unwrap_or_default(),
                window,
                &config.model,
                messages,
                &user_content,
            )
            .map_err(|e| format!("context preparation failed: {}", e))?;
        log::debug!(
            "context prepared: {} tokens, repo_map={} chars, memory={} chars, compacted={}",
            assembled.total_tokens,
            assembled.repo_map.len(),
            assembled.memory.len(),
            assembled.compacted.is_some()
        );
    }
    flush_session(state, messages);

    let provider = std::sync::Arc::new(providers::create_provider(&config)?);
    let mut full_response = String::new();
    let mut max_loops = max_tool_loops;
    const MAX_RATE_LIMIT_RETRIES: u32 = 3;

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
            let mut rate_limit_retries = 0u32;
            let stream_handle = tokio::spawn(async move {
                p.chat_stream(chat_request, tx).await
            });
            let spinner = if request.show_progress {
                let s = indicatif::ProgressBar::new_spinner();
                s.set_style(
                    indicatif::ProgressStyle::default_spinner()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                        .template("{spinner} {msg}")
                        .unwrap(),
                );
                s.set_message("Thinking…");
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
                                entry.2.push_str(name);
                            }
                            if let Some(ref args) =
                                d.function.as_ref().and_then(|f| f.arguments.as_ref())
                            {
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

            // The stream task finished (channel closed). Surface its result;
            // retry on rate-limit errors with exponential backoff.
            match stream_handle.await {
                Ok(Err(e)) if e.contains("429") || e.to_lowercase().contains("rate limit") => {
                    if rate_limit_retries >= MAX_RATE_LIMIT_RETRIES {
                        return Err(format!("Rate limited after {} retries: {}", MAX_RATE_LIMIT_RETRIES, e));
                    }
                    rate_limit_retries += 1;
                    let backoff_ms = 1000u64 * (1u64 << rate_limit_retries);
                    log::warn!("Rate limited (attempt {}/{}), retrying in {}ms", rate_limit_retries, MAX_RATE_LIMIT_RETRIES, backoff_ms);
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    continue;
                }
                Ok(Err(e)) => {
                    return Err(format!("Stream error: {}", e));
                }
                Err(e) => {
                    return Err(format!("Stream task failed: {}", e));
                }
                Ok(Ok(())) => {}
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
                    &**provider,
                    &tools,
                )
                .await?;
            // Snapshot after each completed tool round.
                flush_session(state, messages);
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

            // P0-08: Store turn summary in project memory
            if !full_response.is_empty() {
                let tool_names: Vec<String> = messages
                    .iter()
                    .filter(|m| m.role == "assistant")
                    .filter_map(|m| {
                        m.tool_calls.as_ref().and_then(|tcs| tcs.first())
                    })
                    .map(|tc| tc.function.name.clone())
                    .collect();

                let store = state.memory_store.lock_guard();
                if let Err(e) = crate::memory_summarizer::store_turn_summary(
                    &store,
                    &user_content,
                    &full_response,
                    &tool_names,
                ) {
                    log::warn!("failed to store turn summary: {}", e);
                }
            }

            flush_session(state, messages);

            emitter.emit_done(&full_response)?;
            if let Some(ref u) = last_usage {
                cost_tracker::record_cost(u.input_tokens, u.output_tokens);
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
                s.set_message("Thinking…");
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

            let mut rate_limit_retries = 0u32;
            let response = loop {
                match provider.chat(chat_request.clone()).await {
                    Ok(resp) => break resp,
                    Err(e) if e.contains("429") || e.to_lowercase().contains("rate limit") => {
                        if rate_limit_retries >= MAX_RATE_LIMIT_RETRIES {
                            return Err(format!("Rate limited after {} retries: {}", MAX_RATE_LIMIT_RETRIES, e));
                        }
                        rate_limit_retries += 1;
                        let backoff_ms = 1000u64 * (1u64 << rate_limit_retries);
                        log::warn!("Rate limited (attempt {}/{}), retrying in {}ms", rate_limit_retries, MAX_RATE_LIMIT_RETRIES, backoff_ms);
                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    }
                    Err(e) => return Err(e),
                }
            };
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
                    &**provider,
                    &tools,
                )
                .await?;
                flush_session(state, messages);
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
            flush_session(state, messages);
            emitter.emit_done(&full_response)?;
            if let Some(ref u) = response.usage {
                cost_tracker::record_cost(u.input_tokens, u.output_tokens);
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

pub async fn stream_message<E: ChatEmitter>(
    state: &AppState,
    request: StreamMessageRequest,
    emitter: &E,
) -> Result<String, String> {
    let mut messages = Vec::new();
    stream_message_with_history(state, request, emitter, &mut messages).await
}

/// Headless/API convenience: stream with cancel support and fresh history.
pub async fn stream_message_cancel<E: ChatEmitter>(
    state: &AppState,
    request: StreamMessageRequest,
    emitter: &E,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    let mut messages = Vec::new();
    stream_message_with_history_cancel(state, request, emitter, &mut messages, Some(cancel)).await
}

pub async fn list_models(config: &providers::ProviderConfig) -> Vec<String> {
    match providers::fetch_models(config).await {
        Ok(models) => models.into_iter().map(|m| m.id).collect(),
        Err(_) => {
            let fallback: &[&str] = match config.kind {
                providers::ProviderKind::OpenAI => {
                    &["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-3.5-turbo"]
                }
                providers::ProviderKind::Anthropic => &[
                    "claude-3-5-sonnet-20241022",
                    "claude-3-5-haiku-20241022",
                    "claude-opus-4-20250514",
                ],
                providers::ProviderKind::Groq => &["llama-3.3-70b-versatile", "mixtral-8x7b-32768"],
                providers::ProviderKind::XAI => &["grok-3", "grok-3-mini"],
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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct TestEmitter;

    impl ChatEmitter for TestEmitter {
        fn emit_token(&self, _token: &str) -> Result<(), String> {
            Ok(())
        }
        fn emit_done(&self, _full: &str) -> Result<(), String> {
            Ok(())
        }
        fn emit_error(&self, _error: &str) -> Result<(), String> {
            Ok(())
        }
        fn emit_tool_call(&self, _name: &str, _args: &str) -> Result<(), String> {
            Ok(())
        }
        fn emit_tool_result(
            &self,
            _name: &str,
            _success: bool,
            _output: &str,
        ) -> Result<(), String> {
            Ok(())
        }
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

    // Integration test: requires running tool-harness backend and real SSE server.
    // Skipped while the chat loop infrastructure is refactored.
    // Re-enable with #[test] when the mock tool executor is wired.
    #[tokio::test]
    #[ignore]
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
            max_concurrent_tools: 3,
            temperature: 0.0,
        };
        let state = AppState::new_with_provider_config(":memory:", cfg.clone());

        let request = StreamMessageRequest {
            content: "list rust files".into(),
            agent_type: "chat".into(),
            provider: Some(cfg),
            system_prompt: None,
            permission_mode: "off".into(),
            show_progress: false,
            max_tool_loops: Some(5),
        };

        let emitter = TestEmitter;
        let mut messages = Vec::new();
        let result = stream_message_with_history(&state, request, &emitter, &mut messages).await;

        assert!(
            result.is_ok(),
            "stream_message_with_history failed: {:?}",
            result.err()
        );
        assert!(
            !messages.is_empty(),
            "messages buffer should contain history"
        );

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
    #[ignore]
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
            max_concurrent_tools: 3,
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
                show_progress: false,
                max_tool_loops: Some(5),
            },
            &emitter,
            &mut messages,
        )
        .await;
        assert!(r1.is_ok(), "first call failed: {:?}", r1.err());

        let r2 = stream_message_with_history(
            &state,
            StreamMessageRequest {
                content: "second call".into(),
                agent_type: "chat".into(),
                provider: Some(cfg.clone()),
                system_prompt: None,
                permission_mode: "off".into(),
                show_progress: false,
                max_tool_loops: Some(5),
            },
            &emitter,
            &mut messages,
        )
        .await;
        assert!(r2.is_ok(), "second call failed: {:?}", r2.err());

        // grow across calls: system, user, assistant+tool_calls, tool, assistant × 2
        assert!(
            messages.len() > 3,
            "history should grow across calls, got {} messages",
            messages.len()
        );

        drop(cancel_tx);
        handle.await.ok();
    }

    #[tokio::test]
    #[ignore]
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
                                let resp = if idx <= 2 {
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
            max_concurrent_tools: 3,
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
                show_progress: false,
                max_tool_loops: Some(5),
            },
            &emitter,
            &mut messages,
        )
        .await;

        assert!(
            result.is_ok(),
            "should not crash on parse error: {:?}",
            result.err()
        );

        let roles: Vec<&str> = messages.iter().map(|m| m.role.as_str()).collect();
        assert!(
            roles.contains(&"tool"),
            "should have a tool message even with parse error, got: {:?}",
            roles
        );

        drop(cancel_tx);
        handle.await.ok();
    }

    /// Test that cancel before start returns "cancelled".
    /// This does not need a running LLM backend.
    #[tokio::test]
    async fn test_cancel_before_start_returns_cancelled() {
        let cfg = providers::ProviderConfig {
            kind: providers::ProviderKind::Local,
            api_key: None,
            base_url: Some("http://127.0.0.1:1".into()), // will not be hit
            model: "mock".into(),
            max_tokens: 64,
            max_concurrent_tools: 3,
            temperature: 0.0,
        };
        let state = AppState::new_with_provider_config(":memory:", cfg.clone());
        let cancel = Arc::new(AtomicBool::new(true));
        let emitter = TestEmitter;
        let mut messages = Vec::new();
        let result = stream_message_with_history_cancel(
            &state,
            StreamMessageRequest {
                content: "should not run".into(),
                agent_type: "chat".into(),
                provider: Some(cfg),
                system_prompt: Some("sys".into()),
                permission_mode: "off".into(),
                show_progress: false,
                max_tool_loops: Some(1),
            },
            &emitter,
            &mut messages,
            Some(cancel),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "cancelled");
    }
}
