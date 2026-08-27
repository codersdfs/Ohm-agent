//! Structured summarization — replaces "drop middle messages" with an
//! extractive `CompactionSummary` built from tool-call records and message
//! content (deterministic; no mid-loop LLM call, per ticket 03's extraction
//! path).

use providers::ChatMessage;
use std::collections::HashSet;

/// Structured summary of compacted conversation history.
///
/// Field names are stable so a later LLM-summarization path (ticket 03,
/// hybrid) can populate the same struct without changing consumers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompactionSummary {
    pub decisions: Vec<String>,
    pub files_edited: Vec<String>,
    pub errors_encountered: Vec<String>,
    pub tool_calls_summary: String,
}

impl CompactionSummary {
    pub fn is_empty(&self) -> bool {
        self.decisions.is_empty()
            && self.files_edited.is_empty()
            && self.errors_encountered.is_empty()
            && self.tool_calls_summary.is_empty()
    }

    /// Render into a single system-message string.
    pub fn to_message(&self) -> String {
        let mut out = String::from("Conversation summary (compacted):\n");
        if !self.decisions.is_empty() {
            out.push_str("\nDecisions:\n");
            for d in &self.decisions {
                out.push_str(&format!("- {}\n", d));
            }
        }
        if !self.files_edited.is_empty() {
            out.push_str("\nFiles edited:\n");
            for f in &self.files_edited {
                out.push_str(&format!("- {}\n", f));
            }
        }
        if !self.errors_encountered.is_empty() {
            out.push_str("\nErrors encountered:\n");
            for e in &self.errors_encountered {
                out.push_str(&format!("- {}\n", e));
            }
        }
        if !self.tool_calls_summary.is_empty() {
            out.push_str(&format!("\nTool calls:\n{}\n", self.tool_calls_summary));
        }
        out
    }
}

