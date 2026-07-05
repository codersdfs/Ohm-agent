// 14-step execution pipeline

use crate::{ToolRegistry, ToolUseContext, ToolInput, ToolResult, ToolError, PermissionResult, BudgetCheck};
use crate::{PermissionResolver, ResultBudget};
use crate::HooksRegistry;

/// Execution pipeline for tools
pub struct ExecutionPipeline {
    registry: ToolRegistry,
    permission_resolver: PermissionResolver,
    budget: ResultBudget,
    hooks: HooksRegistry,
}

impl Default for ExecutionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionPipeline {
    pub fn new() -> Self {
        Self {
            registry: ToolRegistry::new(),
            permission_resolver: PermissionResolver::new(),
            budget: ResultBudget::new(),
            hooks: HooksRegistry::new(),
        }
    }

    pub fn with_registry(mut self, registry: ToolRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn with_permission_resolver(mut self, resolver: PermissionResolver) -> Self {
        self.permission_resolver = resolver;
        self
    }

    pub fn with_budget(mut self, budget: ResultBudget) -> Self {
        self.budget = budget;
        self
    }

    pub fn with_hooks(mut self, hooks: HooksRegistry) -> Self {
        self.hooks = hooks;
        self
    }

    /// Execute a tool through all 14 pipeline steps
    /// Returns (ToolResult, BudgetCheck)
    pub async fn execute(
        &self,
        tool_name: &str,
        input: ToolInput,
        ctx: &ToolUseContext,
    ) -> Result<(ToolResult, BudgetCheck), ToolError> {
        // Step 1: Tool Lookup
        let tool = self.registry.get(tool_name)
            .ok_or_else(|| ToolError::new(format!("Tool not found: {}", tool_name)))?;

        // Step 2: Abort check (via CancellationToken)
        if let Some(token) = &ctx.abort_token {
            if token.is_cancelled() {
                return Err(ToolError::new("Execution aborted"));
            }
        }

        // Step 3: JSON schema validation
        self.validate_input_schema(tool_name, &input)?;

        // Step 4: Semantic validation (tool-specific)
        // Semantic validation happens inside tool.call() below.
        // In the future, a validate_semantics method could be added to the Tool trait.

        // Step 5: Speculative classifier start (stub - no-op)
        // This would be for caching/future optimization

        // Step 6: Input backfill (expand ~, etc.)
        let input = self.backfill_input(input)?;

        // Step 7: PreToolUse hooks
        self.hooks.run_pre_hooks(tool_name, &input).await;

        // Step 8: Permission resolution
        let perm_result = self.permission_resolver.resolve(tool_name, &input, ctx).await;

        // Step 9: If denied → return denied result
        match perm_result {
            PermissionResult::Deny => {
                return Ok((ToolResult::error(format!("Tool '{}' denied by permissions", tool_name)), BudgetCheck {
                    within_limit: true,
                    truncated: false,
                    persisted_path: None,
                }));
            }
            PermissionResult::Prompt(msg) => {
                // Interactive prompt handling via callback
                if let Some(ref cb) = ctx.prompt_callback {
                    if !cb(&msg) {
                        return Ok((ToolResult::error(format!("Tool '{}' denied by user", tool_name)), BudgetCheck {
                            within_limit: true,
                            truncated: false,
                            persisted_path: None,
                        }));
                    }
                }
            }
            PermissionResult::Allow => {}
        }

        // Step 10: Execute tool.call()
        let mut result = tool.call(input.clone(), ctx).await
            .map_err(|e| {
                log::error!("Tool {} execution failed: {}", tool_name, e);
                e
            })?;

        // Step 11: Result budgeting
        let budget_check = self.budget.truncate(&result.output).await.1;
        let (truncated, persisted_path) = if budget_check.truncated {
            let persisted = budget_check.persisted_path.clone();
            let mut output = String::new();
            if let Some(ref p) = persisted {
                output.push_str(&format!("<persisted-output path=\"{}\" />", p.display()));
            }
            result.output = output;
            (true, persisted)
        } else {
            (false, None)
        };

        // Step 12: PostToolUse hooks
        self.hooks.run_post_hooks(tool_name, &result).await;

        // Step 13: New messages injection (stub - sub-agent transcripts)
        // This would be handled by orchestrator

        // Step 14: Error classification + telemetry-safe logging
        if !result.success {
            log::warn!("Tool {} completed with error: {:?}", tool_name, result.error);
        }

        Ok((result, BudgetCheck {
            within_limit: !truncated,
            truncated,
            persisted_path,
        }))
    }

    fn validate_input_schema(&self, tool_name: &str, input: &ToolInput) -> Result<(), ToolError> {
        let tool = self.registry.get(tool_name)
            .ok_or_else(|| ToolError::new(format!("Tool not found: {}", tool_name)))?;
        let schema = tool.parameters_schema();

        crate::schema::validate_input(&schema, &input.args)
            .map_err(|e| ToolError::with_details("Schema validation failed", e.to_string()))
    }

    fn backfill_input(&self, mut input: ToolInput) -> Result<ToolInput, ToolError> {
        // Expand ~ to home directory on supported platforms
        if let Some(path) = input.args.get("filePath").and_then(|v| v.as_str()) {
            if path.starts_with('~') {
                let home = dirs::home_dir()
                    .map(|h| h.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "~".into());
                let expanded = format!("{}{}", home, &path[1..]);
                input.args["filePath"] = serde_json::json!(expanded);
            }
        }

        // Expand path in glob patterns
        if let Some(path) = input.args.get("path").and_then(|v| v.as_str()) {
            if path.starts_with('~') {
                let home = dirs::home_dir()
                    .map(|h| h.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "~".into());
                let expanded = format!("{}{}", home, &path[1..]);
                input.args["path"] = serde_json::json!(expanded);
            }
        }

        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Tool;
    use crate::{PermissionMode, PermissionResolver};
    use async_trait::async_trait;

    struct MockTool;

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str { "mock" }
        fn description(&self) -> &str { "Mock tool" }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                },
                "required": ["input"]
            })
        }
        async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
            let input_val = input.args.get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(ToolResult::success(format!("output: {}", input_val)))
        }
    }

    #[tokio::test]
    async fn test_pipeline_execute_success() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool));

        let pipeline = ExecutionPipeline::new()
            .with_registry(registry)
            .with_permission_resolver(PermissionResolver::new()
                .with_mode(PermissionMode::BypassPermissions));

        let input = ToolInput {
            tool: "mock".into(),
            args: serde_json::json!({ "input": "test" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = pipeline.execute("mock", input, &ctx).await;
        assert!(result.is_ok());
        let (result, _check) = result.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "output: test");
    }

    #[tokio::test]
    async fn test_pipeline_unknown_tool() {
        let pipeline = ExecutionPipeline::new();
        let input = ToolInput {
            tool: "unknown".into(),
            args: serde_json::json!({}),
        };
        let ctx = ToolUseContext::new("test");

        let result = pipeline.execute("unknown", input, &ctx).await;
        assert!(result.is_err());
    }
}