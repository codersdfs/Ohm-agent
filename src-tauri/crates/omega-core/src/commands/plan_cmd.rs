use crate::pipeline::plan::{PlanAgent, StructuredPlan, PLAN_SYSTEM_PROMPT};
use crate::AppState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanGeneratedPayload {
    pub task_id: String,
    pub plan: StructuredPlan,
    pub raw_output: String,
}

pub async fn generate_plan(state: &AppState, task: String) -> Result<PlanGeneratedPayload, String> {
    let task_id = uuid::Uuid::new_v4().to_string();
    log::info!("generate_plan: task_id={}", task_id);

    {
        let mut pipeline = state.pipeline.lock().await;
        pipeline.status = crate::pipeline::PipelineStatus::Planning;
        pipeline.task_id = task_id.clone();
        pipeline.plan = None;
        pipeline.structured_plan = None;
        pipeline.plan_approved = false;
        pipeline.current_step_index = 0;
    }

    let agent = PlanAgent::new();
    let result = agent.generate_streaming(state, &task).await;

    match result {
        Ok((plan, raw_output)) => {
            {
                let mut pipeline = state.pipeline.lock().await;
                pipeline.plan = Some(raw_output.clone());
                pipeline.structured_plan = Some(plan.clone());
                pipeline.status = crate::pipeline::PipelineStatus::Idle;
            }
            Ok(PlanGeneratedPayload {
                task_id,
                plan,
                raw_output,
            })
        }
        Err(e) => {
            let mut pipeline = state.pipeline.lock().await;
            pipeline.status = crate::pipeline::PipelineStatus::Failed(e.clone());
            Err(e)
        }
    }
}

pub async fn get_plan(state: &AppState) -> Result<Option<StructuredPlan>, String> {
    let pipeline = state.pipeline.lock().await;
    Ok(pipeline.structured_plan.clone())
}

pub async fn approve_plan(state: &AppState) -> Result<String, String> {
    let mut pipeline = state.pipeline.lock().await;
    if pipeline.structured_plan.is_none() {
        return Err("No plan to approve".into());
    }
    pipeline.plan_approved = true;
    Ok("Plan approved".into())
}

pub fn get_plan_system_prompt() -> Result<String, String> {
    Ok(PLAN_SYSTEM_PROMPT.to_string())
}
