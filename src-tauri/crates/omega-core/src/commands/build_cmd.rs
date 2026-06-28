use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::pipeline::build::{BuildAgent, BuildSessionEntry};

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildConfigResponse {
    pub auto_approve: bool,
}

pub async fn execute_build(
    state: &AppState,
) -> Result<Vec<BuildSessionEntry>, String> {
    log::info!("execute_build: starting build pipeline");

    let plan = {
        let p = state.pipeline.lock().await;
        p.structured_plan.clone().ok_or_else(|| "No plan has been generated. Generate and approve a plan first.".to_string())?
    };

    if !state.pipeline.lock().await.plan_approved {
        return Err("Plan has not been approved. Approve the plan before building.".to_string());
    }

    let agent = BuildAgent::new();
    let session = agent.execute_plan(state, &plan).await?;

    Ok(session)
}

pub async fn respond_permission(
    state: &AppState,
    request_id: String,
    approved: bool,
) -> Result<String, String> {
    log::info!("respond_permission: id={}, approved={}", request_id, approved);

    state.permission_results.lock().unwrap().insert(request_id.clone(), approved);
    state.pending_permissions.lock().unwrap().remove(&request_id);

    Ok(if approved { "Approved" } else { "Denied" }.to_string())
}

pub async fn get_build_session(
    state: &AppState,
) -> Result<Vec<BuildSessionEntry>, String> {
    let log = state.session_log.lock().unwrap();
    Ok(log.clone())
}

pub async fn get_build_config(
    state: &AppState,
) -> Result<BuildConfigResponse, String> {
    let config = state.build_config.lock().unwrap();
    Ok(BuildConfigResponse { auto_approve: config.auto_approve })
}

pub async fn set_build_config(
    state: &AppState,
    auto_approve: bool,
) -> Result<String, String> {
    let mut config = state.build_config.lock().unwrap();
    config.auto_approve = auto_approve;
    Ok(format!("Build config updated: auto_approve={}", auto_approve))
}
