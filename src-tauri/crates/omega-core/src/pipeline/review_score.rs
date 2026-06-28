use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationBreakdown {
    pub category: String,
    pub count: u32,
    pub penalty: u32,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmReviewIssue {
    pub category: String,
    pub severity: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub gate_score: u32,
    pub llm_score: Option<u32>,
    pub combined_score: u32,
    pub gate_penalties: Vec<ViolationBreakdown>,
    pub llm_issues: Vec<LlmReviewIssue>,
    pub passed: bool,
    pub pass_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionStats {
    pub total_patterns: usize,
    pub promoted: usize,
    pub frequency_1: usize,
    pub frequency_2: usize,
    pub frequency_3_plus: usize,
    pub demoted_last_run: usize,
}

const LLM_BASE_SCORE: u32 = 100;
const LLM_ERROR_PENALTY: u32 = 20;
const LLM_WARN_PENALTY: u32 = 10;

/// Parse LLM review output into structured issues.
pub fn parse_llm_review(review: &str) -> Vec<LlmReviewIssue> {
    let mut issues = vec![];

    for line in review.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("```") {
            continue;
        }

        // Classify severity
        let lower = trimmed.to_lowercase();
        let severity = if lower.starts_with("error") || lower.contains("critical") || lower.contains("vulnerability") || lower.contains("security") {
            "error"
        } else if lower.starts_with("warn") || lower.starts_with("caution") || lower.contains("should") || lower.contains("consider") || lower.contains("recommend") {
            "warn"
        } else {
            continue;
        };

        // Classify category
        let category = if lower.contains("bug") || lower.contains("logic") || lower.contains("incorrect") {
            "logic"
        } else if lower.contains("error") || lower.contains("handling") || lower.contains("panic") || lower.contains("unwrap") {
            "error_handling"
        } else if lower.contains("perform") || lower.contains("slow") || lower.contains("inefficient") || lower.contains("optim") {
            "performance"
        } else if lower.contains("security") || lower.contains("vulnerability") || lower.contains("injection") || lower.contains("xss") {
            "security"
        } else if lower.contains("architect") || lower.contains("design") || lower.contains("coupling") || lower.contains("structure") {
            "architecture"
        } else {
            "general"
        };

        issues.push(LlmReviewIssue {
            category: category.to_string(),
            severity: severity.to_string(),
            description: trimmed.to_string(),
        });
    }

    issues
}

/// Score LLM review output. Returns (score, issues).
pub fn score_llm_review(review: &str) -> (u32, Vec<LlmReviewIssue>) {
    let issues = parse_llm_review(review);
    let mut score = LLM_BASE_SCORE;

    for issue in &issues {
        match issue.severity.as_str() {
            "error" => score = score.saturating_sub(LLM_ERROR_PENALTY),
            "warn" => score = score.saturating_sub(LLM_WARN_PENALTY),
            _ => {}
        }
    }

    (score, issues)
}

/// Aggregate Gate score + LLM review score into a combined score.
pub fn aggregate_scores(
    gate_score: u32,
    llm_review: Option<&str>,
    gate_violations: &[crate::commands::tools::GateViolationInfo],
    pass_threshold: u32,
) -> ScoreBreakdown {
    let mut gate_penalties: Vec<ViolationBreakdown> = vec![];

    // Group gate violations by category
    if !gate_violations.is_empty() {
        use std::collections::HashMap;
        let mut by_cat: HashMap<String, Vec<String>> = HashMap::new();
        for v in gate_violations {
            by_cat.entry(v.category.clone())
                .or_insert_with(Vec::new)
                .push(v.message.clone());
        }
        for (cat, msgs) in by_cat {
            let count = msgs.len() as u32;
            let penalty = match cat.to_lowercase().as_str() {
                "structural" => count * 15,
                "taste" => count * 10,
                "golden" => count * 20,
                "repeated" => count * 25,
                _ => count * 10,
            };
            gate_penalties.push(ViolationBreakdown {
                category: cat,
                count,
                penalty,
                messages: msgs,
            });
        }
    }

    let (llm_score, llm_issues) = match llm_review {
        Some(review) if review.len() > 50 => {
            let (s, issues) = score_llm_review(review);
            (Some(s), issues)
        }
        _ => (None, vec![]),
    };

    let combined = match llm_score {
        Some(ls) => {
            // Weighted: 60% gate + 40% llm
            (gate_score as f64 * 0.6 + ls as f64 * 0.4).round() as u32
        }
        None => gate_score,
    };

    let passed = combined >= pass_threshold;

    ScoreBreakdown {
        gate_score,
        llm_score,
        combined_score: combined,
        gate_penalties,
        llm_issues,
        passed,
        pass_threshold,
    }
}
