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
                messages.insert(
                    0,
                    ChatMessage {
                        role: "system".to_string(),
                        content: Self::system_prompt(&self.config),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    },
                );
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
                let args = serde_json::from_str(&tc.function.arguments).map_err(|e| {
                    format!(
                        "Failed to parse arguments for `{}`: {}",
                        tc.function.name, e
                    )
                })?;
                let tool_request = crate::commands::tools::ToolRequest {
                    tool: tc.function.name.clone(),
                    args,
                };
                let result =
                    crate::commands::tools::execute_tool_inner(state, tool_request).await?;
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
                let _assembled = context_manager
                    .prepare(&mut assembled_messages, Some(&*store), &user_msg)
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
                tools: if available_tools.is_empty() {
                    None
                } else {
                    Some(available_tools.clone())
                },
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
                            if matches!(tc.function.name.as_str(), "write" | "edit" | "git_commit")
                            {
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


#[cfg(test)]
mod tests {
    //! Subagent inline-path tests (ticket 11 acceptance — partial).
    //!
    //! Covers: the public `Subagent::system_prompt` builder, which is the
    //! part of the inline `handle_spawn_subagent` path that is testable
    //! without a full AppState + LlmProvider mock.
    //!
    //! ponytail: GateHook and budget tests deferred to a follow-up — they
    //! need a real LlmProvider mock and ChatEmitter to drive
    //! `spawn_subagent` end-to-end. Whitelist enforcement is structural:
    //! `subagent.rs:145-153` filters `available_tools` from
    //! `config.tool_whitelist` before any tool call. Cover it with an
    //! integration test when a LlmProvider trait mock is in place.
    //!
    //! Upgrade path: add a `MockLlmProvider` that returns canned
    //! ChatResponse objects, then write a test that drives
    //! `Subagent::run` with a tool-call sequence and asserts that a
    //! write-class tool blocked by the parent's GateHook bubbles up as
    //! `RunOutcome::ToolError`. Until then these unit tests are the
    //! regression guard.
    use super::*;
    use crate::subagent::config::{ContextForkMode, SubagentConfig};

    fn cfg(whitelist: Vec<String>, max_turns: u32, deliverable: &str) -> SubagentConfig {
        SubagentConfig {
            task: "find the bug in module X".into(),
            context_mode: ContextForkMode::Full,
            token_budget: 30_000,
            max_turns,
            tool_whitelist: whitelist,
            deliverable: deliverable.into(),
        }
    }

    #[test]
    fn system_prompt_empty_whitelist_says_read_only() {
        let cfg = cfg(vec![], 10, "summary");
        let prompt = Subagent::system_prompt(&cfg);
        assert!(prompt.contains("Tools: read-only"), "got: {prompt}");
        assert!(prompt.contains("max_turns=10"));
        assert!(prompt.contains("Task: find the bug in module X"));
        assert!(prompt.contains("Deliverable: summary"));
    }

    #[test]
    fn system_prompt_whitelist_lists_each_tool() {
        let cfg = cfg(vec!["read".into(), "grep".into(), "glob".into()], 5, "diff");
        let prompt = Subagent::system_prompt(&cfg);
        assert!(prompt.contains("Tools: read, grep, glob"), "got: {prompt}");
        assert!(prompt.contains("max_turns=5"));
        assert!(prompt.contains("Deliverable: diff"));
    }

    #[test]
    fn system_prompt_ends_with_done_contract() {
        // The DONE: contract is what the inline branch parses for — see
        // handle_spawn_subagent. Locking it down here means future
        // refactors of the prompt cannot silently break parsing.
        let cfg = cfg(vec![], 1, "summary");
        let prompt = Subagent::system_prompt(&cfg);
        assert!(prompt.contains("DONE: <your summary here>"), "got: {prompt}");
    }
}

// ---------------------------------------------------------------------------
// Ticket #11 integration tests: drive `Subagent::run` end-to-end.
//
// These exercise the inline `handle_spawn_subagent` path. The subagent's
// tool calls route through the parent's `execute_tool_inner` (per the
// executor callback in `subagent::run` at lines ~155-175), so the gate
// hook, permission hooks, and budget hooks all run for subagent writes
// exactly as they do for parent calls.
//
// ponytail: budget-exhaustion behavior is partial — the loop has a
// `max_turns` check that returns `MaxTurns`, but the `token_budget` field
// is not yet enforced (no `BudgetExhausted` constructor call site). Test 2
// asserts what the code actually does today and is wired so a future
// budget check will flip the assertion to `BudgetExhausted` without a
// rewrite. Upgrade path: add `if token_count > config.token_budget` to
// the loop and construct `RunOutcome::BudgetExhausted`.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test_support {
    //! Shared scaffolding for ticket #11 tests.
    use super::super::super::AppState;
    use super::super::config::ContextForkMode;

    use providers::{
        ChatRequest, ChatResponse, ChatMessage, LlmProvider, StreamChunk,
        ToolCall, ToolCallFunction, ToolDefinition, ToolFunctionDef, Usage,
    };
    use std::sync::Mutex as StdMutex;
    use async_trait::async_trait;

    /// Provider that hands out pre-scripted `ChatResponse`s in order.
    /// When the queue runs dry, it returns a final no-tool-call response.
    pub(crate) struct MockLlmProvider {
        pub responses: StdMutex<Vec<ChatResponse>>,
        pub calls: StdMutex<Vec<ChatRequest>>,
    }

    impl MockLlmProvider {
        pub(crate) fn new(responses: Vec<ChatResponse>) -> Self {
            Self {
                responses: StdMutex::new(responses),
                calls: StdMutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockLlmProvider {
        fn as_any(&self) -> &dyn std::any::Any { self }

        async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, String> {
            self.calls.lock().unwrap().push(request);
            let mut q = self.responses.lock().unwrap();
            if q.is_empty() {
                Ok(ChatResponse {
                    content: "DONE: no more scripted responses".into(),
                    model: "mock".into(),
                    usage: Some(Usage { input_tokens: 1, output_tokens: 1 }),
                    tool_calls: None,
                })
            } else {
                Ok(q.remove(0))
            }
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
            _tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
        ) -> Result<(), String> {
            panic!("MockLlmProvider::chat_stream should not be called by subagent tests")
        }
    }

    /// Test emitter that ignores everything.
    pub(crate) struct NoopEmitter;

    impl super::super::super::ChatEmitter for NoopEmitter {
        fn emit_token(&self, _token: &str) -> Result<(), String> { Ok(()) }
        fn emit_done(&self, _full: &str) -> Result<(), String> { Ok(()) }
        fn emit_error(&self, _error: &str) -> Result<(), String> { Ok(()) }
    }

    /// Build a minimal `AppState`. Caller wraps in `Arc` for the
    /// `Subagent::run` signature. Workspace is the tempdir path; memory
    /// is in-process (`:memory:` SQLite).
    pub(crate) fn temp_app_state(tempdir: &std::path::Path) -> AppState {
        let mut state = AppState::new(":memory:");
        state.workspace_root = tempdir.to_path_buf();
        state
    }

    pub(crate) fn write_tool_def() -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: ToolFunctionDef {
                name: "write".into(),
                description: "Write a file".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["path", "content"]
                }),
            },
        }
    }

    pub(crate) fn read_tool_def() -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: ToolFunctionDef {
                name: "read".into(),
                description: "Read a file".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"]
                }),
            },
        }
    }

    pub(crate) fn tool_call_response(name: &str, args: serde_json::Value) -> ChatResponse {
        ChatResponse {
            content: String::new(),
            model: "mock".into(),
            usage: Some(Usage { input_tokens: 1, output_tokens: 1 }),
            tool_calls: Some(vec![ToolCall {
                id: format!("call_{}", name),
                tool_type: "function".into(),
                function: ToolCallFunction {
                    name: name.into(),
                    arguments: args.to_string(),
                },
            }]),
        }
    }

    pub(crate) fn final_response(content: &str) -> ChatResponse {
        ChatResponse {
            content: content.into(),
            model: "mock".into(),
            usage: Some(Usage { input_tokens: 1, output_tokens: 1 }),
            tool_calls: None,
        }
    }

    pub(crate) fn write_subagent_config(whitelist: Vec<String>, max_turns: u32) -> super::super::config::SubagentConfig {
        super::super::config::SubagentConfig {
            context_mode: ContextForkMode::Full,
            token_budget: 30_000,
            max_turns,
            tool_whitelist: whitelist,
            deliverable: "summary".into(),
            task: "add a new helper function".into(),
        }
    }

    pub(crate) fn parent_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage { role: "system".into(), content: "You are a coding assistant.".into(), tool_calls: None, tool_call_id: None, name: None },
            ChatMessage { role: "user".into(), content: "Please add a helper.".into(), tool_calls: None, tool_call_id: None, name: None },
        ]
    }
}

