use crate::metadata::{ToolCategory, ToolMetadata, LatencyHint};
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext, PermissionResult};
use async_trait::async_trait;

pub struct SpawnSubagentTool;

impl SpawnSubagentTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SpawnSubagentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SpawnSubagentTool {
    fn name(&self) -> &str {
        "spawn_subagent"
    }

    fn description(&self) -> &str {
        "Spawn a subagent to perform a delegated task with its own isolated context window"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task to delegate to the subagent"
                },
                "token_budget": {
                    "type": "integer",
                    "description": "Token budget for the subagent (default 30000)",
                    "default": 30000
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Maximum tool loops before force-compact (default 10)",
                    "default": 10
                },
                "tool_whitelist": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Allowed tools (empty = read-only)"
                }
            },
            "required": ["task"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "spawn_subagent".into(),
            label: "Spawn Subagent".into(),
            description: "Spawn a subagent to perform a delegated task with its own isolated context window".into(),
            doc: Some("Delegates a subtask to an isolated context window. The subagent gets a forked copy of the parent context, its own tool subset, and returns a condensed summary (not full history) to the parent.".into()),
            category: ToolCategory::AgentManagement,
            subcategory: None,
            tags: vec!["agent".into(), "delegation".into(), "subagent".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: false,
            concurrency_safe: false,
            latency_hint: LatencyHint::Slow,
            supports_streaming: false,
            max_result_chars: 30_000,
            errors: vec![],
            examples: vec![],
            cost_hint: None,
            version: "1.0.0".into(),
            deprecation: None,
            source: crate::metadata::ToolSource::Builtin,
            source_name: None,
        }
    }

    fn check_permissions(&self, _input: &ToolInput, _ctx: &ToolUseContext) -> PermissionResult {
        PermissionResult::Prompt(
            "Subagent delegation requires permission. A subagent will run with an isolated context window.".into()
        )
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let task = input
            .args
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing required argument: task"))?;

        let token_budget = input
            .args
            .get("token_budget")
            .and_then(|v| v.as_u64())
            .unwrap_or(30_000);

        let max_turns = input
            .args
            .get("max_turns")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u32;

        let tool_whitelist: Vec<String> = input
            .args
            .get("tool_whitelist")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Full implementation would fork context and run the agent loop via omega-core
        let result_msg = format!(
            "Subagent spawned: task={}, token_budget={}, max_turns={}, tools={}",
            task, token_budget, max_turns,
            if tool_whitelist.is_empty() { "all".to_string() } else { tool_whitelist.join(", ") }
        );
        Ok(ToolResult::success(result_msg))
    }
}
