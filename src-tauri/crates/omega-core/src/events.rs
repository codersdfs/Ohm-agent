//! Event-sourced sessions — SessionEvent enum, recover/replay, crash recovery.
//!
//! Sessions are now append-only event logs instead of simple message lists.
//! Events include tool starts/completions, gate checks, context compaction,
//! and checkpoint/fork events. Crash recovery detects orphaned tool calls.

use serde::{Deserialize, Serialize};

/// An event in the session log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// A user message was received.
    UserMessage {
        id: String,
        content: String,
        ts: String,
    },
    /// A provider turn started.
    TurnStarted {
        turn_id: String,
        model: String,
        ts: String,
    },
    /// Provider produced tool calls (projected before execution).
    ToolCallProjected {
        turn_id: String,
        message_id: String,
        tool_calls: Vec<ToolCallInfo>,
    },
    /// A tool started executing.
    ToolStarted {
        call_id: String,
        turn_id: String,
        tool: String,
        input: String,
        ts: String,
    },
    /// A tool completed successfully.
    ToolCompleted {
        call_id: String,
        result: String,
        duration_ms: u64,
    },
    /// A tool was interrupted — ONLY emitted by crash recovery.
    ToolInterrupted {
        call_id: String,
        reason: String,
    },
    /// Gate ran on output (post-write).
    GateCheck {
        turn_id: String,
        score: u32,
        violations: Vec<String>,
    },
    /// Context was compacted.
    ContextCompacted {
        summary: String,
        dropped_count: usize,
        token_count: u32,
    },
    /// Git-based workspace checkpoint.
    Checkpoint {
        checkpoint_id: String,
        git_ref: String,
        description: String,
        ts: String,
    },
    /// Session forked from this point.
    Fork {
        fork_id: String,
        parent_session: String,
        fork_point: String,
        ts: String,
    },
    /// Session ended.
    SessionEnded {
        reason: String,
        turn_count: u32,
        ts: String,
    },
}

/// Tool call information for the projected event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Crash recovery: find ToolStarted without matching ToolCompleted.
///
/// Mirrors OpenCode v2: abandoned side effects are never silently replayed.
pub fn find_orphaned_tools(events: &[SessionEvent]) -> Vec<String> {
    let mut orphaned = Vec::new();
    
    for event in events {
        if let SessionEvent::ToolStarted { call_id, .. } = event {
            let completed = events.iter().any(|e| matches!(
                e,
                SessionEvent::ToolCompleted { call_id: c, .. } if c == call_id
            ));
            if !completed {
                orphaned.push(call_id.clone());
            }
        }
    }
    
    orphaned
}

/// Replay events to reconstruct the conversation history.
pub fn replay_to_messages(events: &[SessionEvent]) -> Vec<providers::ChatMessage> {
    let mut messages = Vec::new();
    
    for event in events {
        match event {
            SessionEvent::UserMessage { content, .. } => {
                messages.push(providers::ChatMessage {
                    role: "user".to_string(),
                    content: content.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
            SessionEvent::ToolCompleted { call_id, result, .. } => {
                messages.push(providers::ChatMessage {
                    role: "tool".to_string(),
                    content: result.clone(),
                    tool_calls: None,
                    tool_call_id: Some(call_id.clone()),
                    name: None,
                });
            }
            _ => {}
        }
    }
    
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_orphaned_tools() {
        let events = vec![
            SessionEvent::ToolStarted {
                call_id: "call_1".to_string(),
                turn_id: "turn_1".to_string(),
                tool: "read".to_string(),
                input: "{}".to_string(),
                ts: "2024-01-01T00:00:00Z".to_string(),
            },
            SessionEvent::ToolCompleted {
                call_id: "call_1".to_string(),
                result: "file contents".to_string(),
                duration_ms: 100,
            },
            SessionEvent::ToolStarted {
                call_id: "call_2".to_string(),
                turn_id: "turn_1".to_string(),
                tool: "write".to_string(),
                input: "{}".to_string(),
                ts: "2024-01-01T00:00:01Z".to_string(),
            },
        ];
        
        let orphaned = find_orphaned_tools(&events);
        assert_eq!(orphaned, vec!["call_2"]);
    }

    #[test]
    fn test_replay_to_messages() {
        let events = vec![
            SessionEvent::UserMessage {
                id: "msg_1".to_string(),
                content: "Hello".to_string(),
                ts: "2024-01-01T00:00:00Z".to_string(),
            },
            SessionEvent::ToolCompleted {
                call_id: "call_1".to_string(),
                result: "file contents".to_string(),
                duration_ms: 100,
            },
        ];
        
        let messages = replay_to_messages(&events);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "tool");
    }
}
