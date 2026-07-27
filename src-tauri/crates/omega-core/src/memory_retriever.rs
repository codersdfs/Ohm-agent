//! Memory retrieval and formatting for context engineering.
//!
//! Retrieves relevant project memories from the SQLite memory store and
//! formats them into a system-prompt snippet with a dynamic token budget.

use crate::context::estimate_tokens;
use memory::{MemoryEntry, MemoryLayer, MemoryStore};

/// Maximum characters per memory entry in the formatted output.
const MAX_ENTRY_CHARS: usize = 500;

/// Resolve the project key for memory storage/retrieval.
///
/// Uses `git rev-parse --show-toplevel` to get the repository root.
/// Falls back to `"no-git:<absolute-cwd-path>"` when not in a git repo.
pub fn project_key() -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let path = String::from_utf8_lossy(&o.stdout);
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        _ => {}
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    format!("no-git:{}", cwd.display())
}

/// Format memory entries into a system-prompt snippet.
///
/// Returns a string like:
/// ```
/// # Relevant project memory (from previous sessions):
/// - key_name: value text
/// ```
/// Returns empty string if no entries.
pub fn format_memory_context(entries: &[MemoryEntry], relevances: &[f64]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push("# Relevant project memory (from previous sessions):".to_string());

    for (entry, rel) in entries.iter().zip(relevances.iter()) {
        let value: String = if entry.value.chars().count() > MAX_ENTRY_CHARS {
            entry.value.chars().take(MAX_ENTRY_CHARS).collect::<String>()
                + "…[truncated]"
        } else {
            entry.value.clone()
        };

        if *rel > 0.0 {
            lines.push(format!("- {}: {}", entry.key, value));
        }
    }

    if lines.len() <= 1 {
        return String::new();
    }

    lines.join("\n") + "\n"
}

/// Retrieve relevant memories from the project layer, respecting a token budget.
///
/// Searches the memory store for entries matching `query`, then formats them
/// into a system-prompt snippet. Stops adding entries when the cumulative
/// token count exceeds `budget_tokens`.
pub fn retrieve_memories(store: &MemoryStore, query: &str, budget_tokens: usize) -> String {
    let result = match store.search(query, Some("project"), 50) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("memory search failed: {}", e);
            return String::new();
        }
    };

    if result.entries.is_empty() {
        return String::new();
    }

    let mut context = String::new();
    let mut current_tokens = 0usize;

    for (entry, rel) in result.entries.iter().zip(result.relevance.iter()) {
        if *rel <= 0.0 {
            continue;
        }

        let value: String = if entry.value.chars().count() > MAX_ENTRY_CHARS {
            entry.value.chars().take(MAX_ENTRY_CHARS).collect::<String>()
                + "…[truncated]"
        } else {
            entry.value.clone()
        };
        let line = format!("- {}: {}\n", entry.key, value);

        let line_tokens = estimate_tokens(&[providers::ChatMessage {
            role: "system".into(),
            content: line.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }]);

        if current_tokens + line_tokens > budget_tokens && !context.is_empty() {
            log::info!(
                "memory budget reached: {} tokens (limit: {})",
                current_tokens,
                budget_tokens
            );
            break;
        }

        context.push_str(&line);
        current_tokens += line_tokens;
    }

    if context.is_empty() {
        return String::new();
    }

    format!("# Relevant project memory (from previous sessions):\n{}\n", context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_memory_context_basic() {
        let entries = vec![
            memory::MemoryEntry {
                id: "1".into(),
                layer: memory::MemoryLayer::Project,
                key: "build_cmd".into(),
                value: "cargo tauri dev".into(),
                embedding: None,
                timestamp: "2026-07-27T00:00:00Z".into(),
            },
        ];
        let relevances = vec![0.95];
        let result = format_memory_context(&entries, &relevances);
        assert!(result.contains("build_cmd"), "should contain key");
        assert!(result.contains("cargo tauri dev"), "should contain value");
        assert!(result.contains("Relevant project memory"), "should have header");
    }

    #[test]
    fn test_format_memory_context_empty() {
        let result = format_memory_context(&[], &[]);
        assert!(result.is_empty(), "empty entries should produce empty string");
    }

    #[test]
    fn test_format_memory_context_multiple() {
        let entries = vec![
            memory::MemoryEntry {
                id: "1".into(),
                layer: memory::MemoryLayer::Project,
                key: "api_url".into(),
                value: "https://api.example.com".into(),
                embedding: None,
                timestamp: "2026-07-27T00:00:00Z".into(),
            },
            memory::MemoryEntry {
                id: "2".into(),
                layer: memory::MemoryLayer::Project,
                key: "db_name".into(),
                value: "omega".into(),
                embedding: None,
                timestamp: "2026-07-27T00:00:00Z".into(),
            },
        ];
        let relevances = vec![0.9, 0.7];
        let result = format_memory_context(&entries, &relevances);
        assert!(result.contains("api_url"), "should contain first key");
        assert!(result.contains("db_name"), "should contain second key");
        assert!(result.contains("https://api.example.com"), "should contain first value");
    }

    #[test]
    fn test_retrieve_memories_respects_budget() {
        let store = memory::MemoryStore::new(":memory:").unwrap();
        for i in 0..5 {
            store.store(
                memory::MemoryLayer::Project,
                &format!("key_{}", i),
                &format!("value_{}", "x".repeat(100)),
            ).unwrap();
        }
        let result = retrieve_memories(&store, "value", 50);
        let token_count = estimate_tokens(&[providers::ChatMessage {
            role: "system".into(),
            content: result.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }]);
        assert!(token_count <= 100, "should stay within budget, got {} tokens", token_count);
    }

    #[test]
    fn test_retrieve_memories_empty_store() {
        let store = memory::MemoryStore::new(":memory:").unwrap();
        let result = retrieve_memories(&store, "anything", 1000);
        assert!(result.is_empty(), "empty store should return empty string");
    }
}
