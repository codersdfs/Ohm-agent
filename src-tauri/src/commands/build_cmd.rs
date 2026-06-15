use serde::{Deserialize, Serialize};
use tauri::State;
use crate::AppState;
use crate::pipeline::build::{BuildAgent, BuildSessionEntry, PermissionRequest};

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildConfigResponse {
    pub auto_approve: bool,
}

#[tauri::command]
pub async fn execute_build(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
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
    let session = agent.execute_plan(&state, &plan, &app_handle).await?;

    Ok(session)
}

#[tauri::command]
pub async fn respond_permission(
    state: State<'_, AppState>,
    request_id: String,
    approved: bool,
) -> Result<String, String> {
    log::info!("respond_permission: id={}, approved={}", request_id, approved);

    // Store the response
    state.permission_results.lock().unwrap().insert(request_id.clone(), approved);

    // Remove from pending set to signal the BuildAgent
    state.pending_permissions.lock().unwrap().remove(&request_id);

    Ok(if approved { "Approved" } else { "Denied" }.to_string())
}

#[tauri::command]
pub async fn get_build_session(
    state: State<'_, AppState>,
) -> Result<Vec<BuildSessionEntry>, String> {
    let log = state.session_log.lock().unwrap();
    Ok(log.clone())
}

#[tauri::command]
pub async fn get_build_config(
    state: State<'_, AppState>,
) -> Result<BuildConfigResponse, String> {
    let config = state.build_config.lock().unwrap();
    Ok(BuildConfigResponse { auto_approve: config.auto_approve })
}

#[tauri::command]
pub async fn set_build_config(
    state: State<'_, AppState>,
    auto_approve: bool,
) -> Result<String, String> {
    let mut config = state.build_config.lock().unwrap();
    config.auto_approve = auto_approve;
    Ok(format!("Build config updated: auto_approve={}", auto_approve))
}

#[tauri::command]
pub async fn get_pending_permission(
    _state: State<'_, AppState>,
) -> Result<Option<PermissionRequest>, String> {
    // The frontend listens for events, but this allows polling
    Ok(None)
}
