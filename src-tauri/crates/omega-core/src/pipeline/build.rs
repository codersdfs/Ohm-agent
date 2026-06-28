use crate::AppState;
use crate::commands::tools::{ToolRequest, ToolResult, GateViolationInfo};
use crate::pipeline::plan::StructuredPlan;
use crate::PermissionEvent;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSessionEntry {
    pub step_index: usize,
    pub tool: String,
    pub args: serde_json::Value,
    pub success: bool,
    pub output_preview: String,
    pub error: Option<String>,
    pub gate_passed: Option<bool>,
    pub gate_score: Option<u32>,
    pub duration_ms: u64,
    pub retries: u8,
    pub timestamp_start: String,
    pub timestamp_end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    pub tool: String,
    pub args: serde_json::Value,
    pub reason: String,
    pub step_id: u32,
    pub step_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildProgress {
    pub total_steps: usize,
    pub completed_steps: usize,
    pub current_step: usize,
    pub status: String,
    pub total_retries: u32,
}

pub struct BuildAgent;

impl BuildAgent {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute_plan(
        &self,
        state: &AppState,
        plan: &StructuredPlan,
    ) -> Result<Vec<BuildSessionEntry>, String> {
        let mut session: Vec<BuildSessionEntry> = vec![];

        log::info!("BuildAgent: executing plan with {} steps", plan.steps.len());

        {
            let mut p = state.pipeline.lock().await;
            p.status = crate::pipeline::PipelineStatus::Building;
            p.current_step_index = 0;
        }

        for (step_idx, step) in plan.steps.iter().enumerate() {
            log::info!("BuildAgent: step {}/{}: {}", step_idx + 1, plan.steps.len(), step.description);

            {
                let mut p = state.pipeline.lock().await;
                p.current_step_index = step_idx;
                p.status = crate::pipeline::PipelineStatus::Building;
            }

            let needs_permission = matches!(step.action.as_str(), "create" | "modify" | "delete");
            if needs_permission {
                let perm_req = PermissionRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    tool: match step.action.as_str() {
                        "delete" => "bash".into(),
                        _ => "write".into(),
                    },
                    args: serde_json::json!({
                        "filePath": step.file_path,
                        "description": step.description,
                    }),
                    reason: format!("Step #{}: {} — {}", step.id, step.action, step.description),
                    step_id: step.id,
                    step_description: step.description.clone(),
                };

                let approved = Self::wait_for_permission(state, &perm_req).await;
                if !approved {
                    session.push(BuildSessionEntry {
                        step_index: step_idx,
                        tool: perm_req.tool,
                        args: perm_req.args,
                        success: false,
                        output_preview: String::new(),
                        error: Some("Permission denied by user".into()),
                        gate_passed: None,
                        gate_score: None,
                        duration_ms: 0,
                        retries: 0,
                        timestamp_start: chrono::Utc::now().to_rfc3339(),
                        timestamp_end: chrono::Utc::now().to_rfc3339(),
                    });
                    log::info!("BuildAgent: step {} denied", step_idx);
                    continue;
                }
            }

            let tool_req = Self::step_to_tool_request(state, step).await;
            let start = Instant::now();
            let start_ts = chrono::Utc::now().to_rfc3339();

            let result = Self::execute_tool_with_retry(state, tool_req.clone(), 3).await;
            let duration_ms = start.elapsed().as_millis() as u64;
            let end_ts = chrono::Utc::now().to_rfc3339();

            let entry = BuildSessionEntry {
                step_index: step_idx,
                tool: tool_req.tool,
                args: tool_req.args,
                success: result.success,
                output_preview: result.output.chars().take(200).collect(),
                error: result.error.clone(),
                gate_passed: result.gate_result.as_ref().map(|g| g.passed),
                gate_score: result.gate_result.as_ref().map(|g| g.score),
                duration_ms,
                retries: 0,
                timestamp_start: start_ts,
                timestamp_end: end_ts,
            };

            let status = if result.success { "completed" } else { "failed" };
            log::info!("BuildAgent: step {}: {} (gate {:?} score {:?} {}ms)", step_idx, status, entry.gate_passed, entry.gate_score, duration_ms);
            session.push(entry);
        }

        {
            let mut p = state.pipeline.lock().await;
            p.status = crate::pipeline::PipelineStatus::Idle;
            p.build_output = Some(format!("Completed {} steps", session.len()));
        }
        {
            let mut log = state.session_log.lock().unwrap();
            *log = session.clone();
        }

        let completed = session.iter().filter(|e| e.success).count();
        let total_ms: u64 = session.iter().map(|e| e.duration_ms).sum();
        log::info!("BuildAgent: complete: {}/{} steps, {}ms total", completed, plan.steps.len(), total_ms);

        Ok(session)
    }

    async fn execute_tool_with_retry(
        state: &AppState,
        tool_req: ToolRequest,
        max_retries: u8,
    ) -> ToolResult {
        let mut last_violations: Vec<GateViolationInfo> = vec![];

        for attempt in 0..=max_retries {
            let mut args = tool_req.args.clone();
            if attempt > 0 && !last_violations.is_empty() {
                let feedback: Vec<String> = last_violations.iter()
                    .map(|v| format!("Gate violation: [{}] {} Hint: {}", v.category, v.message, v.tool_hint.as_deref().unwrap_or("fix manually")))
                    .collect();
                if let Some(obj) = args.as_object_mut() {
                    obj.insert("_gate_feedback".into(), serde_json::Value::String(feedback.join("\n")));
                }
            }

            let req = ToolRequest { tool: tool_req.tool.clone(), args: args.clone() };
            log::info!("BuildAgent: tool={} attempt={}/{}", req.tool, attempt + 1, max_retries + 1);

            let result = match crate::commands::tools::execute_tool_inner(state, req).await {
                Ok(r) => r,
                Err(e) => {
                    if attempt < max_retries {
                        if let Some(obj) = args.as_object_mut() {
                            obj.insert("_error_feedback".into(), serde_json::Value::String(format!("Previous attempt failed: {}", e)));
                        }
                        continue;
                    }
                    return ToolResult { success: false, output: String::new(), error: Some(e), gate_result: None };
                }
            };

            match result {
                ToolResult { success: true, gate_result: Some(ref g), .. } if g.passed => {
                    for v in &g.violations {
                        let mut db = state.rules_db.lock().unwrap();
                        let lang = state.detected_language.lock().unwrap().clone();
                        if let Some(pattern) = v.message.rsplit(": ").next() {
                            db.promote_or_increment(&lang, &v.category.to_lowercase(), pattern, &v.message, "error");
                        }
                    }
                    log::info!("BuildAgent: tool={} passed gate (score={}) on attempt {}", tool_req.tool, g.score, attempt + 1);
                    return result;
                }
                ToolResult { success: true, gate_result: Some(ref g), .. } => {
                    last_violations = g.violations.clone();
                    log::info!("BuildAgent: tool={} failed gate (score={}) on attempt {}", tool_req.tool, g.score, attempt + 1);
                }
                ToolResult { success: false, ref error, .. } => {
                    if let Some(ref e) = error {
                        if attempt < max_retries {
                            if let Some(obj) = args.as_object_mut() {
                                obj.insert("_error_feedback".into(), serde_json::Value::String(format!("Previous attempt failed: {}", e)));
                            }
                            continue;
                        }
                        log::info!("BuildAgent: tool={} failed after {} attempts: {}", tool_req.tool, attempt + 1, e);
                        return result;
                    }
                }
                _ => return result,
            }
        }

        ToolResult {
            success: true,
            output: format!("Written with Gate violations after {} retries", max_retries),
            error: Some("Gate retry limit reached".into()),
            gate_result: None,
        }
    }

    async fn step_to_tool_request(_state: &AppState, step: &crate::pipeline::plan::PlanStep) -> ToolRequest {
        match step.action.as_str() {
            "create" | "modify" => ToolRequest {
                tool: "write".into(),
                args: serde_json::json!({
                    "filePath": step.file_path,
                    "content": "",
                    "_step_description": step.description,
                }),
            },
            "delete" => ToolRequest {
                tool: "bash".into(),
                args: serde_json::json!({
                    "command": format!("Remove-Item -LiteralPath \"{}\"", step.file_path.as_deref().unwrap_or("")),
                }),
            },
            _ => ToolRequest {
                tool: "bash".into(),
                args: serde_json::json!({ "command": step.description }),
            },
        }
    }

    /// Wait for user permission via non-blocking broadcast + polling.
    /// The permission request is broadcast on state.permission_tx (picked up by
    /// the Tauri command wrapper and forwarded to the frontend). The frontend
    /// calls respond_permission, which writes to state.permission_results.
    async fn wait_for_permission(state: &AppState, perm_req: &PermissionRequest) -> bool {
        if state.build_config.lock().unwrap().auto_approve {
            return true;
        }

        // Broadcast the permission request to any subscriber (e.g. Tauri event forwarder)
        let event = PermissionEvent {
            request_id: perm_req.id.clone(),
            tool: perm_req.tool.clone(),
            args: perm_req.args.clone(),
            reason: perm_req.reason.clone(),
            step_id: perm_req.step_id,
            step_description: perm_req.step_description.clone(),
        };
        let _ = state.permission_tx.send(event);

        // Poll for a response written by respond_permission
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut results = state.permission_results.lock().unwrap();
            if let Some(result) = results.remove(&perm_req.id) {
                state.pending_permissions.lock().unwrap().remove(&perm_req.id);
                return result;
            }
        }
    }
}