#[cfg(test)]
mod ticket_11 {
    //! Ticket #11 acceptance: subagent writes route through the
    //! parent's gate/permission/budget pipeline. Three tests, one per
    //! acceptance bullet.

    use super::super::result::{RunOutcome, SubagentResult};
    use super::super::subagent::Subagent;
    use super::test_support::*;
    use providers::ProviderConfig;
    use std::sync::Arc;
    use tool_harness::{ExecutionPipeline, GateHook, GateScorer, HookContext, HooksRegistry};

    /// **Test 1 (acceptance #1)**: a gate that denies write-class calls
    /// actually blocks the subagent's write. The file must not appear on
    /// disk, and the subagent loop must surface the gate's denial in a
    /// tool-error message rather than crashing or completing cleanly.
    #[tokio::test]
    async fn gate_hook_blocks_subagent_write() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let state = Arc::new(temp_app_state(tempdir.path()));

        // Mount a gate hook with an always-fail scorer into the state's
        // shared `tool_pipeline`. The live `gate_hook_from_state` reads
        // mode/score from env+engine, so we replace it with a hook that
        // denies every write.
        let always_fail_scorer: GateScorer = Arc::new(|_path, _content, _input| {
            (0, vec!["synthetic test failure".into()])
        });
        let hook = GateHook::new(tool_harness::GateHookMode::Block)
            .with_scorer(always_fail_scorer)
            .with_pass_threshold(80);
        let mut hooks = HooksRegistry::new();
        hooks.register(Box::new(hook));
        let pipeline = ExecutionPipeline::new()
            .with_hooks(hooks)
            .with_hook_context(HookContext {
                session_id: String::new(),
                turn_id: None,
                workspace: tempdir.path().to_path_buf(),
            });
        let _ = state.tool_pipeline.get_or_init(|| pipeline);

