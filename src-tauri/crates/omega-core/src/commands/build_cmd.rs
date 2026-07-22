use crate::pipeline::build::{BuildAgent, BuildSessionEntry};
use crate::{AppState, MutexExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildConfigResponse {
    pub auto_approve: bool,
}

pub async fn execute_build(state: &AppState) -> Result<Vec<BuildSessionEntry>, String> {
    log::info!("execute_build: starting build pipeline");

    // Hard guard at the command boundary as well as inside BuildAgent.
    if !crate::pipeline::build::experimental_pipeline_enabled() {
        return Err(format!(
            "Build pipeline is experimental and disabled by default. \
Use the chat agent for coding, or set {}=1 to enable. See ROADMAP.md P0-02 / P2-04.",
            crate::pipeline::build::EXPERIMENTAL_PIPELINE_ENV
        ));
    }

    let plan = {
        let p = state.pipeline.lock().await;
        p.structured_plan.clone().ok_or_else(|| {
            "No plan has been generated. Generate and approve a plan first.".to_string()
        })?
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
    log::info!(
        "respond_permission: id={}, approved={}",
        request_id,
        approved
    );

    state
        .permission_results
        .lock_guard()
        .insert(request_id.clone(), approved);
    state.pending_permissions.lock_guard().remove(&request_id);

    Ok(if approved { "Approved" } else { "Denied" }.to_string())
}

pub async fn get_build_session(state: &AppState) -> Result<Vec<BuildSessionEntry>, String> {
    let log = state.session_log.lock_guard();
    Ok(log.clone())
}

pub async fn get_build_config(state: &AppState) -> Result<BuildConfigResponse, String> {
    let config = state.build_config.lock_guard();
    Ok(BuildConfigResponse {
        auto_approve: config.auto_approve,
    })
}

pub async fn set_build_config(state: &AppState, auto_approve: bool) -> Result<String, String> {
    let mut config = state.build_config.lock_guard();
    config.auto_approve = auto_approve;
    Ok(format!(
        "Build config updated: auto_approve={}",
        auto_approve
    ))
}
