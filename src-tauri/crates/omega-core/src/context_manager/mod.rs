//! ContextManager — orchestrates the P1 context-engineering strategies.
//!
//! Replaces `context.rs`'s naive `estimate_tokens` + keep-last-N `compact`
//! with: real token counting ([`token_counter`]), a graph-ranked repo-map
//! ([`harness::repomap`]), structured summarization ([`compaction`]), JIT
//! Hermes memory ([`jit_retrieval`]), and per-section budgets ([`budget`]).

pub mod budget;
pub mod compaction;
pub mod jit_retrieval;
pub mod token_counter;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use budget::TokenBudget;
use compaction::CompactionSummary;
use harness::repomap::RepoMap;
use providers::ChatMessage;
use token_counter::TokenCounter;

/// Assembled context ready for the provider call.
#[derive(Debug, Clone)]
pub struct AssembledContext {
    pub messages: Vec<ChatMessage>,
    pub repo_map: String,
    pub memory: String,
    pub compacted: Option<CompactionSummary>,
    pub total_tokens: u32,
}

/// The multi-strategy context manager.
///
/// Build once per session (or per workspace); [`ContextManager::prepare`]
/// runs before each provider call.
pub struct ContextManager {
    /// Token counter for the active provider.
    pub token_counter: Box<dyn TokenCounter>,
    /// Per-section budget for the active provider's window.
    pub budget: TokenBudget,
    /// Repo-map over the workspace root.
    repo_map: Mutex<RepoMap>,
    /// Workspace root used to build the repo-map.
    workspace_root: PathBuf,
    /// JIT memory retriever.
    jit: jit_retrieval::JitRetriever,
    /// Number of verbatim turns preserved across compaction.
    keep_last_turns: usize,
    /// Whether the repo-map has been built for this workspace.
    repo_map_built: Mutex<bool>,
}

impl ContextManager {
    /// Create a manager for `workspace_root`, a provider window, and a model
    /// hint (used to pick the token counter).
    pub fn new(
        workspace_root: impl AsRef<Path>,
        window: u64,
        model_hint: &str,
        keep_last_turns: usize,
    ) -> Self {
        let budget = TokenBudget::from_window(window);
        Self {
            token_counter: token_counter::resolve(model_hint),
            budget,
            repo_map: Mutex::new(RepoMap::new()),
            workspace_root: workspace_root.as_ref().to_path_buf(),
            jit: jit_retrieval::JitRetriever::default(),
            keep_last_turns,
            repo_map_built: Mutex::new(false),
        }
    }

    /// Build (or refresh) the repo-map over the workspace root.
    pub fn ensure_repo_map(&self) -> Result<(), String> {
        let mut built = self.repo_map_built.lock().unwrap();
        if *built {
            return Ok(());
        }
        let mut map = self.repo_map.lock().unwrap();
        let count = map.index_repo(&self.workspace_root)?;
        log::debug!("repo-map indexed {} files under {:?}", count, self.workspace_root);
        *built = true;
        Ok(())
    }

    /// Render the repo-map within its budget allocation.
    pub fn render_repo_map(&self) -> String {
        match self.repo_map.lock() {
            Ok(map) => map.render(self.budget.repo_map),
            Err(_) => String::new(),
        }
    }

    /// Retrieve JIT memory for `user_message`.
    pub fn retrieve_memory(&self, store: &memory::MemoryStore, user_message: &str) -> String {
        self.jit.retrieve(store, user_message)
    }

    /// Total tokens for the current message list plus injected sections.
    pub fn count_tokens(&self, messages: &[ChatMessage], extra: &[&str]) -> u32 {
        let mut total = self.token_counter.count_messages(messages);
        for section in extra {
            total += self.token_counter.count_text(section);
        }
        total
    }

    /// Compaction trigger threshold for the current budget.
    pub fn trigger_threshold(&self) -> u32 {
        self.budget.trigger_threshold()
    }

    /// Compact `messages`, preserving system prompt + last N turns verbatim
    /// and summarizing everything older.
    pub fn compact(&self, messages: Vec<ChatMessage>) -> (Vec<ChatMessage>, CompactionSummary) {
        compaction::compact(messages, self.keep_last_turns)
    }

