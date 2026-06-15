use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use crate::AppState;
use crate::pipeline::plan::{PlanAgent, StructuredPlan, PLAN_SYSTEM_PROMPT};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanGeneratedPayload {
    pub task_id: String,
    pub plan: StructuredPlan,
    pub raw_output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTokenPayload {
    pub task_id: String,
    pub token: String,
    pub done: bool,
}

#[tauri::command]
pub async fn generate_plan(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    task: String,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4().to_string();
    log::info!("generate_plan: task_id={}", task_id);

    // Update pipeline state
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
    let result = agent.generate_streaming(&state, &task).await;

    match result {
        Ok((plan, raw_output)) => {
            // Save plan to pipeline state
            {
                let mut pipeline = state.pipeline.lock().await;
                pipeline.plan = Some(raw_output.clone());
                pipeline.structured_plan = Some(plan.clone());
                pipeline.status = crate::pipeline::PipelineStatus::Idle;
            }

            // Emit plan-generated event
            let _ = app_handle.emit("plan-generated", PlanGeneratedPayload {
                task_id: task_id.clone(),
                plan,
                raw_output,
            });

            Ok(task_id)
        }
        Err(e) => {
            let mut pipeline = state.pipeline.lock().await;
            pipeline.status = crate::pipeline::PipelineStatus::Failed(e.clone());
            let _ = app_handle.emit("plan-error", serde_json::json!({
                "task_id": task_id,
                "error": e,
            }));
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn get_plan(
    state: State<'_, AppState>,
) -> Result<Option<StructuredPlan>, String> {
    let pipeline = state.pipeline.lock().await;
    Ok(pipeline.structured_plan.clone())
}

#[tauri::command]
pub async fn approve_plan(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut pipeline = state.pipeline.lock().await;
    if pipeline.structured_plan.is_none() {
        return Err("No plan to approve".into());
    }
    pipeline.plan_approved = true;
    Ok("Plan approved".into())
}

#[tauri::command]
pub async fn get_plan_system_prompt() -> Result<String, String> {
    Ok(PLAN_SYSTEM_PROMPT.to_string())
}
