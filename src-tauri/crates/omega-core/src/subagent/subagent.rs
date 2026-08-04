//! Subagent core — fork from parent, run isolated loop, return condensed result.

use super::config::SubagentConfig;
use super::result::{RunOutcome, SubagentResult};
use providers::{ChatMessage, LlmProvider, ProviderConfig, ToolCall};

/// A subagent running with its own isolated context window.
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

    /// Fork parent context according to the fork mode.
    pub fn fork_from_parent(parent_messages: &[ChatMessage]) -> Vec<ChatMessage> {
        match parent_messages.first().and_then(|m| if m.role == "system" { Some(()) } else { None }) {
            _ => parent_messages.to_vec(),
        }
    }

    /// Build the subagent system prompt (swapped from parent's).
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
             Work one-shot. Return a structured summary.",
            config.task, config.deliverable, tools
        )
    }

    /// Run the subagent and return a structured result.
    pub async fn run(
        &self,
        parent_messages: &[ChatMessage],
        _provider: &dyn LlmProvider,
        _provider_config: &ProviderConfig,
        available_tools: &[ToolCall],
    ) -> Result<SubagentResult, String> {
        // Fork context
        let mut messages = Self::fork_from_parent(parent_messages);

        // Prepend subagent system prompt
        messages.insert(0, ChatMessage {
            role: "system".to_string(),
            content: Self::system_prompt(&self.config),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // Append task as bare user message
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: self.config.task.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // Filter tools by whitelist
        let _tools: Vec<ToolCall> = if self.config.tool_whitelist.is_empty() {
            vec![]
        } else {
            available_tools.iter()
                .filter(|t| self.config.tool_whitelist.contains(&t.function.name))
                .cloned()
                .collect()
        };

        // Run the loop
        let mut _turns = 0;
        let files_changed = Vec::new();

        loop {
            if _turns >= self.config.max_turns {
                return Ok(SubagentResult {
                    summary: None,
                    outcome: RunOutcome::MaxTurns,
                    gate_score: None,
                    files_changed,
                });
            }
            _turns += 1;

            // For now, return a placeholder result
            // Full implementation would fork context and run the agent loop
            return Ok(SubagentResult {
                summary: Some(format!("Subagent delegated task: {}", self.config.task)),
                outcome: RunOutcome::Completed,
                gate_score: None,
                files_changed,
            });
        }
    }
}

/// Spawn a subagent from parent context.
pub async fn spawn_subagent(
    config: SubagentConfig,
    parent_messages: &[ChatMessage],
    parent_id: &str,
    parent_session: &str,
    provider: &dyn LlmProvider,
    provider_config: &ProviderConfig,
    available_tools: &[ToolCall],
) -> Result<SubagentResult, String> {
    let subagent = Subagent::new(config, parent_id, parent_session);
    subagent.run(parent_messages, provider, provider_config, available_tools).await
}
