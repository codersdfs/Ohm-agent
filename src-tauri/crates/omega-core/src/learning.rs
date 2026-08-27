use crate::AppState;
use harness::negative_knowledge::{compute_signature, normalize_message, NegativeKnowledgeStore};

/// Learning module — manages negative knowledge injection into system prompt.
pub struct LearningModule;

impl LearningModule {
    /// Record a failure event and check if it should be promoted.
    pub fn record_failure(
        state: &AppState,
        raw_message: &str,
        kind: &str,
        fix_recipe: Option<String>,
    ) -> Result<bool, String> {
        let db_path = format!("{}/.omega/negative_knowledge.sqlite", state.db_path);
        let store = NegativeKnowledgeStore::new(&db_path)?;
        let normalized = normalize_message(raw_message);
        let signature = compute_signature(&normalized);

        let event = harness::negative_knowledge::FailureEvent {
            signature,
            raw: raw_message.to_string(),
            kind: kind.to_string(),
            fix_recipe,
            count: 0,
        };

        store.record_failure(&event)
    }

    /// Get promoted rules to inject into system prompt (max 2000 chars).
    pub fn get_prompt_rules(state: &AppState) -> String {
        let db_path = format!("{}/.omega/negative_knowledge.sqlite", state.db_path);
        if let Ok(store) = NegativeKnowledgeStore::new(&db_path) {
            if let Ok(rules) = store.get_promoted_rules() {
                let mut prompt = String::from("\n# Promoted Rules (from negative knowledge):\n");
                for rule in rules.iter().take(10) {
                    prompt.push_str(&format!("- {} (freq: {})\n", rule.message, rule.frequency));
                }
                if prompt.len() > 2000 {
                    prompt.truncate(2000);
                }
                return prompt;
            }
        }
        String::new()
    }

    /// Get statistics for the `omega gate stats` command.
    pub fn get_stats(state: &AppState) -> Result<serde_json::Value, String> {
        let db_path = format!("{}/.omega/negative_knowledge.sqlite", state.db_path);
        let store = NegativeKnowledgeStore::new(&db_path)?;
        let stats = store.get_stats()?;

        Ok(serde_json::json!({
            "total_patterns": stats.total_patterns,
            "promoted": stats.promoted,
            "recurrence_rate_before": stats.recurrence_rate_before,
            "recurrence_rate_after": stats.recurrence_rate_after,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_message_works() {
        let result = normalize_message("src/main.rs:42: error: test");
        assert!(!result.contains("src/main.rs"));
    }

    #[test]
    fn compute_signature_is_deterministic() {
        let sig1 = compute_signature("test message");
        let sig2 = compute_signature("test message");
        assert_eq!(sig1, sig2);
    }
}
