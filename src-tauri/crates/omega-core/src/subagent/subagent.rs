//! Subagent core — fork from parent, run isolated loop, return condensed result.
//!
//! The subagent forks the parent's conversation context, swaps in a subagent
//! system prompt, runs an isolated agent loop (sequential tool calls, max-turns
//! bounded), and returns a structured summary per ticket 03's template.
//!
//! See `plans/p2-subagent-delegation/map.md` for design decisions.

use crate::AppState;
use crate::ChatEmitter;
use crate::MutexExt;

use super::config::SubagentConfig;
use super::result::{RunOutcome, SubagentResult};
use crate::context_manager::context_delta::{ContextDelta, ContextSnapshot};

use providers::{ChatMessage, ChatRequest, LlmProvider, ProviderConfig, ToolCall, ToolDefinition};

/// A subagent running with its own isolated context window.
///
/// Fields `parent_id` / `parent_session` are reserved for event-log integration
/// (fork tracking) in a future iteration.
#[allow(dead_code)]
pub struct Subagent {
    config: SubagentConfig,
    parent_id: String,
    parent_session: String,
}
impl Subagent {
    /// Create a new subagent from config and parent session context.
    pub fn new(config: SubagentConfig, parent_id: &str, parent_session: &str) -> Self {
        Self {
            config,
            parent_id: parent_id.to_string(),
            parent_session: parent_session.to_string(),
        }
    }