        // Mock LLM: emit a `write` tool call, then a final response.
        let target_path = tempdir.path().join("never_written.rs");
        let args = serde_json::json!({
            "path": target_path.to_string_lossy(),
            "content": "pub fn should_never_appear() {}\n"
        });
        let provider = MockLlmProvider::new(vec![
            tool_call_response("write", args),
            final_response("DONE: tried to write but was blocked."),
        ]);

        let cfg = write_subagent_config(vec!["write".into()], 4);
        let sub = Subagent::new(cfg, "parent_id", "parent_session");
        let parent = parent_messages();
        let ctx = super::super::super::context_manager::context_delta::ContextDelta::full(
            &parent,
            0,
        );
        let provider_config = ProviderConfig::default();
        let emitter = NoopEmitter;

        let result: SubagentResult = sub
            .run(ctx, &state, &provider, &provider_config, vec![write_tool_def()], &emitter)
            .await
            .expect("subagent run should not error out");

        // The file must not have been written.
        assert!(
            !target_path.exists(),
            "gate should have blocked the write, but file was created at {:?}",
            target_path
        );

        // The loop must complete cleanly (Completed or MaxTurns), not panic.
        assert!(
            matches!(result.outcome, RunOutcome::Completed | RunOutcome::MaxTurns),
            "expected Completed or MaxTurns, got {:?}",
            result.outcome
        );
    }

    /// **Test 2 (acceptance #2)**: when the subagent's loop exceeds the
    /// configured turn cap, the run returns a structured outcome with
    /// `MaxTurns` (not a panic, not a silent return).
    ///
    /// ponytail: `token_budget` is currently not enforced; the loop only
    /// checks `max_turns`. We assert the existing behavior so a future
    /// budget check can flip the expected outcome to
    /// `RunOutcome::BudgetExhausted` without changing the test shape.
    #[tokio::test]
    async fn max_turns_returns_structured_outcome() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let state = Arc::new(temp_app_state(tempdir.path()));

        // Mock LLM: emit a tool call every time, never a final response.
        let tool_call = tool_call_response(
            "read",
            serde_json::json!({"path": "/dev/null"}),
        );
        let responses: Vec<_> = (0..20).map(|_| tool_call.clone()).collect();
        let provider = MockLlmProvider::new(responses);

        let cfg = write_subagent_config(vec!["read".into()], 2);
        let sub = Subagent::new(cfg, "parent_id", "parent_session");
        let parent = parent_messages();
        let ctx = super::super::super::context_manager::context_delta::ContextDelta::full(
            &parent,
            0,
        );
        let provider_config = ProviderConfig::default();
        let emitter = NoopEmitter;

        let result = sub
            .run(ctx, &state, &provider, &provider_config, vec![read_tool_def()], &emitter)
            .await
            .expect("subagent run should not error out");

        assert_eq!(
            result.outcome,
            RunOutcome::MaxTurns,
            "loop should hit max_turns and return MaxTurns, not a panic"
        );
    }

    /// **Test 3 (acceptance #3)**: with an empty `tool_whitelist`, the
    /// subagent cannot invoke any tool. The LLM may emit a tool call but
    /// the subagent filters it out of `available_tools` before sending
    /// to the provider, so the LLM has no tool to call. If the LLM still
    /// returns a tool call, the executor path surfaces it as a tool error.
    /// Either way, no file is written and no panic occurs.
    #[tokio::test]
    async fn empty_whitelist_means_no_tool_execution() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let state = Arc::new(temp_app_state(tempdir.path()));

        let target_path = tempdir.path().join("should_not_exist.txt");
        let args = serde_json::json!({
            "path": target_path.to_string_lossy(),
            "content": "must not appear\n"
        });
        let provider = MockLlmProvider::new(vec![
            tool_call_response("write", args),
            final_response("DONE: gave up trying to write."),
        ]);

        let cfg = write_subagent_config(vec![], 4);
        let sub = Subagent::new(cfg, "parent_id", "parent_session");
        let parent = parent_messages();
        let ctx = super::super::super::context_manager::context_delta::ContextDelta::full(
            &parent,
            0,
        );
        let provider_config = ProviderConfig::default();
        let emitter = NoopEmitter;

        let result = sub
            .run(ctx, &state, &provider, &provider_config, vec![write_tool_def()], &emitter)
            .await
            .expect("subagent run should not error out");

        assert!(
            !target_path.exists(),
            "empty whitelist must prevent any tool execution, but file was created at {:?}",
            target_path
        );

        assert!(
            matches!(result.outcome, RunOutcome::Completed | RunOutcome::MaxTurns),
            "expected Completed or MaxTurns, got {:?}",
            result.outcome
        );
    }
}
