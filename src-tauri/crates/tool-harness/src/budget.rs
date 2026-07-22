// Result and context budgeting

use std::path::PathBuf;

const MAX_RESULT_CHARS: usize = 50_000;
const PERSISTENCE_DIR: &str = "tool-results";

/// Per-tool result budgeting
pub struct ResultBudget {
    pub max_chars: usize,
    pub persisted_dir: PathBuf,
}

impl Default for ResultBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultBudget {
    pub fn new() -> Self {
        let proj_dirs = directories::ProjectDirs::from("com", "omega", "omega-agent");
        let persisted_dir = proj_dirs
            .map(|d| d.data_dir().join(PERSISTENCE_DIR))
            .unwrap_or_else(|| PathBuf::from(format!(".{}", PERSISTENCE_DIR)));

        Self {
            max_chars: MAX_RESULT_CHARS,
            persisted_dir,
        }
    }

    /// Check and process result budgeting
    pub async fn check_and_process(&self, output: String, _tool_name: &str) -> crate::BudgetCheck {
        let trimmed = output.trim();
        // Consistent "max_chars" semantics -- use char count, not byte length
        if trimmed.chars().count() > self.max_chars {
            // Persist to file
            let hash = format!("{:016x}", md5::compute(trimmed.as_bytes()));
            let file_path = self.persisted_dir.join(format!("{}.txt", hash));
            
            // Create directory if needed
            if let Err(e) = std::fs::create_dir_all(&self.persisted_dir) {
                log::warn!("Failed to create persistence directory: {}", e);
                return crate::BudgetCheck {
                    within_limit: false,
                    truncated: true,
                    persisted_path: None,
                };
            }

            // Write content
            if let Err(e) = tokio::fs::write(&file_path, trimmed).await {
                log::warn!("Failed to persist output: {}", e);
            }

            crate::BudgetCheck {
                within_limit: false,
                truncated: true,
                persisted_path: Some(file_path),
            }
        } else {
            crate::BudgetCheck {
                within_limit: true,
                truncated: false,
                persisted_path: None,
            }
        }
    }

    /// Truncate output and return with persistence tag if needed
    pub async fn truncate(&self, output: &str) -> (String, crate::BudgetCheck) {
        let trimmed = output.trim();
        // Use char count for consistent "max_chars" semantics
        if trimmed.chars().count() > self.max_chars {
            // Find safe byte boundary at the max_chars-th character boundary
            let byte_boundary = trimmed
                .char_indices()
                .take(self.max_chars)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);
            let prefix = &trimmed[..byte_boundary];
            let original_chars = trimmed.chars().count();
            let mut result = String::new();
            result.push_str(prefix);

            let check = self.check_and_process(trimmed.to_string(), "").await;
            let path_note = check
                .persisted_path
                .as_ref()
                .map(|p| format!(" Full result saved to {}.", p.display()))
                .unwrap_or_default();
            result.push_str(&format!(
                "\n\n...[truncated: kept first {} chars of {}; re-read with a narrower command, offset/limit, or open the persisted file.{}]",
                self.max_chars, original_chars, path_note
            ));
            (result, check)
        } else {
            (trimmed.to_string(), crate::BudgetCheck {
                within_limit: true,
                truncated: false,
                persisted_path: None,
            })
        }
    }
}

/// Conversation-level token budgeting
pub struct ConversationBudget {
    pub total_turns: usize,
    pub estimated_tokens: usize,
}

impl Default for ConversationBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationBudget {
    pub fn new() -> Self {
        Self {
            total_turns: 0,
            estimated_tokens: 0,
        }
    }

    pub fn update(&mut self, input_len: usize, output_len: usize) {
        self.total_turns += 1;
        self.estimated_tokens += input_len + output_len;
    }

    pub fn within_limit(&self, max_tokens: usize) -> bool {
        self.estimated_tokens <= max_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_budget_default() {
        let budget = ResultBudget::new();
        assert_eq!(budget.max_chars, 50_000);
    }

    #[tokio::test]
    async fn test_truncate_small_output() {
        let budget = ResultBudget::new();
        let (output, check) = budget.truncate("hello world").await;
        assert_eq!(output, "hello world");
        assert!(check.within_limit);
        assert!(!check.truncated);
    }

    #[tokio::test]
    async fn test_truncate_large_output() {
        let budget = ResultBudget::new();
        let large = "x".repeat(55_000);
        let (output, check) = budget.truncate(&large).await;
        assert!(check.truncated || output.len() < large.len());
    }

    #[tokio::test]
    async fn test_truncate_utf8_boundary_safe() {
        // A string with multi-byte UTF-8 characters where the byte boundary
        // would previously panic (3-byte emoji right at the limit edge).
        // Build content: ~49_999 ASCII chars then multi-byte chars past the limit
        let prefix = "a".repeat(49_990);
        // 3-byte UTF-8 character (€ = U+20AC = E2 82 AC in UTF-8)
        let mut mixed = String::new();
        mixed.push_str(&prefix);
        // Fill the rest with multi-byte chars to hit exactly past the limit
        for _ in 0..20 {
            mixed.push('€'); // 3 bytes each
        }

        let budget = ResultBudget::new();
        let (output, check) = budget.truncate(&mixed).await;

        // Should NOT panic. Output should be truncated.
        assert!(check.truncated || output.len() < mixed.len(),
            "Truncation should have occurred");
        // Output must be valid UTF-8 (no panic = it's safe)
        assert!(std::str::from_utf8(output.as_bytes()).is_ok(),
            "Output must be valid UTF-8");
    }

    #[tokio::test]
    async fn test_truncate_all_utf8_characters() {
        // Pure multi-byte content longer than max_chars (50K)
        // Each '€' is 3 bytes but 1 char; 55_000 € chars
        let large: String = "€".repeat(55_000); // 55K chars

        let budget = ResultBudget::new();
        let (output, check) = budget.truncate(&large).await;

        // Should truncate
        assert!(check.truncated, "Should have been truncated");
        assert!(std::str::from_utf8(output.as_bytes()).is_ok(),
            "Output must be valid UTF-8");
        assert!(
            output.contains("truncated"),
            "truncation notice should mention truncated"
        );
    }
}