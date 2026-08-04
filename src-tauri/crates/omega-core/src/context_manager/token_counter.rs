//! Provider-aware token counting.
//!
//! Replaces the naive `chars / 4` estimator with a `TokenCounter` trait.
//! The `TokenizersCounter` uses the HuggingFace `tokenizers` crate when a
//! tokenizer config is available; `Chars4Counter` is the zero-dependency
//! baseline (per research decision 3, exactness beyond overflow prevention
//! is not required).

use providers::ChatMessage;
use std::path::Path;

/// Token-counting strategy. Implementations must be cheap enough to run
/// before every provider call.
pub trait TokenCounter: Send + Sync {
    fn count_text(&self, text: &str) -> u32;

    fn count_messages(&self, messages: &[ChatMessage]) -> u32 {
        let mut count = 0u32;
        for msg in messages {
            count += self.count_text(&msg.content);
            if let Some(tool_calls) = msg.tool_calls.as_deref() {
                for t in tool_calls {
                    count += self.count_text(&t.function.arguments);
                }
            }
        }
        count
    }
}

/// Baseline estimator: 4 characters per token (the previous `estimate_tokens`).
pub struct Chars4Counter;

impl TokenCounter for Chars4Counter {
    fn count_text(&self, text: &str) -> u32 {
        (text.chars().count() / 4) as u32
    }
}

/// `tokenizers`-backed BPE counter. Loads a tokenizer config from disk;
/// construction fails (returns `None` from [`load`]) when no config is
/// available, in which case callers fall back to [`Chars4Counter`].
///
/// Local-first: the config must already be present on disk (bundled or
/// previously cached). This implementation never downloads at runtime.
pub struct TokenizersCounter {
    tokenizer: tokenizers::Tokenizer,
}

impl TokenizersCounter {
    /// Load a tokenizer from a `tokenizer.json`-format file.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let tokenizer = tokenizers::Tokenizer::from_file(path)
            .map_err(|e| format!("failed to load tokenizer {}: {}", path.display(), e))?;
        Ok(Self { tokenizer })
    }

    /// Load from bytes (e.g. an embedded asset).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let tokenizer = tokenizers::Tokenizer::from_bytes(bytes)
            .map_err(|e| format!("failed to deserialize tokenizer: {}", e))?;
        Ok(Self { tokenizer })
    }

    /// Try well-known locations: `OMEGA_TOKENIZER_PATH` first, then
    /// `omega-core/assets/tokenizers/*.json` relative to the repo root.
    pub fn load_from_env_or_defaults() -> Option<Self> {
        if let Ok(path) = std::env::var("OMEGA_TOKENIZER_PATH") {
            if let Ok(t) = Self::from_file(Path::new(&path)) {
                return Some(t);
            }
        }
        for candidate in [
            "omega-core/assets/tokenizers/tokenizer.json",
            "assets/tokenizers/tokenizer.json",
        ] {
            if let Ok(t) = Self::from_file(Path::new(candidate)) {
                return Some(t);
            }
        }
        None
    }
}

impl TokenCounter for TokenizersCounter {
    fn count_text(&self, text: &str) -> u32 {
        match self.tokenizer.encode(text, true) {
            Ok(encoding) => encoding.get_ids().len() as u32,
            Err(_) => (text.chars().count() / 4) as u32,
        }
    }
}

/// Resolve a token counter for a model hint.
///
/// Uses `tokenizers` when a config is available; otherwise falls back to
/// chars/4. The model hint is reserved for provider-aware mapping (ticket 02)
/// — today all providers share the same counter.
pub fn resolve(_model_hint: &str) -> Box<dyn TokenCounter> {
    if let Some(counter) = TokenizersCounter::load_from_env_or_defaults() {
        log::debug!("token counter: tokenizers (BPE)");
        Box::new(counter)
    } else {
        log::debug!("token counter: chars/4 fallback");
        Box::new(Chars4Counter)
    }
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
    fn chars4_counts_content() {
        let c = Chars4Counter;
        assert_eq!(c.count_text("Hello world"), 2);
        assert_eq!(c.count_text(""), 0);
    }

    #[test]
    fn chars4_counts_messages_with_tool_calls() {
        let messages = vec![
            msg("user", "abcd"),
            ChatMessage {
                role: "assistant".into(),
                content: String::new(),
                tool_calls: Some(vec![providers::ToolCall {
                    id: "1".into(),
                    tool_type: "function".into(),
                    function: providers::ToolCallFunction {
                        name: "read".into(),
                        arguments: "abcdefgh".into(),
                    },
                }]),
                tool_call_id: None,
                name: None,
            },
        ];
        let c = Chars4Counter;
        // 4 chars user + 8 chars args = 12 chars / 4 = 3 tokens
        assert_eq!(c.count_messages(&messages), 3);
    }

    #[test]
    fn resolve_falls_back_without_config() {
        let counter = resolve("gpt-4o");
        // Whatever implementation, it must not panic and must accept empty text.
        assert_eq!(counter.count_text(""), 0);
    }

    #[test]
    fn from_bytes_rejects_garbage() {
        assert!(TokenizersCounter::from_bytes(b"not a tokenizer").is_err());
    }
}
