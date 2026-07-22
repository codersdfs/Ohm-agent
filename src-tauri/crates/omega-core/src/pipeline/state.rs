use crate::commands::tools::GateCheckResult;
use crate::pipeline::plan::StructuredPlan;
use crate::pipeline::review_score::{PromotionStats, ScoreBreakdown};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub auto_approve: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            auto_approve: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentType {
    Plan,
    Build,
    Review,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineStatus {
    Idle,
    Planning,
    Building,
    Reviewing,
    Retrying(u8, u8),
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReviewMode {
    Off,
    Summary,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConfig {
    pub mode: ReviewMode,
    pub max_retries: u8,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            mode: ReviewMode::Summary,
            max_retries: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: Option<GateCheckResult>,
    pub retry_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    pub task_id: String,
    pub status: PipelineStatus,
    pub retry_count: u8,
    pub max_retries: u8,
    pub current_score: u32,
    pub pass_threshold: u32,
    pub tools_called: Vec<ToolCallRecord>,
    pub gate_violations: Vec<crate::commands::tools::GateViolationInfo>,
    pub plan: Option<String>,
    pub structured_plan: Option<StructuredPlan>,
    pub plan_approved: bool,
    pub current_step_index: usize,
    pub build_output: Option<String>,
    pub review_output: Option<String>,
    pub score_breakdown: Option<ScoreBreakdown>,
    pub promotion_stats: Option<PromotionStats>,
}

impl PipelineState {
    pub fn new(task_id: String) -> Self {
        Self {
            task_id,
            status: PipelineStatus::Idle,
            retry_count: 0,
            max_retries: 3,
            current_score: 0,
            pass_threshold: 80,
            tools_called: vec![],
            gate_violations: vec![],
            plan: None,
            structured_plan: None,
            plan_approved: false,
            current_step_index: 0,
            build_output: None,
            review_output: None,
            score_breakdown: None,
            promotion_stats: None,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn reset_for_new_task(&mut self) {
        self.status = PipelineStatus::Idle;
        self.retry_count = 0;
        self.current_score = 0;
        self.tools_called.clear();
        self.gate_violations.clear();
        self.plan = None;
        self.structured_plan = None;
        self.plan_approved = false;
        self.current_step_index = 0;
        self.build_output = None;
        self.review_output = None;
        self.score_breakdown = None;
        self.promotion_stats = None;
    }
}

pub type SharedPipelineState = Arc<Mutex<PipelineState>>;
