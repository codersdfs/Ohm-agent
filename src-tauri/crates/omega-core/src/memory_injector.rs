//! Auto-inject Hermes memory into context.
//!
//! Before each provider call, searches Hermes (project + user layers)
//! for memories relevant to the current user message and injects the
//! top results as a `<retrieved_context>` block after the system prompt.

use memory::MemoryStore;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::context::estimate_tokens;
use crate::MutexExt;

pub const DEFAULT_MAX_TOKENS: u32 = 2000;
pub const DEFAULT_MIN_RELEVANCE: f64 = 0.3;
pub const DEFAULT_MAX_ENTRIES: usize = 5;

pub struct MemoryInjector {
    memory: Arc<Mutex<MemoryStore>>,
    max_tokens: u32,
    min_relevance: f64,
    max_entries: usize,
    last_injected: Mutex<HashSet<String>>,
}

impl MemoryInjector {
    pub fn new(memory: Arc<Mutex<MemoryStore>>) -> Self {
        Self {
            memory,
            max_tokens: DEFAULT_MAX_TOKENS,
            min_relevance: DEFAULT_MIN_RELEVANCE,
            max_entries: DEFAULT_MAX_ENTRIES,
            last_injected: Mutex::new(HashSet::new()),
        }
    }

    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = max;
        self
    }

    pub fn with_min_relevance(mut self, min: f64) -> Self {
        self.min_relevance = min;
        self
    }

    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    pub fn retrieve(&self, user_message: &str) -> Option<String> {
        let store = self.memory.lock_guard();

        let mut results = Vec::new();
        let mut injected_keys = self.last_injected.lock_guard();
        let mut token_budget = self.max_tokens as usize;

        for layer in &["project", "user"] {
            let search_result = match store.search(user_message, Some(layer), self.max_entries) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("memory search failed for layer {}: {}", layer, e);
                    continue;
                }
            };

            for (entry, relevance) in search_result.entries.iter().zip(search_result.relevance.iter()) {
                if *relevance < self.min_relevance {
                    continue;
                }

                if injected_keys.contains(&entry.key) {
                    continue;
                }

                let value = if entry.value.chars().count() > 500 {
                    format!("{}…", entry.value.chars().take(500).collect::<String>())
                } else {
                    entry.value.clone()
                };

                let line = format!("- {}: {}", entry.key, value);
                let line_tokens = estimate_tokens(&[providers::ChatMessage {
                    role: "system".into(),
                    content: line.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                }]);

                if line_tokens > token_budget {
                    break;
                }

                results.push(line);
                token_budget -= line_tokens;
                injected_keys.insert(entry.key.clone());
            }
        }

        if results.is_empty() {
            return None;
        }

        let header = "# Relevant project memory (from previous sessions):";
        let context = format!("{}\n{}\n", header, results.join("\n"));
        Some(context)
    }

    pub fn clear_injected(&self) {
        self.last_injected.lock_guard().clear();
    }
}

pub fn build_turn_context(
    state: &crate::AppState,
    user_msg: &str,
) -> Result<String, String> {
    let mut ctx = String::new();

    let learning_rules = crate::learning::LearningModule::get_prompt_rules(state);
    if !learning_rules.is_empty() {
        ctx.push_str(&learning_rules);
    }

    let injector = MemoryInjector::new(Arc::new(Mutex::new(
        memory::MemoryStore::new(&state.db_path)
            .map_err(|e| format!("Failed to create memory store: {}", e))?,
    )));

    if let Some(hermes) = injector.retrieve(user_msg) {
        ctx.push_str(&format!(
            "\n<retrieved_context>\n{}\n</retrieved_context>",
            hermes
        ));
    }

    Ok(ctx)
}
