//! Just-in-time memory retrieval — surfaces Hermes memories relevant to the
//! current user message before each provider call, budget-gated.
//!
//! This is the "JIT" connection between the existing `memory` crate (Hermes:
//! SQLite + FTS5 + n-gram embeddings) and the context pipeline, per strategy 4
//! of the P1 proposal.

use memory::MemoryStore;

/// Default token budget for retrieved memory (proposal: ~2000 tokens).
pub const DEFAULT_MEMORY_BUDGET: usize = 2000;

/// Retrieves and formats memories relevant to `query` from Hermes.
pub struct JitRetriever {
    /// Token cap for the formatted output.
    pub budget_tokens: usize,
}

impl Default for JitRetriever {
    fn default() -> Self {
        Self {
            budget_tokens: DEFAULT_MEMORY_BUDGET,
        }
    }
}

impl JitRetriever {
    /// Search Hermes for `query` and return a `<retrieved_context>` block, or
    /// empty when nothing relevant is found.
    ///
    /// The returned string is designed to be injected as a system message
    /// after the base system prompt (per proposal strategy 4).
    pub fn retrieve(&self, store: &MemoryStore, query: &str) -> String {
        let result = match store.search(query, None, 5) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("jit retrieval search failed: {}", e);
                return String::new();
            }
        };

        if result.entries.is_empty() {
            return String::new();
        }

        let mut lines: Vec<String> = vec![];
        let mut used: usize = 0;

        for (entry, relevance) in result.entries.iter().zip(result.relevance.iter()) {
            if *relevance <= 0.0 {
                continue;
            }
            let value: String = if entry.value.chars().count() > 500 {
                entry.value.chars().take(500).collect::<String>() + "…[truncated]"
            } else {
                entry.value.clone()
            };
            let line = format!("- {}: {}", entry.key, value);
            let approx_tokens = line.chars().count() / 4;
            if !lines.is_empty() && used + approx_tokens > self.budget_tokens {
                break;
            }
            lines.push(line);
            used += approx_tokens;
        }

        if lines.is_empty() {
            return String::new();
        }

        let body = lines.join("\n");
        format!(
            "<retrieved_context>\n# Relevant memories:\n{}\n</retrieved_context>",
            body
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_store_returns_empty() {
        let store = MemoryStore::new(":memory:").unwrap();
        let jit = JitRetriever::default();
        assert!(jit.retrieve(&store, "anything").is_empty());
    }

    #[test]
    fn retrieves_stored_memory() {
        let store = MemoryStore::new(":memory:").unwrap();
        store
            .store(
                memory::MemoryLayer::Project,
                "auth_pattern",
                "login uses JWT with httpOnly cookies",
            )
            .unwrap();
        let jit = JitRetriever::default();
        let out = jit.retrieve(&store, "login auth");
        assert!(
            out.contains("auth_pattern"),
            "expected memory in output: {}",
            out
        );
        assert!(out.starts_with("<retrieved_context>"));
        assert!(out.ends_with("</retrieved_context>"));
    }

    #[test]
    fn respects_budget() {
        let store = MemoryStore::new(":memory:").unwrap();
        for i in 0..5 {
            store
                .store(
                    memory::MemoryLayer::Project,
                    &format!("key_{}", i),
                    &"x".repeat(200),
                )
                .unwrap();
        }
        let jit = JitRetriever { budget_tokens: 60 };
        let out = jit.retrieve(&store, "value");
        let approx = out.chars().count() / 4;
        assert!(
            approx <= 60 + 40,
            "budget exceeded: ~{} tokens (budget 60)",
            approx
        );
    }
}
