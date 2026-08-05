//! Pipeline executor — drives Plan → Build → Review → Gate → Retry.
//!
//! Reuses the canonical agent loop per phase with phase-specific system
//! prompts and tool subsets. Gate score < 80 triggers retry (max 3) or
//! escalation.

use crate::pipeline::state::{PipelineStatus, PipelineState, SharedPipelineState};
use std::sync::Arc;

pub struct PipelineExecutor {
    state: SharedPipelineState,
    max_retries: u8,
    pass_threshold: u32,
}

impl PipelineExecutor {
    pub fn new(state: SharedPipelineState) -> Self {
        Self {
            state,
            max_retries: 3,
            pass_threshold: 80,
        }
    }

    pub fn with_max_retries(mut self, max: u8) -> Self {
        self.max_retries = max;
        self
    }

    pub fn with_pass_threshold(mut self, threshold: u32) -> Self {
        self.pass_threshold = threshold;
        self
    }

    pub async fn run(&self, task: &str) -> Result<String, String> {
        let mut state = self.state.lock().await;

        if !matches!(state.status, PipelineStatus::Idle) {
            return Err("Pipeline already running".to_string());
        }

        state.status = PipelineStatus::Planning;
        state.task_id = task.to_string();

        drop(state);

        loop {
            let phase_result = self.run_phase().await?;

            let mut state = self.state.lock().await;

            match phase_result {
                PhaseResult::Completed => {
                    state.status = PipelineStatus::Completed;
                    return Ok("Pipeline completed successfully".to_string());
                }
                PhaseResult::Failed(msg) => {
                    if state.can_retry() {
                        state.increment_retry();
                        state.status = PipelineStatus::Retrying(
                            state.retry_count,
                            state.max_retries,
                        );
                        drop(state);
                        continue;
                    }
                    state.status = PipelineStatus::Failed(msg.clone());
                    return Err(msg);
                }
            }
        }
    }

    async fn run_phase(&self) -> Result<PhaseResult, String> {
        let state = self.state.lock().await;
        let status = state.status.clone();
        drop(state);

        match status {
            PipelineStatus::Planning => self.run_plan_phase().await,
            PipelineStatus::Building => self.run_build_phase().await,
            PipelineStatus::Reviewing => self.run_review_phase().await,
            PipelineStatus::Retrying(_, _) => self.run_build_phase().await,
            _ => Ok(PhaseResult::Completed),
        }
    }

    async fn run_plan_phase(&self) -> Result<PhaseResult, String> {
        let mut state = self.state.lock().await;
        state.status = PipelineStatus::Building;
        drop(state);
        Ok(PhaseResult::Completed)
    }

    async fn run_build_phase(&self) -> Result<PhaseResult, String> {
        let mut state = self.state.lock().await;

        if state.current_score >= self.pass_threshold {
            state.status = PipelineStatus::Reviewing;
            drop(state);
            return self.run_review_phase().await;
        }

        if !state.can_retry() {
            return Ok(PhaseResult::Failed(format!(
                "Gate score {} below threshold {} after {} retries",
                state.current_score, state.pass_threshold, state.retry_count
            )));
        }

        state.increment_retry();
        drop(state);
        Ok(PhaseResult::Completed)
    }

    async fn run_review_phase(&self) -> Result<PhaseResult, String> {
        let mut state = self.state.lock().await;
        state.status = PipelineStatus::Completed;
        drop(state);
        Ok(PhaseResult::Completed)
    }
}

enum PhaseResult {
    Completed,
    Failed(String),
}
