//! Lightweight snapshot for subagent context-fork staleness detection.
//!
//! Holds an immutable structural view of the parent's message list at fork
//! time (message count, token count, and an optional generation counter).
//! This lets a long-lived subagent detect when the parent context has grown
//! or been compacted past the fork point — without holding a second full
//! message-list clone.
//!
//! The companion [`ContextDelta`] is a borrowed view used at spawn time to
//! carry the parent's message slice into the subagent loop.

use providers::ChatMessage;

/// Immutable metadata captured from the parent context at fork time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSnapshot {
    /// Number of messages in the parent at fork time.
    pub fork_point_len: usize,
    /// Approximate token count of the parent context at fork time.
    pub token_count: u32,
    /// Parent context generation counter at fork time.
    /// Bumped on the parent after every message append.
    pub generation: u64,
}

impl ContextSnapshot {
    pub fn new(messages: &[ChatMessage], generation: u64) -> Self {
        Self {
            fork_point_len: messages.len(),
            token_count: 0, // caller fills via `token_counter` if available
            generation,
        }
    }

    pub fn with_token_count(mut self, token_count: u32) -> Self {
        self.token_count = token_count;
        self
    }

    /// How many messages the parent has added since this snapshot.
    pub fn delta_len(&self, parent_current_len: usize) -> usize {
        parent_current_len.saturating_sub(self.fork_point_len)
    }

    /// True if the parent has advanced beyond the snapshot generation.
    pub fn is_stale(&self, parent_generation: u64) -> bool {
        parent_generation > self.generation
    }
}

/// Incremental view of the parent's context at fork time.
///
/// Borrowed (no clone) slice + a snapshot for staleness checks. The subagent
/// materialises an owned copy only when it needs to mutate.
#[derive(Debug, Clone)]
pub struct ContextDelta<'a> {
    /// Borrowed prefix of the parent's conversation up to the fork boundary.
    pub messages: &'a [ChatMessage],
    /// Structural metadata captured from the parent at fork time.
    pub snapshot: ContextSnapshot,
}

impl<'a> ContextDelta<'a> {
    pub fn new(messages: &'a [ChatMessage], snapshot: ContextSnapshot) -> Self {
        Self { messages, snapshot }
    }

    /// Full fork: borrow the entire parent message list.
    pub fn full(messages: &'a [ChatMessage], generation: u64) -> Self {
        let snapshot = ContextSnapshot::new(messages, generation);
        Self::new(messages, snapshot)
    }

    /// Materialise an owned copy of the inherited messages.
    pub fn to_messages(&self) -> Vec<ChatMessage> {
        self.messages.to_vec()
    }
}
