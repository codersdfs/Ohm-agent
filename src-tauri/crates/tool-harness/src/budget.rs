// Result and context budgeting

use std::path::PathBuf;

const MAX_RESULT_CHARS: usize = 30_000;
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
        if trimmed.len() > self.max_chars {
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
        if trimmed.len() > self.max_chars {
            let prefix = &trimmed[..self.max_chars];
            let mut result = String::new();
            result.push_str(prefix);
            result.push_str("\n\n[Output truncated. Full result persisted to file.]");

            let check = self.check_and_process(trimmed.to_string(), "").await;
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
        assert_eq!(budget.max_chars, 30_000);
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
        let large = "x".repeat(35_000);
        let (output, check) = budget.truncate(&large).await;
        assert!(check.truncated || output.len() < large.len());
    }
}