    /// Prepare messages for a provider call.
    ///
    /// 1. Ensures the repo-map is built and injects its rendered form as a
    ///    system message after the base system prompt.
    /// 2. Injects JIT Hermes memory (if any) as a system message.
    /// 3. Counts total tokens; compacts when over the budget trigger.
    ///
    /// `store` may be `None` to skip memory injection (headless paths).
    pub fn prepare(
        &self,
        messages: &mut Vec<ChatMessage>,
        store: Option<&memory::MemoryStore>,
        user_message: &str,
    ) -> Result<AssembledContext, String> {
        self.ensure_repo_map()?;
        let repo_map_str = self.render_repo_map();
        let memory_str = store
            .map(|s| self.retrieve_memory(s, user_message))
            .unwrap_or_default();

        // Idempotent injection: skip a section already present from an
        // earlier turn in the same session.
        let has_repo_map = messages
            .iter()
            .any(|m| m.role == "system" && m.content.contains("# Repo map"));
        let has_memory = messages
            .iter()
            .any(|m| m.role == "system" && m.content.contains("<retrieved_context>"));

        if !repo_map_str.is_empty() && !has_repo_map {
            insert_system_after_base(messages, &repo_map_str);
        }
        if !memory_str.is_empty() && !has_memory {
            insert_system_after_base(messages, &memory_str);
        }

        let total = self.count_tokens(messages, &[repo_map_str.as_str(), memory_str.as_str()]);
        let mut compacted = None;
        if total > self.trigger_threshold() {
            let (compacted_msgs, summary) = self.compact(messages.clone());
            if !summary.is_empty() {
                compacted = Some(summary.clone());
                *messages = compacted_msgs;
                log::info!(
                    "context compacted ({} tokens > threshold {}): summary msg inserted",
                    total,
                    self.trigger_threshold()
                );
            }
        }

        let final_total = self.token_counter.count_messages(messages);
        Ok(AssembledContext {
            messages: messages.clone(),
            repo_map: repo_map_str,
            memory: memory_str,
            compacted,
            total_tokens: final_total,
        })
    }
}

/// Insert `content` as a system message immediately after the base system
/// prompt (index 0 when present, else at index 0).
fn insert_system_after_base(messages: &mut Vec<ChatMessage>, content: &str) {
    let insert_idx = if messages.first().map(|m| m.role == "system").unwrap_or(false) {
        1
    } else {
        0
    };
    messages.insert(
        insert_idx,
        ChatMessage {
            role: "system".into(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    #[test]
    fn prepare_injects_repo_map_and_memory() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("a.rs"), "pub fn alpha() {}\n").unwrap();

        let store = memory::MemoryStore::new(":memory:").unwrap();
        store
            .store(memory::MemoryLayer::Project, "ctx_key", "login uses alpha tokens")
            .unwrap();

        let cm = ContextManager::new(temp.path(), 128_000, "gpt-4o", 6);
        let mut messages = vec![msg("system", "base sys"), msg("user", "do alpha login")];
        let assembled = cm
            .prepare(&mut messages, Some(&store), "do alpha login")
            .unwrap();

        assert!(assembled.repo_map.contains("alpha"), "repo map missing alpha");
        assert!(assembled.memory.contains("ctx_key"), "memory missing");
        assert!(assembled.total_tokens > 0);
        // Base system + repo-map + memory + user.
        assert!(messages.len() >= 4);
    }

    #[test]
    fn prepare_without_repo_files_is_ok() {
        let temp = tempfile::tempdir().unwrap();
        let cm = ContextManager::new(temp.path(), 32_000, "claude-3-5-sonnet", 6);
        let mut messages = vec![msg("user", "hi")];
        let assembled = cm.prepare(&mut messages, None, "hi").unwrap();
        assert!(assembled.repo_map.is_empty());
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn compacts_when_over_threshold() {
        let temp = tempfile::tempdir().unwrap();
        let cm = ContextManager::new(temp.path(), 4_096, "gpt-4o-mini", 2);
        let mut messages = vec![msg("system", "sys")];
        for i in 0..60 {
            messages.push(msg(
                "user",
                &format!("long question number {} with plenty of padding text here", i),
            ));
            messages.push(msg(
                "assistant",
                &format!("long answer number {} with plenty of padding text here", i),
            ));
        }
        let assembled = cm.prepare(&mut messages, None, "x").unwrap();
        assert!(
            assembled.compacted.is_some(),
            "expected compaction on a 60-turn fixture against a 4k window"
        );
    }
}
