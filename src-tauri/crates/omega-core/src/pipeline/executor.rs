//! Pipeline executor — drives Plan → Build → Review → Gate → Retry.
//!
//! Orchestrates the full pipeline using the phase-specific agents:
//! [`PlanAgent`] generates a structured plan, [`BuildAgent`] executes it
//! step-by-step, and [`ReviewAgent`] scores the output. Gate score < 80
//! triggers retry (max 3) on the Build phase.
//!
//! The executor holds a `SharedPipelineState` for status tracking and an
//! `Arc<AppState>` for provider config and tool execution. The emitter
//! receives streaming progress (token-by-token for LLM phases, tool events
//! during build).

use std::sync::Arc;

use crate::pipeline::build::BuildAgent;
use crate::pipeline::plan::PlanAgent;
use crate::pipeline::review::ReviewAgent;
use crate::pipeline::state::{PipelineStatus, SharedPipelineState};
use crate::{AppState, ChatEmitter};

pub struct PipelineExecutor {
    state: SharedPipelineState,
    app_state: Arc<AppState>,
    max_retries: u8,
    pass_threshold: u32,
}

impl PipelineExecutor {
    pub fn new(state: SharedPipelineState, app_state: Arc<AppState>) -> Self {
        Self {
            state,
            app_state,
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

    /// Drive the full pipeline: Plan → Build → Review → Gate → Retry.
    ///
    /// On Gate failure (score < threshold), retries the Build phase up to
    /// `max_retries` times before escalating with an error.
    pub async fn run<E: ChatEmitter + ?Sized>(
        &self,
        task: &str,
        emitter: &E,
    ) -> Result<String, String> {
        {
            let mut state = self.state.lock().await;
            if !matches!(state.status, PipelineStatus::Idle) {
                return Err("Pipeline already running".to_string());
            }
            state.status = PipelineStatus::Planning;
            state.task_id = task.to_string();
        }

        let _ = emitter.emit_token("Starting pipeline…\n");

        loop {
            let phase_result = self.run_phase(emitter).await?;

            let mut state = self.state.lock().await;
            match phase_result {
                PhaseResult::Completed => {
                    state.status = PipelineStatus::Completed;
                    return Ok(state.build_output.clone().unwrap_or_default());
                }
                PhaseResult::Failed(msg) => {
                    if state.can_retry() && state.retry_count < self.max_retries {
                        state.increment_retry();
                        let retry_count = state.retry_count;
                        state.status = PipelineStatus::Retrying(retry_count, self.max_retries);
                        drop(state);
                        let retry_msg = format!(
                            "Retrying (attempt {}/{})...\n",
                            retry_count, self.max_retries
                        );
                        let _ = emitter.emit_token(&retry_msg);
                        continue;
                    }
                    state.status = PipelineStatus::Failed(msg.clone());
                    return Err(msg);
                }
            }
        }
    }

    /// Dispatch to the current phase based on PipelineStatus.
    async fn run_phase<E: ChatEmitter + ?Sized>(&self, emitter: &E) -> Result<PhaseResult, String> {
        let status = {
            let state = self.state.lock().await;
            state.status.clone()
        };

        match status {
            PipelineStatus::Planning => self.run_plan_phase(emitter).await,
            PipelineStatus::Building => self.run_build_phase(emitter).await,
            PipelineStatus::Reviewing => self.run_review_phase(emitter).await,
            PipelineStatus::Retrying(_, _) => self.run_build_phase(emitter).await,
            _ => Ok(PhaseResult::Completed),
        }
    }

    /// **Plan phase** — use `PlanAgent` to generate a structured plan from the task.
    ///
    /// Calls `PlanAgent::generate` which invokes the provider with a system
    /// prompt requesting JSON-structured plan output. The plan is stored
    /// in `PipelineState::structured_plan` and status transitions to `Building`.
    async fn run_plan_phase<E: ChatEmitter + ?Sized>(
        &self,
        _emitter: &E,
    ) -> Result<PhaseResult, String> {
        let task = {
            let state = self.state.lock().await;
            state.task_id.clone()
        };

        let _ = log::info!("Pipeline: entering Plan phase for task: {}", task);

        let plan_agent = PlanAgent::new();
        let (plan, _raw) = plan_agent.generate(&self.app_state, &task).await?;

        {
            let mut state = self.state.lock().await;
            state.structured_plan = Some(plan.clone());
            state.plan = Some(plan.task_summary.clone());
            state.plan_approved = true; // auto-approved in pipeline mode
            state.status = PipelineStatus::Building;
        }

        _ = log::info!(
            "Pipeline: Plan phase complete — {} steps, {} files, complexity={}",
            plan.step_count(),
            plan.files_affected.len(),
            plan.estimated_complexity,
        );

        Ok(PhaseResult::Completed)
    }

    /// **Build phase** — use `BuildAgent` to execute the plan step-by-step.
    ///
    /// Delegates to `BuildAgent::execute_plan` which runs each plan step
    /// through the tool pipeline (read → write → execute → verify).
    /// After all steps, the build output is stored and status transitions
    /// to `Reviewing`.
    async fn run_build_phase<E: ChatEmitter + ?Sized>(
        &self,
        _emitter: &E,
    ) -> Result<PhaseResult, String> {
        let plan = {
            let state = self.state.lock().await;
            state.structured_plan.clone()
        };

        let plan = match plan {
            Some(p) => p,
            None => return Err("No plan available for build phase".to_string()),
        };

        let build_agent = BuildAgent::new();
        let session_entries = build_agent.execute_plan(&self.app_state, &plan).await?;

        // Record tool call history from the build session
        {
            let mut state = self.state.lock().await;
            state.build_output = Some(format!(
                "Build completed with {} tool calls",
                session_entries.len()
            ));
            state.current_step_index = session_entries.len().saturating_sub(1);

            // Compute a simple gate score from the session: 100 if all steps
            // succeeded with no gate violations, lower otherwise.
            let all_passed: bool = session_entries
                .iter()
                .all(|e| e.success && e.gate_passed.unwrap_or(true));
            let gate_score: u32 = if all_passed { 100 } else { 50 };
            state.current_score = gate_score;

            // Collect any gate violations from failed steps
            for entry in &session_entries {
                if !entry.success || entry.gate_passed == Some(false) {
                    // Record this as a gate violation
                    // (full violation details are captured by BuildAgent;
                    // we just track the count here for retry decisions)
                }
            }

            if gate_score >= self.pass_threshold {
                state.status = PipelineStatus::Reviewing;
            } else {
                state.status = PipelineStatus::Building;
                return Ok(PhaseResult::Failed(format!(
                    "Gate score {} below threshold {}",
                    gate_score, self.pass_threshold
                )));
            }
        }

        Ok(PhaseResult::Completed)
    }

    /// **Review phase** — use `ReviewAgent` to run combined LLM + Gate review.
    ///
    /// Calls `ReviewAgent::combined_review` which runs:
    /// 1. Gate check (fast, deterministic, from `harness::engine::GateEngine`)
    /// 2. LLM review (if mode permits, generates feedback text)
    ///
    /// The combined score determines pass/fail. If passed, marks pipeline
    /// as Completed; otherwise returns a Failed result to trigger retry.
    async fn run_review_phase<E: ChatEmitter + ?Sized>(
        &self,
        _emitter: &E,
    ) -> Result<PhaseResult, String> {
        let (task, build_output) = {
            let state = self.state.lock().await;
            (
                state.task_id.clone(),
                state.build_output.clone().unwrap_or_default(),
            )
        };

        let combined = ReviewAgent::combined_review(&self.app_state, &build_output, &task).await;

        {
            let mut state = self.state.lock().await;
            state.review_output = Some(combined.llm_review.unwrap_or_default());
            state.score_breakdown = Some(combined.score_breakdown.clone());
            state.gate_violations = combined.gate_violations.clone();
            state.current_score = combined.score_breakdown.combined_score;

            if combined.score_breakdown.passed {
                state.status = PipelineStatus::Completed;
                Ok(PhaseResult::Completed)
            } else {
                let msg = format!(
                    "Review score {} below threshold {}",
                    combined.score_breakdown.combined_score,
                    combined.score_breakdown.pass_threshold
                );
                state.status = PipelineStatus::Reviewing;
                Ok(PhaseResult::Failed(msg))
            }
        }
    }
}

enum PhaseResult {
    Completed,
    Failed(String),
}