    /// Fork parent context according to the fork mode, returning an incremental
    /// view (`ContextDelta`) rather than an owned clone.
    ///
    /// Per ticket 01's resolution: Full fork borrows the entire parent
    /// context (all messages up to the delegation point). The system prompt
    /// is swapped later in `run()`.
    ///
    /// The `token_count` snapshot is derived from the parent's token
    /// counter if available; callers may pass 0 when it is unknown.
    pub fn fork_from_parent(
        parent_messages: &[ChatMessage],
        parent_token_count: u32,
        parent_generation: u64,
    ) -> ContextDelta<'_> {
        let snapshot = ContextSnapshot {
            fork_point_len: parent_messages.len(),
            token_count: parent_token_count,
            generation: parent_generation,
        };
        match &parent_messages[..] {
            [] => ContextDelta::new(&[], snapshot),
            [system, rest @ ..] if system.role == "system" => {
                // Keep the original system prompt — it's replaced in run()
                // Borrow the full window: [system, rest...]
                ContextDelta::new(parent_messages, snapshot)
            }
            _ => ContextDelta::new(parent_messages, snapshot),
        }
    }

    /// Build the subagent system prompt (swapped from parent's).
    ///
    /// Includes: task framing, deliverable description, tool whitelist,
    /// inherited negative-knowledge rules, and the condensed result contract.
    pub fn system_prompt(config: &SubagentConfig) -> String {
        let tools = if config.tool_whitelist.is_empty() {
            "read-only".to_string()
        } else {
            config.tool_whitelist.join(", ")
        };
        format!(
            "You are a subagent worker.\n\n\
             Task: {}\n\n\
             Deliverable: {}\n\n\
             Tools: {}\n\n\
             Work one-shot within max_turns={}. Return a structured summary of your findings.\
             If you complete the task, respond with 'DONE: <your summary here>'.",
            config.task, config.deliverable, tools, config.max_turns
        )
    }

    /// Run the subagent loop and return a structured result.
    ///
    /// # Arguments
    /// - `context`: the forked parent context as an incremental `ContextDelta`
    /// - `state`: the shared AppState (for tool execution, memory, rules)
    /// - `provider`: the LLM provider to use for the subagent
    /// - `provider_config`: provider config (model, max_tokens, temperature)
    /// - `tools`: tool definitions available to the subagent (filtered by whitelist)
    /// - `emitter`: chat emitter for progress/logging (can be NoopEmitter for headless)
    pub async fn run<E: ChatEmitter + ?Sized>(
        &self,
        context: ContextDelta<'_>,
        state: &AppState,
        provider: &dyn LlmProvider,
        provider_config: &ProviderConfig,
        tools: Vec<ToolDefinition>,
        _emitter: &E,
    ) -> Result<SubagentResult, String> {
        // Materialise the forked context from the delta.
        let mut messages = context.to_messages();

        // Swap the system prompt: remove parent's system prompt, insert subagent's.
        // Per ticket 01 resolution: Full fork + swapped system.
        if let Some(first) = messages.first() {
            if first.role == "system" {
                messages[0] = ChatMessage {
                    role: "system".to_string(),
                    content: Self::system_prompt(&self.config),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                };
            } else {
                messages.insert(0, ChatMessage {
                    role: "system".to_string(),
                    content: Self::system_prompt(&self.config),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
        }

        // Append task as bare user message
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: self.config.task.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // Filter tools by whitelist (task-driven, per ticket 06)
        let available_tools: Vec<ToolDefinition> = if self.config.tool_whitelist.is_empty() {
            vec![]
        } else {
            tools
                .into_iter()
                .filter(|t| {
                    // Extract the function name from the ToolDefinition
                    self.config.tool_whitelist.contains(&t.function.name)
                })
                .collect()
        };

        // Build the tool executor — delegates to the parent's tool pipeline
        let executor = |tc: &ToolCall| {
            let state = state;
            let tc = tc.clone();
            async move {
                let args = serde_json::from_str(&tc.function.arguments)
                    .map_err(|e| format!("Failed to parse arguments for `{}`: {}", tc.function.name, e))?;
                let tool_request = crate::commands::tools::ToolRequest {
                    tool: tc.function.name.clone(),
                    args,
                };
                let result = crate::commands::tools::execute_tool_inner(state, tool_request).await?;
                Ok::<String, String>(result.output)
            }
        };

        // Run the loop
        let mut turns: u32 = 0;
        let mut files_changed: Vec<String> = Vec::new();
        let window = provider_config.kind.context_window();
        let context_manager = crate::context_manager::ContextManager::new(
            std::env::current_dir().unwrap_or_default(),
            window,
            &provider_config.model,
            6,
        );

        let max_loops = self.config.max_turns as u32 * 2; // each tool round = 1 turn + 1 provider call

        loop {
            // Check turn budget
            if turns >= max_loops {
                return Ok(SubagentResult {
                    summary: Some(self.build_summary(&messages, &files_changed, None)),
                    outcome: RunOutcome::MaxTurns,
                    gate_score: None,
                    files_changed,
                });
            }
            turns += 1;

            // P1: assemble context before each provider call
            let mut assembled_messages = messages.clone();
            {
                let store = state.memory_store.lock_guard();
                let user_msg = self.config.task.clone();
                let _assembled = context_manager.prepare(&mut assembled_messages, Some(&*store), &user_msg)
                    .map_err(|e| format!("subagent context preparation failed: {}", e))?;
                // Use the assembled context (repo-map + memory injected, compaction applied)
                std::mem::swap(&mut assembled_messages, &mut messages);
            }

            // Check token budget (P2 ticket 04 — enforced by P1's ContextManager)
            // If the repo-map + memory + history exceeds budget, we get compacted
            // automatically. The token_budget is a delta cap from the fork point.

            // Call provider
            let chat_request = ChatRequest {
                messages: messages.clone(),
                config: provider_config.clone(),
                stream: false,
                tools: if available_tools.is_empty() { None } else { Some(available_tools.clone()) },
            };

            let response = provider.chat(chat_request).await?;

            if let Some(tool_calls) = &response.tool_calls {
                // Add assistant message with tool calls
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: response.content.clone(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                // Execute each tool call
                for tc in tool_calls {
                    let result = executor(tc).await;
                    match result {
                        Ok(output) => {
                            // Track file-changing tools for the summary
                            if matches!(tc.function.name.as_str(), "write" | "edit" | "git_commit") {
                                files_changed.push(tc.function.name.clone());
                            }

                            messages.push(ChatMessage {
                                role: "tool".into(),
                                content: output,
                                tool_calls: None,
                                tool_call_id: Some(tc.id.clone()),
                                name: Some(tc.function.name.clone()),
                            });
                        }
                        Err(e) => {
                            messages.push(ChatMessage {
                                role: "tool".into(),
                                content: format!("Tool `{}` failed: {}", tc.function.name, e),
                                tool_calls: None,
                                tool_call_id: Some(tc.id.clone()),
                                name: Some(tc.function.name.clone()),
                            });
                        }
                    }
                }

                continue;
            }

            // No tool calls — this is the final response
            let final_response = response.content;

            return Ok(SubagentResult {
                summary: Some(self.build_summary(&messages, &files_changed, Some(&final_response))),
                outcome: RunOutcome::Completed,
                gate_score: None,
                files_changed,
            });
        }
    }

    /// Build the structured summary per ticket 03's template.
    ///
    /// Two-part: prose (final assistant message, ≤ 3 sentences, only if present)
    /// + always-rendered structured line.
    fn build_summary(
        &self,
        _messages: &[ChatMessage],
        files_changed: &[String],
        final_response: Option<&str>,
    ) -> String {
        let mut prose = String::new();

        if let Some(resp) = final_response {
            // Extract content before any "DONE:" marker, limited to 3 sentences
            let clean = resp.strip_prefix("DONE:").unwrap_or(resp);
            prose = clean.trim().to_string();
        }

        let outcome_line = SubagentResult {
            summary: None,
            outcome: RunOutcome::Completed,
            gate_score: None,
            files_changed: files_changed.to_vec(),
        }
        .structured_line();

        if prose.is_empty() {
            outcome_line
        } else {
            format!("{}\n\n{}", prose, outcome_line)
        }
    }
}

/// Spawn a subagent from parent context.
///
/// The subagent forks the parent's context (via [`ContextDelta`/ContextDelta]),
/// runs an isolated loop using the provided provider and tool definitions,
/// and returns a structured summary.
///
/// [`ContextDelta`]: crate::context_manager::context_delta::ContextDelta
pub async fn spawn_subagent<E: ChatEmitter + ?Sized>(
    config: SubagentConfig,
    parent_messages: &[ChatMessage],
    parent_token_count: u32,
    parent_generation: u64,
    parent_id: &str,
    parent_session: &str,
    state: &AppState,
    provider: &dyn LlmProvider,
    provider_config: &ProviderConfig,
    tools: Vec<ToolDefinition>,
    emitter: &E,
) -> Result<SubagentResult, String> {
    let delta = Subagent::fork_from_parent(parent_messages, parent_token_count, parent_generation);
    let subagent = Subagent::new(config, parent_id, parent_session);
    subagent
        .run(delta, state, provider, provider_config, tools, emitter)
        .await
}
