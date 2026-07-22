use crate::{AppState, MutexExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: u32,
    pub action: String,
    pub description: String,
    pub file_path: Option<String>,
    pub estimated_lines: Option<u32>,
    pub dependencies: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredPlan {
    pub task_summary: String,
    pub language: String,
    pub steps: Vec<PlanStep>,
    pub files_affected: Vec<String>,
    pub estimated_complexity: String,
    pub risk_level: String,
}

/// System prompt used to instruct the LLM to produce a structured plan.
pub const PLAN_SYSTEM_PROMPT: &str = r#"You are Omega Agent's Plan agent. Your job is to analyze a task and produce a structured, actionable plan.

Output your plan as a JSON object with this exact structure:
{
  "task_summary": "One-line summary of what needs to be done",
  "language": "Rust | TypeScript | Python | etc",
  "steps": [
    {
      "id": 1,
      "action": "create | modify | delete | refactor | test",
      "description": "Clear description of what this step does",
      "file_path": "relative/path/to/file.ext",
      "estimated_lines": 50,
      "dependencies": []
    }
  ],
  "files_affected": ["src/main.rs", "src/lib.rs"],
  "estimated_complexity": "low | medium | high",
  "risk_level": "low | medium | high"
}

Rules:
- Break the task into atomic, ordered steps
- Each step must have a single clear action on one file
- Dependencies reference step IDs that must complete first
- Estimate lines of code changed per step
- Do NOT write any code — only plan
- Output ONLY the JSON object, no markdown wrappers, no explanation
"#;

impl StructuredPlan {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let cleaned: String = json
            .chars()
            .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
            .collect();
        serde_json::from_str(&cleaned).map_err(|e| {
            format!(
                "Failed to parse plan JSON: {} — raw: {}",
                e,
                json.chars().take(200).collect::<String>()
            )
        })
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub fn total_estimated_lines(&self) -> u32 {
        self.steps.iter().filter_map(|s| s.estimated_lines).sum()
    }
}

pub struct PlanAgent;

impl PlanAgent {
    pub fn new() -> Self {
        Self
    }

    /// Generate a structured plan using the configured LLM provider.
    /// Returns the plan and the raw LLM output (for streaming to chat).
    pub async fn generate(
        &self,
        state: &AppState,
        task: &str,
    ) -> Result<(StructuredPlan, String), String> {
        log::info!("PlanAgent: generating plan for task");

        let config = state.provider_config.lock_guard().clone();
        let provider = providers::create_provider(&config)?;

        let messages = vec![
            providers::ChatMessage {
                role: "system".into(),
                content: PLAN_SYSTEM_PROMPT.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            providers::ChatMessage {
                role: "user".into(),
                content: format!(
                    "Task: {}\n\nProject files detected: {:?}",
                    task,
                    state.detected_language.lock_guard().label()
                ),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        let chat_request = providers::ChatRequest {
            messages,
            config,
            stream: false,
            tools: None,
        };

        let response = provider.chat(chat_request).await?;
        let raw = response.content.clone();

        let plan = StructuredPlan::from_json(&raw).map_err(|e| {
            format!(
                "Plan parsing error: {}. Raw output: {}",
                e,
                raw.chars().take(300).collect::<String>()
            )
        })?;

        log::info!(
            "PlanAgent: generated plan with {} steps, {} files, ~{} lines",
            plan.step_count(),
            plan.files_affected.len(),
            plan.total_estimated_lines()
        );

        Ok((plan, raw))
    }

    /// Generate plan with streaming — sends tokens via Tauri event then returns parsed plan.
    pub async fn generate_streaming(
        &self,
        state: &AppState,
        task: &str,
    ) -> Result<(StructuredPlan, String), String> {
        log::info!("PlanAgent: generating streaming plan");

        let config = state.provider_config.lock_guard().clone();
        let provider = providers::create_provider(&config)?;

        let messages = vec![
            providers::ChatMessage {
                role: "system".into(),
                content: PLAN_SYSTEM_PROMPT.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            providers::ChatMessage {
                role: "user".into(),
                content: format!(
                    "Task: {}\n\nProject language: {}",
                    task,
                    state.detected_language.lock_guard().label()
                ),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let chat_request = providers::ChatRequest {
            messages,
            config,
            stream: true,
            tools: None,
        };

        tokio::spawn(async move {
            let _ = provider.chat_stream(chat_request, tx).await;
        });

        let mut full_output = String::new();
        while let Some(chunk) = rx.recv().await {
            full_output.push_str(&chunk.content);
            if chunk.done {
                break;
            }
        }

        let plan = StructuredPlan::from_json(&full_output)?;

        Ok((plan, full_output))
    }
}
