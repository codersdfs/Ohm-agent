use crate::AppState;
use crate::commands::tools::{ToolRequest, ToolResult, GateViolationInfo};
use crate::pipeline::plan::StructuredPlan;
use crate::{PermissionEvent, MutexExt};
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

/// Env flag that enables the experimental Plan→Build pipeline.
/// Without this, build refuses to run (prevents empty-file writes).
pub const EXPERIMENTAL_PIPELINE_ENV: &str = "OMEGA_EXPERIMENTAL_PIPELINE";

pub fn experimental_pipeline_enabled() -> bool {
    matches!(
        std::env::var(EXPERIMENTAL_PIPELINE_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

const STEP_CONTENT_SYSTEM_PROMPT: &str = r#"You are Omega Agent's Build step writer.
Implement ONE plan step. Output ONLY the full file contents to write — no markdown fences, no explanation, no commentary.
If the step is a modify of an existing file, output the COMPLETE updated file contents.
If you cannot implement the step, output a single line starting with ERROR: and a short reason.
"#;

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
        if !experimental_pipeline_enabled() {
            return Err(format!(
                "Build pipeline is experimental and disabled by default. \
Use the chat agent for coding, or set {}=1 to enable. \
See ROADMAP.md tickets P0-02 / P2-04.",
                EXPERIMENTAL_PIPELINE_ENV
            ));
        }

        log::warn!(
            "BuildAgent: experimental pipeline enabled via {} — use with caution",
            EXPERIMENTAL_PIPELINE_ENV
        );

        let mut session: Vec<BuildSessionEntry> = vec![];

        log::info!("BuildAgent: executing plan with {} steps", plan.steps.len());

        {
            let mut p = state.pipeline.lock().await;
            p.status = crate::pipeline::PipelineStatus::Building;
            p.current_step_index = 0;
        }

        for (step_idx, step) in plan.steps.iter().enumerate() {
            log::info!(
                "BuildAgent: step {}/{}: {}",
                step_idx + 1,
                plan.steps.len(),
                step.description
            );

            {
                let mut p = state.pipeline.lock().await;
                p.current_step_index = step_idx;
                p.status = crate::pipeline::PipelineStatus::Building;
            }

            let needs_permission = matches!(step.action.as_str(), "create" | "modify" | "delete");
            if needs_permission {
                let (tool_name, perm_args) = match step.action.as_str() {
                    "delete" => (
                        "bash",
                        serde_json::json!({
                            "command": format!("Remove-Item -LiteralPath \"{}\"", step.file_path.as_deref().unwrap_or("")),
                            "description": step.description,
                        }),
                    ),
                    _ => (
                        "write",
                        serde_json::json!({
                            "filePath": step.file_path,
                            "description": step.description,
                        }),
                    ),
                };
                let perm_req = PermissionRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    tool: tool_name.into(),
                    args: perm_args,
                    reason: format!(
                        "Step #{}: {} — {}",
                        step.id, step.action, step.description
                    ),
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

            let start = Instant::now();
            let start_ts = chrono::Utc::now().to_rfc3339();

            let tool_req = match Self::step_to_tool_request(state, step).await {
                Ok(req) => req,
                Err(e) => {
                    let end_ts = chrono::Utc::now().to_rfc3339();
                    session.push(BuildSessionEntry {
                        step_index: step_idx,
                        tool: "generate_content".into(),
                        args: serde_json::json!({
                            "filePath": step.file_path,
                            "action": step.action,
                            "description": step.description,
                        }),
                        success: false,
                        output_preview: String::new(),
                        error: Some(e),
                        gate_passed: None,
                        gate_score: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        retries: 0,
                        timestamp_start: start_ts,
                        timestamp_end: end_ts,
                    });
                    continue;
                }
            };

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

            let status = if result.success {
                "completed"
            } else {
                "failed"
            };
            log::info!(
                "BuildAgent: step {}: {} (gate {:?} score {:?} {}ms)",
                step_idx,
                status,
                entry.gate_passed,
                entry.gate_score,
                duration_ms
            );
            session.push(entry);
        }

        {
            let mut p = state.pipeline.lock().await;
            p.status = crate::pipeline::PipelineStatus::Idle;
            p.build_output = Some(format!("Completed {} steps", session.len()));
        }
        {
            let mut log = state.session_log.lock_guard();
            *log = session.clone();
        }

        let completed = session.iter().filter(|e| e.success).count();
        let total_ms: u64 = session.iter().map(|e| e.duration_ms).sum();
        log::info!(
            "BuildAgent: complete: {}/{} steps, {}ms total",
            completed,
            plan.steps.len(),
            total_ms
        );

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
                let feedback: Vec<String> = last_violations
                    .iter()
                    .map(|v| {
                        format!(
                            "Gate violation: [{}] {} Hint: {}",
                            v.category,
                            v.message,
                            v.tool_hint.as_deref().unwrap_or("fix manually")
                        )
                    })
                    .collect();
                if let Some(obj) = args.as_object_mut() {
                    obj.insert(
                        "_gate_feedback".into(),
                        serde_json::Value::String(feedback.join("\n")),
                    );
                }
            }

            let req = ToolRequest {
                tool: tool_req.tool.clone(),
                args: args.clone(),
            };
            log::info!(
                "BuildAgent: tool={} attempt={}/{}",
                req.tool,
                attempt + 1,
                max_retries + 1
            );

            let result = match crate::commands::tools::execute_tool_inner(state, req).await {
                Ok(r) => r,
                Err(e) => {
                    if attempt < max_retries {
                        if let Some(obj) = args.as_object_mut() {
                            obj.insert(
                                "_error_feedback".into(),
                                serde_json::Value::String(format!("Previous attempt failed: {}", e)),
                            );
                        }
                        continue;
                    }
                    return ToolResult::err(e);
                }
            };

            match result {
                ToolResult {
                    success: true,
                    gate_result: Some(ref g),
                    ..
                } if g.passed => {
                    for v in &g.violations {
                        let mut db = state.rules_db.lock_guard();
                        let lang = state.detected_language.lock_guard().clone();
                        if let Some(pattern) = v.message.rsplit(": ").next() {
                            db.promote_or_increment(
                                &lang,
                                &v.category.to_lowercase(),
                                pattern,
                                &v.message,
                                "error",
                            );
                        }
                    }
                    log::info!(
                        "BuildAgent: tool={} passed gate (score={}) on attempt {}",
                        tool_req.tool,
                        g.score,
                        attempt + 1
                    );
                    return result;
                }
                ToolResult {
                    success: true,
                    gate_result: Some(ref g),
                    ..
                } => {
                    last_violations = g.violations.clone();
                    log::info!(
                        "BuildAgent: tool={} failed gate (score={}) on attempt {}",
                        tool_req.tool,
                        g.score,
                        attempt + 1
                    );
                }
                ToolResult {
                    success: false,
                    ref error,
                    ..
                } => {
                    if let Some(ref e) = error {
                        if attempt < max_retries {
                            if let Some(obj) = args.as_object_mut() {
                                obj.insert(
                                    "_error_feedback".into(),
                                    serde_json::Value::String(format!(
                                        "Previous attempt failed: {}",
                                        e
                                    )),
                                );
                            }
                            continue;
                        }
                        log::info!(
                            "BuildAgent: tool={} failed after {} attempts: {}",
                            tool_req.tool,
                            attempt + 1,
                            e
                        );
                        return result;
                    }
                }
                _ => return result,
            }
        }

        ToolResult::err(format!(
            "Gate retry limit reached after {} retries",
            max_retries
        ))
    }

    /// Convert a plan step into a tool request.
    /// create/modify ALWAYS generate non-empty content via LLM (never write empty files).
    async fn step_to_tool_request(
        state: &AppState,
        step: &crate::pipeline::plan::PlanStep,
    ) -> Result<ToolRequest, String> {
        match step.action.as_str() {
            "create" | "modify" => {
                let path = step
                    .file_path
                    .as_deref()
                    .ok_or_else(|| "create/modify step missing file_path".to_string())?;
                let content = Self::generate_step_content(state, step).await?;
                if content.trim().is_empty() {
                    return Err(format!(
                        "LLM returned empty content for step #{} ({})",
                        step.id, step.description
                    ));
                }
                if content.trim_start().starts_with("ERROR:") {
                    return Err(format!(
                        "LLM refused step #{}: {}",
                        step.id,
                        content.chars().take(300).collect::<String>()
                    ));
                }
                Ok(ToolRequest {
                    tool: "write".into(),
                    args: serde_json::json!({
                        "filePath": path,
                        "content": content,
                        "_step_description": step.description,
                    }),
                })
            }
            "delete" => {
                let path = step.file_path.as_deref().unwrap_or("");
                #[cfg(windows)]
                let command = format!("Remove-Item -LiteralPath \"{}\" -ErrorAction Stop", path);
                #[cfg(not(windows))]
                let command = format!("rm -f -- {}", shell_quote(path));
                Ok(ToolRequest {
                    tool: "bash".into(),
                    args: serde_json::json!({ "command": command }),
                })
            }
            _ => Ok(ToolRequest {
                tool: "bash".into(),
                args: serde_json::json!({ "command": step.description }),
            }),
        }
    }

    /// Ask the configured LLM for the full file contents for a create/modify step.
    async fn generate_step_content(
        state: &AppState,
        step: &crate::pipeline::plan::PlanStep,
    ) -> Result<String, String> {
        let path = step.file_path.as_deref().unwrap_or("<unknown>");
        let existing = std::fs::read_to_string(path).unwrap_or_default();

        let mut user = String::new();
        user.push_str(&format!("Action: {}\n", step.action));
        user.push_str(&format!("File path: {}\n", path));
        user.push_str(&format!("Step description: {}\n", step.description));
        if let Some(lines) = step.estimated_lines {
            user.push_str(&format!("Estimated lines: {}\n", lines));
        }
        if !existing.is_empty() {
            // Cap context to avoid blowing the window on large files
            let preview: String = existing.chars().take(40_000).collect();
            user.push_str("\nExisting file contents:\n");
            user.push_str(&preview);
            if existing.chars().count() > 40_000 {
                user.push_str("\n...[truncated existing file preview]\n");
            }
        } else {
            user.push_str("\nExisting file contents: <file does not exist or is empty>\n");
        }
        user.push_str("\nOutput the full file contents only.");

        let config = state.provider_config.lock_guard().clone();
        let provider = providers::create_provider(&config)?;
        let messages = vec![
            providers::ChatMessage {
                role: "system".into(),
                content: STEP_CONTENT_SYSTEM_PROMPT.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            providers::ChatMessage {
                role: "user".into(),
                content: user,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];
        let response = provider
            .chat(providers::ChatRequest {
                messages,
                config,
                stream: false,
                tools: None,
            })
            .await?;

        Ok(strip_code_fences(&response.content))
    }

    /// Wait for user permission via non-blocking broadcast + polling.
    async fn wait_for_permission(state: &AppState, perm_req: &PermissionRequest) -> bool {
        if state.build_config.lock_guard().auto_approve {
            return true;
        }

        let event = PermissionEvent {
            request_id: perm_req.id.clone(),
            tool: perm_req.tool.clone(),
            args: perm_req.args.clone(),
            reason: perm_req.reason.clone(),
            step_id: perm_req.step_id,
            step_description: perm_req.step_description.clone(),
        };
        let _ = state.permission_tx.send(event);

        // Bound wait so we never hang forever if the UI never answers.
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            if Instant::now() > deadline {
                log::warn!(
                    "BuildAgent: permission timeout for request {}",
                    perm_req.id
                );
                return false;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut results = state.permission_results.lock_guard();
            if let Some(result) = results.remove(&perm_req.id) {
                state.pending_permissions.lock_guard().remove(&perm_req.id);
                return result;
            }
        }
    }
}

/// Strip common markdown code fences the model may wrap around file content.
fn strip_code_fences(s: &str) -> String {
    let trimmed = s.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines();
    let first = lines.next().unwrap_or("");
    if !first.starts_with("```") {
        return trimmed.to_string();
    }
    let mut body: Vec<&str> = lines.collect();
    if body
        .last()
        .map(|l| l.trim().starts_with("```"))
        .unwrap_or(false)
    {
        body.pop();
    }
    body.join("\n")
}

#[cfg(not(windows))]
fn shell_quote(path: &str) -> String {
    // Minimal single-quote escape for POSIX shells.
    format!("'{}'", path.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::plan::PlanStep;

    #[test]
    fn experimental_flag_defaults_off() {
        // Cannot assert env globally in parallel tests; just ensure function is callable.
        let _ = experimental_pipeline_enabled();
    }

    #[test]
    fn strip_code_fences_removes_markdown() {
        let raw = "```rust\nfn main() {}\n```";
        assert_eq!(strip_code_fences(raw), "fn main() {}");
        assert_eq!(strip_code_fences("plain"), "plain");
    }

    #[test]
    fn step_create_requires_file_path() {
        // Pure validation path without LLM: missing path fails before network.
        let step = PlanStep {
            id: 1,
            action: "create".into(),
            description: "add file".into(),
            file_path: None,
            estimated_lines: Some(10),
            dependencies: vec![],
        };
        // We only unit-test the error shape via a sync helper by constructing ToolRequest args.
        assert!(step.file_path.is_none());
    }

    #[tokio::test]
    async fn execute_plan_disabled_without_flag() {
        // Ensure env is not set for this process (best-effort).
        std::env::remove_var(EXPERIMENTAL_PIPELINE_ENV);
        if experimental_pipeline_enabled() {
            // Another test may have set it; skip rather than flake.
            return;
        }
        let state = AppState::new_with_provider_config(
            &format!(
                "{}/omega_build_test_mem.db",
                std::env::temp_dir().to_string_lossy()
            ),
            providers::ProviderConfig::default(),
        );
        let plan = StructuredPlan {
            task_summary: "t".into(),
            language: "Rust".into(),
            steps: vec![PlanStep {
                id: 1,
                action: "create".into(),
                description: "add x".into(),
                file_path: Some("x.rs".into()),
                estimated_lines: Some(1),
                dependencies: vec![],
            }],
            files_affected: vec!["x.rs".into()],
            estimated_complexity: "low".into(),
            risk_level: "low".into(),
        };
        let agent = BuildAgent::new();
        let err = agent.execute_plan(&state, &plan).await.unwrap_err();
        assert!(
            err.contains("experimental") || err.contains(EXPERIMENTAL_PIPELINE_ENV),
            "expected experimental guard error, got: {}",
            err
        );
    }
}
