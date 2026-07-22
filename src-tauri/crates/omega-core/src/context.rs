//! Conversation context utilities for token estimation and compaction

use providers::{ChatMessage, ToolCall, ToolCallFunction};

/// Estimates tokens using characters/4 over messages and tool calls
pub fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    let mut count = 0;
    for msg in messages {
        count += msg.content.chars().count();
        if let Some(tc) = msg.tool_calls.as_deref() {
            for t in tc {
                count += t.arguments.chars().count();
            }
        }
    }
    count / 4 // 4 chars per token approximation
}

/// Compacts conversation history when exceeding 70% of model window
pub fn compact(
    messages: Vec<ChatMessage>,
    keep_last_n: usize,
    model_window: u64,
) -> (Vec<ChatMessage>, String) {
    let token_count = estimate_tokens(&messages);
    let threshold = (model_window as f64 * 0.7) as usize;

    if token_count <= threshold {
        return (messages, String::new());
    }

    // Step 1: Keep system prompt (first message if system)
    let has_system = messages.first().map(|m| m.role == "system").unwrap_or(false);

    // Step 2: Find all user/assistant message indices
    let mut ua_indices: Vec<usize> = vec![];
    for (i, msg) in messages.iter().enumerate() {
        if msg.role == "user" || msg.role == "assistant" {
            ua_indices.push(i);
        }
    }

    // Step 3: Determine which UA turns to keep (last keep_last_n)
    let keep_ua_start = if ua_indices.len() > keep_last_n {
        ua_indices.len() - keep_last_n
    } else {
        0
    };
    let keep_ua_indices: std::collections::HashSet<usize> =
        ua_indices[keep_ua_start..].iter().copied().collect();

    // Step 4: Find tool pairs (assistant tool_calls + corresponding tool messages)
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
        } else {
            if let Some(pair) = current_pair.take() {
                if pair.len() > 1 {
                    tool_pairs.push(pair);
                }
            }
        }
    }
    if let Some(pair) = current_pair {
        if pair.len() > 1 {
            tool_pairs.push(pair);
        }
    }

    // Keep last 8 tool pairs
    let keep_tool_pairs: std::collections::HashSet<usize> = if tool_pairs.len() > 8 {
        tool_pairs[tool_pairs.len() - 8..]
            .iter()
            .flat_map(|p| p.iter())
            .copied()
            .collect()
    } else {
        tool_pairs.iter().flat_map(|p| p.iter()).copied().collect()
    };

    // Step 5: Build compacted messages and collect summary lines
    let mut compacted = vec![];
    let mut summary_lines: Vec<String> = vec![];

    for (idx, msg) in messages.iter().enumerate() {
        let is_kept_ua = keep_ua_indices.contains(&idx);
        let is_kept_tool = keep_tool_pairs.contains(&idx);
        let is_system = has_system && idx == 0;

        if is_system {
            compacted.push(msg.clone());
        } else if is_kept_ua || is_kept_tool {
            compacted.push(msg.clone());
        } else if (msg.role == "user" || msg.role == "assistant") && !msg.content.is_empty() {
            // Extractive summary: first non-empty line
            let line = msg.content.lines().find(|l| !l.trim().is_empty());
            if let Some(l) = line {
                summary_lines.push(l.to_string());
            }
        }
    }

    // Step 6: Build summary message and insert after system prompt
    let summary_text = if summary_lines.is_empty() {
        String::new()
    } else {
        let summary = summary_lines.join(" | ");
        format!("Conversation summary: {}", summary)
    };

    if !summary_text.is_empty() {
        let insert_idx = if has_system { 1 } else { 0 };
        compacted.insert(
            insert_idx,
            ChatMessage {
                role: "system".into(),
                content: summary_text,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        );
    }

    (compacted, summary_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_message(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    fn build_tool_message(content: &str, tool_call_id: &str, name: &str) -> ChatMessage {
        ChatMessage {
            role: "tool".into(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
            name: Some(name.to_string()),
        }
    }

    fn build_assistant_with_tool_calls(tool_calls: Vec<ToolCall>) -> ChatMessage {
        ChatMessage {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
        }
    }

    #[test]
    fn test_estimate_tokens_zero_on_empty() {
        let messages: Vec<ChatMessage> = vec![];
        assert_eq!(estimate_tokens(&messages), 0);
    }

    #[test]
    fn test_estimate_tokens_basic() {
        let messages = vec![
            build_message("user", "Hello world"),
            build_message("assistant", "Hi there!"),
        ];
        // "Hello world" (11) + "Hi there!" (9) = 20 chars / 4 = 5 tokens
        assert_eq!(estimate_tokens(&messages), 5);
    }

    #[test]
    fn test_estimate_tokens_with_tool_calls() {
        let messages = vec![
            build_message("assistant", ""),
            ChatMessage {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".into(),
                    tool_type: "function".into(),
                    function: ToolCallFunction {
                        name: "read".into(),
                        arguments: r#"{"filePath": "test.txt"}"#.into(),
                    },
                }]),
                tool_call_id: None,
                name: None,
            },
            build_tool_message("file content", "call_1", "read"),
        ];
        // Assistant content (0) + tool call args (20) + tool content (12) = 32 chars / 4 = 8 tokens
        assert!(estimate_tokens(&messages) >= 8);
    }

    #[test]
    fn test_estimate_tokens_monotonic() {
        let short = vec![build_message("user", "hi")];
        let longer = vec![build_message("user", "hello world this is longer")];
        assert!(estimate_tokens(&longer) > estimate_tokens(&short));
    }

    #[test]
    fn test_compact_no_op_under_threshold() {
        let messages = vec![build_message("user", "hello")];
        let (result, summary) = compact(messages.clone(), 6, 128_000);
        assert_eq!(result.len(), 1);
        assert!(summary.is_empty());
    }

    #[test]
    fn test_compact_preserves_last_user_message() {
        let mut messages = vec![build_message("system", "You are helpful")];
        for i in 0..20 {
            messages.push(build_message("user", &format!("Question {}", i)));
            messages.push(build_message("assistant", &format!("Answer {}", i)));
        }
        // End with a user message (not assistant)
        messages.push(build_message("user", "Final question"));

        let (result, _summary) = compact(messages, 6, 1000);

        // Last message should be the last user message
        assert_eq!(result.last().map(|m| m.role.as_str()), Some("user"));
        assert_eq!(
            result.last().map(|m| m.content.as_str()),
            Some("Final question")
        );
    }

    #[test]
    fn test_compact_large_fixture_no_panic() {
        let mut messages = vec![build_message("system", "You are helpful")];
        for i in 0..200 {
            messages.push(build_message("user", &format!("User message number {} with some padding text to make it longer", i)));
            messages.push(build_message("assistant", &format!("Assistant response number {}", i)));
        }

        // Use low model window to force threshold
        let (result, summary) = compact(messages.clone(), 6, 1000);

        // Should produce a summary
        assert!(!summary.is_empty());
        // Should have fewer messages than original
        assert!(result.len() < messages.len());
    }

    #[test]
    fn test_compact_produces_summary_when_needed() {
        let mut messages = vec![build_message("system", "You are helpful")];
        // Create a large conversation that exceeds threshold
        for _ in 0..100 {
            messages.push(build_message("user", "This is a very long user message with lots of content to fill up tokens and exceed the threshold for compaction"));
            messages.push(build_message("assistant", "This is a very long assistant response with lots of content to fill up tokens and exceed the threshold for compaction"));
        }

        let (result, summary) = compact(messages, 6, 100_000);

        // Should have compacted
        assert!(!summary.is_empty());
        assert!(summary.starts_with("Conversation summary:"));
        // Summary message should be inserted after system
        assert_eq!(result[0].role, "system"); // original system
        assert_eq!(result[1].role, "system"); // summary
        assert!(result[1].content.starts_with("Conversation summary:"));
    }

    #[test]
    fn test_compact_tool_pair_invariant() {
        let mut messages = vec![build_message("system", "You are helpful")];

        // Add 20 user/assistant turns without tools
        for i in 0..20 {
            messages.push(build_message("user", &format!("Question {}", i)));
            messages.push(build_message("assistant", &format!("Answer {}", i)));
        }

        // Add 15 tool pairs (assistant tool_calls + tool result)
        for i in 0..15 {
            let tc = ToolCall {
                id: format!("call_{}", i),
                tool_type: "function".into(),
                function: ToolCallFunction {
                    name: "read".into(),
                    arguments: format!(r#"{{"filePath": "file{}.txt"}}"#, i),
                },
            };
            messages.push(build_assistant_with_tool_calls(vec![tc]));
            messages.push(build_tool_message(
                &format!("Content of file {}", i),
                &format!("call_{}", i),
                "read",
            ));
        }

        // Compact with keep_last_n=6 and low window to force compaction
        let (result, _summary) = compact(messages, 6, 1000);

        // Verify: every retained tool message has its immediately preceding assistant tool_call
        let mut last_assistant_idx: Option<usize> = None;
        let mut retained_tool_pairs = 0;

        for (idx, msg) in result.iter().enumerate() {
            if msg.role == "assistant" && msg.tool_calls.is_some() {
                last_assistant_idx = Some(idx);
            } else if msg.role == "tool" {
                // Every tool must have preceding assistant with tool_calls
                assert!(
                    last_assistant_idx.is_some(),
                    "Tool message at index {} has no preceding assistant with tool_calls",
                    idx
                );
                assert_eq!(
                    last_assistant_idx,
                    Some(idx - 1),
                    "Tool message at index {} not immediately after its assistant",
                    idx
                );
                retained_tool_pairs += 1;
            }
        }

        // At most last 8 tool pairs should be retained
        assert!(
            retained_tool_pairs <= 8,
            "Expected at most 8 tool pairs, got {}",
            retained_tool_pairs
        );
    }
}