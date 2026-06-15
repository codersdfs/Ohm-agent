use serde::{Deserialize, Serialize};
use tauri::State;
use crate::AppState;
use crate::pipeline::review::{ReviewAgent, CombinedReviewOutput};
use crate::pipeline::review_score::{ScoreBreakdown, PromotionStats};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub code: String,
    pub context: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScoreResponse {
    pub score_breakdown: Option<ScoreBreakdown>,
    pub promotion_stats: Option<PromotionStats>,
    pub retry_count: u8,
    pub max_retries: u8,
    pub pipeline_status: String,
}

#[tauri::command]
pub async fn run_review(
    state: State<'_, AppState>,
    request: ReviewRequest,
) -> Result<CombinedReviewOutput, String> {
    log::info!("run_review: code_len={}, context={:?}", request.code.len(), request.context.chars().take(50).collect::<String>());

    // Update pipeline status
    {
        let mut p = state.pipeline.lock().await;
        p.status = crate::pipeline::PipelineStatus::Reviewing;
    }

    let output = ReviewAgent::combined_review(&state, &request.code, &request.context).await;

    // Auto-promote violations
    if !output.gate_violations.is_empty() {
        let mut db = state.rules_db.lock().unwrap();
        let lang = state.detected_language.lock().unwrap().clone();
        for v in &output.gate_violations {
            let cat = v.category.to_lowercase();
            if let Some(pattern) = v.message.rsplit(": ").next() {
                db.promote_or_increment(&lang, &cat, pattern, &v.message, "error");
            }
        }
    }

    // Demote stale rules
    let demoted = ReviewAgent::demote_stale_rules(&state);
    let promo_stats = ReviewAgent::get_promotion_stats(&state);

    // Update pipeline state
    {
        let mut p = state.pipeline.lock().await;
        p.gate_violations = output.gate_violations.clone();
        p.review_output = output.llm_review.clone();
        p.current_score = output.score_breakdown.combined_score;
        p.score_breakdown = Some(output.score_breakdown.clone());
        p.promotion_stats = Some(PromotionStats {
            demoted_last_run: demoted,
            ..promo_stats
        });

        // Retry decision
        if !output.score_breakdown.passed && p.can_retry() {
            p.increment_retry();
            p.status = crate::pipeline::PipelineStatus::Retrying(p.retry_count, p.max_retries);
        } else if output.score_breakdown.passed {
            p.status = crate::pipeline::PipelineStatus::Completed;
        } else {
            p.status = crate::pipeline::PipelineStatus::Failed("Max retries exceeded".into());
        }
    }

    Ok(output)
}

#[tauri::command]
pub async fn get_score_breakdown(
    state: State<'_, AppState>,
) -> Result<ScoreResponse, String> {
    let p = state.pipeline.lock().await;
    Ok(ScoreResponse {
        score_breakdown: p.score_breakdown.clone(),
        promotion_stats: p.promotion_stats.clone(),
        retry_count: p.retry_count,
        max_retries: p.max_retries,
        pipeline_status: format!("{:?}", p.status),
    })
}

#[tauri::command]
pub async fn get_promotion_stats(
    state: State<'_, AppState>,
) -> Result<PromotionStats, String> {
    let stats = ReviewAgent::get_promotion_stats(&state);
    Ok(stats)
}

#[tauri::command]
pub async fn demote_stale_rules(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let count = ReviewAgent::demote_stale_rules(&state);
    Ok(count)
}

#[tauri::command]
pub async fn reset_retry_count(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut p = state.pipeline.lock().await;
    p.retry_count = 0;
    Ok("Retry count reset".into())
}
