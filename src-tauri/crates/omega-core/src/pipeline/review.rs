use crate::commands::tools::GateViolationInfo;
use crate::pipeline::review_score::{aggregate_scores, PromotionStats, ScoreBreakdown};
use crate::{AppState, MutexExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedReviewOutput {
    pub gate_violations: Vec<GateViolationInfo>,
    pub llm_review: Option<String>,
    pub score_breakdown: ScoreBreakdown,
}

pub struct ReviewAgent;

impl ReviewAgent {
    pub fn new() -> Self {
        Self
    }

    /// Run Gate check (always on, synchronous, fast).
    pub fn gate_check(state: &AppState, content: &str) -> Vec<GateViolationInfo> {
        let db = state.rules_db.lock_guard();
        let lang = state.detected_language.lock_guard().clone();
        let violations = db.check_content(content, &lang);

        if violations.is_empty() {
            return vec![];
        }

        violations
            .iter()
            .map(|v| GateViolationInfo {
                category: format!("{:?}", v.category),
                message: v.message.clone(),
                tool_hint: v.tool_hint.clone(),
                line: v.line,
            })
            .collect()
    }

    /// Run LLM review (togglable).
    pub async fn llm_review(state: &AppState, code: &str, context: &str) -> Result<String, String> {
        let config = state.provider_config.lock_guard().clone();
        let review_prompt = format!(
            "You are a Code Review agent. Analyze this code for:\n\
            1. Logic errors and bugs\n\
            2. Missing error handling\n\
            3. Performance issues\n\
            4. Security vulnerabilities\n\
            5. Architectural problems\n\n\
            Context: {}\n\n\
            Code:\n```\n{}\n```\n\n\
            Provide specific, actionable feedback. Use 'Error:' prefix for critical issues, 'Warning:' prefix for recommendations.",
            context, code
        );

        let provider = providers::create_provider(&config)?;
        let messages = vec![providers::ChatMessage {
            role: "user".into(),
            content: review_prompt,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];

        let chat_request = providers::ChatRequest {
            messages,
            config,
            stream: false,
            tools: None,
        };

        let response = provider.chat(chat_request).await?;
        Ok(response.content)
    }

    /// Combined review: Gate + LLM (if mode permits) with score aggregation.
    pub async fn combined_review(
        state: &AppState,
        code: &str,
        context: &str,
    ) -> CombinedReviewOutput {
        let gate_violations = Self::gate_check(state, code);

        // Convert to harness violations for scoring
        let har_violations: Vec<harness::Violation> = gate_violations
            .iter()
            .map(|v| {
                let cat = match v.category.to_lowercase().as_str() {
                    "structural" => harness::ViolationCategory::Structural,
                    "taste" => harness::ViolationCategory::Taste,
                    "golden" => harness::ViolationCategory::Golden,
                    "repeated" => harness::ViolationCategory::Repeated,
                    _ => harness::ViolationCategory::Structural,
                };
                harness::Violation {
                    category: cat,
                    message: v.message.clone(),
                    tool_hint: v.tool_hint.clone(),
                    line: v.line,
                }
            })
            .collect();

        let gate_result = harness::scoring::calculate_score(&har_violations);

        let config = state.review_config.lock_guard().clone();
        let pass_threshold = 80;

        let (llm_review, llm_review_str) = match config.mode {
            crate::pipeline::ReviewMode::Off => (None, None),
            _ => match Self::llm_review(state, code, context).await {
                Ok(review) if review.len() > 50 => (Some(review.clone()), Some(review)),
                Ok(_) => (None, None),
                Err(e) => (Some(format!("LLM review failed: {}", e)), None),
            },
        };

        let score_breakdown = aggregate_scores(
            gate_result.score,
            llm_review_str.as_deref(),
            &gate_violations,
            pass_threshold,
        );

        CombinedReviewOutput {
            gate_violations,
            llm_review,
            score_breakdown,
        }
    }

    /// Get promotion statistics from the rules database.
    pub fn get_promotion_stats(state: &AppState) -> PromotionStats {
        let db = state.rules_db.lock_guard();
        let lang = state.detected_language.lock_guard().clone();
        let group = db.load_for_language(&lang);

        let all: Vec<_> = group.all_rules();
        let total = all.len();
        let promoted = all.iter().filter(|(r, _)| r.promoted).count();
        let f1 = all.iter().filter(|(r, _)| r.frequency == 1).count();
        let f2 = all.iter().filter(|(r, _)| r.frequency == 2).count();
        let f3p = all.iter().filter(|(r, _)| r.frequency >= 3).count();

        PromotionStats {
            total_patterns: total,
            promoted,
            frequency_1: f1,
            frequency_2: f2,
            frequency_3_plus: f3p,
            demoted_last_run: 0,
        }
    }

    /// Demote stale rules that haven't been triggered in many sessions.
    pub fn demote_stale_rules(state: &AppState) -> usize {
        let mut db = state.rules_db.lock_guard();
        let lang = state.detected_language.lock_guard().clone();
        db.demote_stale_rules(&lang)
    }
}
