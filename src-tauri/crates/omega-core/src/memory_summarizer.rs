//! Memory summarization for turn-level context engineering.
//!
//! Condenses each conversation turn into a key-value memory entry
//! stored in the Project layer, keyed by git root.

use crate::memory_retriever::project_key;
use memory::{MemoryLayer, MemoryStore};

/// Maximum characters for the stored summary value.
const MAX_SUMMARY_CHARS: usize = 500;

/// Generate a turn summary string from user message, assistant response, and tool names.
fn generate_summary(user_msg: &str, assistant_msg: &str, tool_names: &[String]) -> String {
    let user_short: String = user_msg.chars().take(100).collect();
    let assistant_short: String = assistant_msg.chars().take(MAX_SUMMARY_CHARS - 100).collect();

    let mut summary = format!("User asked: {}. Assistant: {}", user_short, assistant_short);

    if !tool_names.is_empty() {
        let tools = tool_names.join(", ");
        summary.push_str(&format!(". Tools: {}", tools));
    }

    if summary.chars().count() > MAX_SUMMARY_CHARS {
        summary = summary.chars().take(MAX_SUMMARY_CHARS).collect::<String>() + "…";
    }

    summary
}

/// Store a turn summary in the project memory layer.
///
/// Uses the project key (git root) as the memory key prefix.
/// Returns the memory key used for storage.
pub fn store_turn_summary(
    store: &MemoryStore,
    user_msg: &str,
    assistant_msg: &str,
    tool_names: &[String],
) -> Result<String, String> {
    if assistant_msg.trim().is_empty() {
        return Ok(String::new());
    }

    let summary = generate_summary(user_msg, assistant_msg, tool_names);
    let proj_key = project_key();
    let memory_key = format!("turn-summary:{}", proj_key);

    let id = store.store(MemoryLayer::Project, &memory_key, &summary)?;
    log::info!("stored turn summary (id={}) for project key={}", id, proj_key);

    Ok(memory_key)
}

/// Summarize a turn and return the summary string (without storing).
pub fn summarize_turn(
    store: &MemoryStore,
    user_msg: &str,
    assistant_msg: &str,
    tool_names: &[String],
) -> Result<String, String> {
    let _ = store;
    Ok(generate_summary(user_msg, assistant_msg, tool_names))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_turn_basic() {
        let store = memory::MemoryStore::new(":memory:").unwrap();
        let result = store_turn_summary(
            &store,
            "How do I build this project?",
            "Use cargo tauri dev to build and run the project.",
            &[],
        );
        assert!(result.is_ok(), "should store summary successfully");
    }

    #[test]
    fn test_summarize_turn_with_tools() {
        let store = memory::MemoryStore::new(":memory:").unwrap();
        let result = store_turn_summary(
            &store,
            "Find all Rust files",
            "I found 15 files matching your pattern.",
            &["glob".to_string(), "grep".to_string()],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_summarize_turn_empty_assistant() {
        let store = memory::MemoryStore::new(":memory:").unwrap();
        let result = store_turn_summary(
            &store,
            "Hello",
            "",
            &[],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_summarize_turn_truncates_long_response() {
        let store = memory::MemoryStore::new(":memory:").unwrap();
        let long_response = "x".repeat(2000);
        let result = store_turn_summary(
            &store,
            "Tell me something long",
            &long_response,
            &[],
        );
        assert!(result.is_ok());
        let key = result.unwrap();
        if !key.is_empty() {
            let stored = store.remember(&key, Some("project")).unwrap();
            assert!(stored.is_some());
            let value = stored.unwrap();
            assert!(value.len() <= 600, "stored value should be truncated, got {} chars", value.len());
        }
    }
}
