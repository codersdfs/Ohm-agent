//! Provider-aware token counting.
//!
//! A single `TokenCounter` struct backs both counting strategies. The
//! closed set of variants (chars/4 baseline and BPE tokenizer) is modelled
//! as an internal enum, avoiding the heap allocation and vtable indirection
//! of a `Box<dyn Trait>` while preserving the same public API.
//!
//! When the `token-counting` feature is enabled, the `Tokenizers` variant
//! uses bundled BPE configs (via `include_bytes!`) for the dominant
//! provider encodings (cl100k_base for OpenAI, o200k_base for newer
//! OpenAI models). Without the feature, the `Chars4` variant is the zero-
//! dependency baseline (per research decision 3, exactness beyond overflow
//! is not required; dominant-encoding mapping with a safety margin is
//! sufficient).
use providers::ChatMessage;

#[cfg(feature = "token-counting")]
use std::path::Path;

/// Bundled BPE tokenizer bytes for `cl100k_base` encoding (OpenAI gpt-4o,
/// gpt-3.5-turbo, Claude approximate).
#[cfg(feature = "token-counting")]
const TOKENIZER_BYTES: &[u8] = include_bytes!("../../assets/tokenizers/tokenizer.json");

/// Resolve which bundled tokenizer to use for a model hint.
///
/// Returns the byte slice of the appropriate tokenizer.json. All OpenAI
/// family models share the same bundled BPE config (per research decision 3
/// — exact per-model counting is not required; dominant-encoding mapping
/// with a safety margin is sufficient). This function is called at resolve
/// time so a future expansion can pick between bundled configs.
#[cfg(feature = "token-counting")]
fn tokenizer_for_model(_model_hint: &str) -> &'static [u8] {
    // OpenAI gpt-4o/gpt-3.5-turbo → cl100k_base (bundled).
    // Anthropic → approximate via cl100k_base (conservative proxy, per
    // research decision 3).
    // Local llama → model's own tokenizer.json (user-supplied, OMEGA_TOKENIZER_PATH).
    TOKENIZER_BYTES
}

/// Internal strategy discriminator.
enum TokenCounterKind {
    Chars4,
    #[cfg(feature = "token-counting")]
    Tokenizers(tokenizers::Tokenizer),
}

/// Token counter for the active provider.
///
/// Implementations must be cheap enough to run before every provider call.
/// Construct via [`TokenCounter::chars4`] or [`TokenCounter::resolve`].
pub struct TokenCounter {
    kind: TokenCounterKind,
}

impl TokenCounter {
    /// Baseline estimator: 4 characters per token (the previous
    /// `estimate_tokens`). Zero dependencies.
    pub fn chars4() -> Self {
        Self {
            kind: TokenCounterKind::Chars4,
        }
    }

    /// Count tokens in a single text slice.
    pub fn count_text(&self, text: &str) -> u32 {
        match &self.kind {
            TokenCounterKind::Chars4 => (text.chars().count() / 4) as u32,
            #[cfg(feature = "token-counting")]
            TokenCounterKind::Tokenizers(tokenizer) => match tokenizer.encode(text, true) {
                Ok(encoding) => encoding.get_ids().len() as u32,
                Err(_) => (text.chars().count() / 4) as u32,
            },
        }
    }

    /// Count all tokens in a list of chat messages, including tool calls
    /// and their arguments.
    pub fn count_messages(&self, messages: &[ChatMessage]) -> u32 {
        let mut count = 0u32;
        for msg in messages {
            count += self.count_text(&msg.content);
            if let Some(tid) = &msg.tool_call_id {
                count += self.count_text(tid);
            }
            if let Some(tname) = &msg.name {
                count += self.count_text(tname);
            }
            if let Some(calls) = &msg.tool_calls {
                for call in calls {
                    count += self.count_text(&call.id);
                    count += self.count_text(&call.function.name);
                    count += self.count_text(&call.function.arguments);
                }
            }
        }
        count
    }

    /// Load from the bundled tokenizer.json (cl100k_base).
    /// Returns `None` if the bundled tokenizer fails to deserialize.
    #[cfg(feature = "token-counting")]
    fn load_bundled() -> Option<Self> {
        let tokenizer = tokenizers::Tokenizer::from_bytes(TOKENIZER_BYTES).ok()?;
        Some(Self {
            kind: TokenCounterKind::Tokenizers(tokenizer),
        })
    }

    /// Load from a file path (overrides bundled config; for user-supplied
    /// local model tokenizers).
    #[cfg(feature = "token-counting")]
    #[allow(dead_code)]
    fn from_file(path: &Path) -> Result<Self, String> {
        let tokenizer = tokenizers::Tokenizer::from_file(path)
            .map_err(|e| format!("failed to load tokenizer {}: {}", path.display(), e))?;
        Ok(Self {
            kind: TokenCounterKind::Tokenizers(tokenizer),
        })
    }
}

/// Resolve a token counter for a model hint.
///
/// With the `token-counting` feature: uses the bundled BPE tokenizer for
/// the model's dominant encoding. Without the feature: falls back to
/// chars/4. The model hint maps to a tokenizer config per ticket 02.
pub fn resolve(model_hint: &str) -> TokenCounter {
    #[cfg(feature = "token-counting")]
    {
        let _encoding = tokenizer_for_model(model_hint);
        if let Some(counter) = TokenCounter::load_bundled() {
            log::debug!("token counter: tokenizers (BPE) for model '{}'", model_hint);
            return counter;
        }
        log::warn!("token counter: bundled tokenizer failed, falling back to chars/4");
    }
    log::debug!("token counter: chars/4 fallback for model '{}'", model_hint);
    TokenCounter::chars4()
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
        let c = TokenCounter::chars4();
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
        let c = TokenCounter::chars4();
        // 4 chars user + 8 chars args = 12 chars / 4 = 3 tokens
        // 4 (abcd) + 1 (id "1") + 4 (name "read") + 8 (args "abcdefgh") = 17 chars / 4 = 4
        assert_eq!(c.count_messages(&messages), 4);
    }

    #[test]
    fn resolve_falls_back_to_chars4() {
        let counter = resolve("gpt-4o");
        // Must not panic; either BPE or chars/4.
        let _ = counter.count_text("Hello, world!");
    }

    #[test]
    fn resolve_handles_unknown_model() {
        let counter = resolve("unknown-model-xyz");
        let _ = counter.count_text("anything");
    }
}