/// Compact conversation history.
///
/// Always preserves: the leading system prompt (verbatim), the last
/// `keep_last_n` user/assistant turns (verbatim), and their tool-call pairs.
/// Everything older is summarized extractively into a [`CompactionSummary`
/// message inserted after the system prompt.
pub fn compact(
    messages: Vec<ChatMessage>,
    keep_last_n: usize,
) -> (Vec<ChatMessage>, CompactionSummary) {
    // All leading system messages (base prompt + injected repo-map/memory
    // sections) are preserved verbatim — never summarized.
    let leading_system: HashSet<usize> = messages
        .iter()
        .take_while(|m| m.role == "system")
        .enumerate()
        .map(|(i, _)| i)
        .collect();

    // Find user/assistant turn indices.
    let mut ua_indices: Vec<usize> = vec![];
    for (i, msg) in messages.iter().enumerate() {
        if msg.role == "user" || msg.role == "assistant" {
            ua_indices.push(i);
        }
    }

    let keep_ua_start = ua_indices.len().saturating_sub(keep_last_n);
    let keep_ua_indices: HashSet<usize> = ua_indices[keep_ua_start..].iter().copied().collect();

    // Find tool pairs (assistant tool_calls + following tool messages).
    let mut tool_pairs: Vec<Vec<usize>> = vec![];
    let mut current_pair: Option<Vec<usize>> = None;
    for (i, msg) in messages.iter().enumerate() {
        if msg.role == "assistant" && msg.tool_calls.is_some() {
            if let Some(pair) = current_pair.take() {
                if pair.len() > 1 {
                    tool_pairs.push(pair);
                }
            }
            current_pair = Some(vec![i]);
        } else if msg.role == "tool" {
            if let Some(ref mut pair) = current_pair {
                pair.push(i);
            }
        } else if let Some(pair) = current_pair.take() {
            if pair.len() > 1 {
                tool_pairs.push(pair);
            }
        }
    }
    if let Some(pair) = current_pair {
        if pair.len() > 1 {
            tool_pairs.push(pair);
        }
    }

    // Keep the last 8 tool pairs verbatim (matches prior behavior).
    let keep_tool_pairs: HashSet<usize> = if tool_pairs.len() > 8 {
        tool_pairs[tool_pairs.len() - 8..]
            .iter()
            .flat_map(|p| p.iter())
            .copied()
            .collect()
    } else {
        tool_pairs.iter().flat_map(|p| p.iter()).copied().collect()
    };

    let mut summary = CompactionSummary::default();
    let mut compacted: Vec<ChatMessage> = vec![];
    let mut tool_calls_snapshot: Vec<String> = vec![];

    for (idx, msg) in messages.iter().enumerate() {
        let is_system = leading_system.contains(&idx);
        let is_kept_ua = keep_ua_indices.contains(&idx);
        let is_kept_tool = keep_tool_pairs.contains(&idx);

        if is_system {
            compacted.push(msg.clone());
        } else if is_kept_ua || is_kept_tool {
            compacted.push(msg.clone());
        } else {
            // Everything else feeds the summary.
            summarize_message(msg, &mut summary, &mut tool_calls_snapshot);
        }
    }

    if !summary.is_empty() {
        // Insert the summary immediately after the preserved system block.
        compacted.insert(
            leading_system.len(),
            ChatMessage {
                role: "system".into(),
                content: summary.to_message(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        );
    }

    (compacted, summary)
}

fn summarize_message(
    msg: &ChatMessage,
    summary: &mut CompactionSummary,
    tool_calls_snapshot: &mut Vec<String>,
) {
    match msg.role.as_str() {
        "tool" => {
            if msg.content.to_lowercase().contains("error") {
                let snippet = first_line(&msg.content);
                if !snippet.is_empty() {
                    summary.errors_encountered.push(snippet);
                }
            }
        }
        "assistant" => {
            if let Some(calls) = msg.tool_calls.as_deref() {
                for t in calls {
                    tool_calls_snapshot.push(format!(
                        "{}: {}",
                        t.function.name,
                        first_line(&t.function.arguments)
                    ));
                }
            }
            if !msg.content.is_empty() {
                let line = first_line(&msg.content);
                if !line.is_empty() {
                    summary.decisions.push(line);
                }
            }
        }
        _ => {
            // User messages older than the kept window are dropped from the
            // summary (their content is captured by later decisions/tool
            // results); nothing to record here.
        }
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
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

    fn tool_call(name: &str, args: &str) -> providers::ToolCall {
        providers::ToolCall {
            id: "call_1".into(),
            tool_type: "function".into(),
            function: providers::ToolCallFunction {
                name: name.into(),
                arguments: args.into(),
            },
        }
    }

    #[test]
    fn compact_under_window_is_no_op() {
        let messages = vec![msg("user", "hello")];
        let (result, summary) = compact(messages.clone(), 6);
        assert_eq!(result.len(), 1);
        assert!(summary.is_empty());
    }

    #[test]
    fn preserves_system_and_last_n() {
        let mut messages = vec![msg("system", "You are helpful")];
        for i in 0..20 {
            messages.push(msg("user", &format!("Question {}", i)));
            messages.push(msg("assistant", &format!("Answer {}", i)));
        }
        messages.push(msg("user", "Final question"));

        let (result, summary) = compact(messages, 6);
        assert!(!summary.is_empty());
        assert_eq!(result[0].role, "system");
        assert_eq!(
            result.last().map(|m| m.content.as_str()),
            Some("Final question")
        );
        // Summary message inserted right after the system prompt.
        assert_eq!(result[1].role, "system");
        assert!(result[1].content.starts_with("Conversation summary"));
    }

    #[test]
    fn tool_pairs_stay_verbatim() {
        let mut messages = vec![msg("system", "sys")];
        for i in 0..10 {
            messages.push(msg("user", &format!("q{}", i)));
        }
        for i in 0..10 {
            messages.push(ChatMessage {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: Some(vec![tool_call("read", "{\"f\":\"a\"}")]),
                tool_call_id: None,
                name: None,
            });
            messages.push(msg("tool", format!("result {}", i).as_str()));
        }
        let (result, _summary) = compact(messages, 6);
        let tool_messages = result.iter().filter(|m| m.role == "tool").count();
        assert!(
            tool_messages <= 8,
            "expected <= 8 tool results, got {}",
            tool_messages
        );
    }

    #[test]
    fn errors_are_captured() {
        // 12 tool pairs — the 8-pair verbatim cap drops the early pairs into
        // the summary path, so an early error result gets captured.
        let mut messages = vec![msg("system", "sys")];
        for i in 0..12 {
            messages.push(ChatMessage {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: Some(vec![tool_call(
                    "write",
                    &format!("{{\"path\":\"{}.rs\"}}", i),
                )]),
                tool_call_id: None,
                name: None,
            });
            if i == 0 {
                messages.push(msg("tool", "Error: permission denied"));
            } else {
                messages.push(msg("tool", &format!("ok {}", i)));
            }
        }
        messages.push(msg("user", "final"));
        let (_result, summary) = compact(messages, 2);
        assert!(
            summary
                .errors_encountered
                .iter()
                .any(|e| e.contains("permission denied")),
            "expected permission error captured: {:?}",
            summary.errors_encountered
        );
    }

    #[test]
    fn summary_message_roundtrips() {
        let s = CompactionSummary {
            decisions: vec!["Use tokio".into()],
            files_edited: vec!["src/a.rs".into()],
            errors_encountered: vec![],
            tool_calls_summary: "write".into(),
        };
        let text = s.to_message();
        assert!(text.contains("Use tokio"));
        assert!(text.contains("src/a.rs"));
    }
}
