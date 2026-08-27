// 14-step execution pipeline

use crate::hooks::HookContext;
use crate::HooksRegistry;
use crate::{
    BudgetCheck, PermissionResult, Tool, ToolError, ToolErrorKind, ToolInput, ToolRegistry,
    ToolResult, ToolUseContext,
};
use crate::{PermissionResolver, ResultBudget};

/// Execution pipeline for tools
pub struct ExecutionPipeline {
    registry: ToolRegistry,
    permission_resolver: PermissionResolver,
    budget: ResultBudget,
    hooks: HooksRegistry,
    /// Context threaded to every hook invocation (session id, turn id,
    /// workspace root). Built once per pipeline instead of per call.
    hook_ctx: HookContext,
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
            hook_ctx: HookContext {
                session_id: String::new(),
                turn_id: None,
                workspace: std::path::PathBuf::from("."),
            },
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

    /// Inject real session/workspace context for hook invocations.
    /// Without this, hooks see empty placeholders.
    pub fn with_hook_context(mut self, ctx: HookContext) -> Self {
        self.hook_ctx = ctx;
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
        let tool = self.registry.get(tool_name).ok_or_else(|| {
            ToolError::with_kind_and_source(
                ToolErrorKind::NotFound,
                format!("Tool not found: {}", tool_name),
                tool_name,
            )
        })?;

        // Step 2: Abort check (via CancellationToken)
        if let Some(token) = &ctx.abort_token {
            if token.is_cancelled() {
                return Err(ToolError::with_kind(
                    ToolErrorKind::Aborted,
                    "Execution aborted",
                ));
            }
        }

        // Step 3: JSON schema validation
        self.validate_input_schema(tool, &input)?;

        // Step 4: Semantic validation (tool-specific)
        // Semantic validation happens inside tool.call() below.
        // In the future, a validate_semantics method could be added to the Tool trait.

        // Step 5: Speculative classifier start (stub - no-op)
        // This would be for caching/future optimization

        // Step 6: Input backfill (expand ~, etc.)
        let input = self.backfill_input(input)?;

        // Step 7: PreToolUse hooks — a Deny short-circuits BEFORE execution
        // (block-before-write). Inject appends guidance to the eventual output.
        let mut injected: Vec<String> = Vec::new();
        match self.hooks.run_pre_tool(&self.hook_ctx, tool_name, &input) {
            crate::hooks::HookDecision::Allow => {}
            crate::hooks::HookDecision::Deny(reason) => {
                log::warn!("Tool {} denied by pre-tool hook: {}", tool_name, reason);
                return Ok((
                    ToolResult::error(format!("Tool '{}' blocked by hook: {}", tool_name, reason)),
                    BudgetCheck {
                        within_limit: true,
                        truncated: false,
                        persisted_path: None,
                    },
                ));
            }
            crate::hooks::HookDecision::Inject(msg) => {
                injected.push(msg);
            }
        }

        // Step 8: Permission resolution — includes tool.check_permissions()
        let perm_result = self
            .permission_resolver
            .resolve(tool_name, &input, ctx, Some(tool))
            .await;

        // Step 9: If denied → return denied result
        match perm_result {
            PermissionResult::Deny => {
                return Ok((
                    ToolResult::error(format!("Tool '{}' denied by permissions", tool_name)),
                    BudgetCheck {
                        within_limit: true,
                        truncated: false,
                        persisted_path: None,
                    },
                ));
            }
            PermissionResult::Prompt(msg) => {
                // Interactive prompt handling via callback
                if let Some(ref cb) = ctx.prompt_callback {
                    if !cb(&msg) {
                        return Ok((
                            ToolResult::error(format!("Tool '{}' denied by user", tool_name)),
                            BudgetCheck {
                                within_limit: true,
                                truncated: false,
                                persisted_path: None,
                            },
                        ));
                    }
                }
            }
            PermissionResult::Allow => {}
        }

        // Step 10: Execute tool.call()
        let mut result = tool.call(input.clone(), ctx).await.map_err(|e| {
            log::error!("Tool {} execution failed: {}", tool_name, e);
            e
        })?;

        // Apply any Inject decisions from step 7 to the tool output.
        if !injected.is_empty() && result.success {
            let mut guidance = injected.join("\n");
            guidance.push('\n');
            guidance.push_str(&result.output);
            result.output = guidance;
        }

        // Step 11: Result budgeting — use the truncated string from truncate()
        let (truncated_output, budget_check) = self.budget.truncate(&result.output).await;
        result.output = truncated_output;
        let truncated = budget_check.truncated;
        let persisted_path = budget_check.persisted_path.clone();

        // Step 12: PostToolUse hooks
        let _ = self.hooks.run_post_tool(&self.hook_ctx, tool_name, &result);

        // Step 13: New messages injection (stub - sub-agent transcripts)
        // This would be handled by orchestrator

        // Step 14: Error classification + telemetry-safe logging
        if !result.success {
            log::warn!(
                "Tool {} completed with error: {:?}",
                tool_name,
                result.error
            );
        }

        Ok((
            result,
            BudgetCheck {
                within_limit: !truncated,
                truncated,
                persisted_path,
            },
        ))
    }

    fn validate_input_schema(&self, tool: &dyn Tool, input: &ToolInput) -> Result<(), ToolError> {
        let schema = tool.parameters_schema();

        crate::schema::validate_input(&schema, &input.args).map_err(|e| ToolError {
            kind: ToolErrorKind::SchemaValidation,
            message: "Schema validation failed".into(),
            details: Some(e.to_string()),
            retryable: false,
            source_tool: tool.name().to_string().into(),
        })
    }

    fn backfill_input(&self, mut input: ToolInput) -> Result<ToolInput, ToolError> {
        // Expand ~ to home directory for any string argument that starts with ~
        if let Some(obj) = input.args.as_object_mut() {
            for (_key, value) in obj.iter_mut() {
                if let Some(s) = value.as_str() {
                    if s.starts_with('~') && s.len() > 1 {
                        let home = dirs::home_dir()
                            .map(|h| h.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "~".into());
                        *value = serde_json::json!(format!("{}{}", home, &s[1..]));
                    }
                }
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
        fn name(&self) -> &str {
            "mock"
        }
        fn description(&self) -> &str {
            "Mock tool"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                },
                "required": ["input"]
            })
        }
        async fn call(
            &self,
            input: ToolInput,
            _ctx: &ToolUseContext,
        ) -> Result<ToolResult, ToolError> {
            let input_val = input
                .args
                .get("input")
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
            .with_permission_resolver(
                PermissionResolver::new().with_mode(PermissionMode::BypassPermissions),
            );

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

    // ── Pre-hook decision tests ──────────────────────────────────────────────

    use crate::hooks::HooksRegistry;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingTool {
        calls: std::sync::Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for CountingTool {
        fn name(&self) -> &str {
            "counting"
        }
        fn description(&self) -> &str {
            "Counts invocations"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn call(
            &self,
            _input: ToolInput,
            _ctx: &ToolUseContext,
        ) -> Result<ToolResult, ToolError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ToolResult::success("executed"))
        }
    }

    /// Hook that denies every call with a fixed reason.
    struct DenyHook {
        reason: &'static str,
    }

    #[async_trait]
    impl crate::hooks::Hook for DenyHook {
        fn on_tool_pre(
            &self,
            _ctx: &crate::hooks::HookContext,
            _tool_name: &str,
            _input: &ToolInput,
        ) -> crate::hooks::HookDecision {
            crate::hooks::HookDecision::Deny(self.reason.to_string())
        }
    }

    /// Hook that captures the context values it observes.
    struct ContextCaptureHook {
        seen_session: std::sync::Arc<std::sync::Mutex<Option<String>>>,
        seen_workspace: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    }

    #[async_trait]
    impl crate::hooks::Hook for ContextCaptureHook {
        fn on_tool_pre(
            &self,
            ctx: &crate::hooks::HookContext,
            _tool_name: &str,
            _input: &ToolInput,
        ) -> crate::hooks::HookDecision {
            *self.seen_session.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(ctx.session_id.clone());
            *self
                .seen_workspace
                .lock()
                .unwrap_or_else(|e| e.into_inner()) =
                Some(ctx.workspace.to_string_lossy().into_owned());
            crate::hooks::HookDecision::Allow
        }
    }

    #[tokio::test]
    async fn test_pipeline_pre_hook_deny_blocks_execution() {
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(CountingTool {
            calls: calls.clone(),
        }));

        let mut hooks = HooksRegistry::new();
        hooks.register(Box::new(DenyHook {
            reason: "gate score 40 < 80",
        }));

        let pipeline = ExecutionPipeline::new()
            .with_registry(registry)
            .with_permission_resolver(
                PermissionResolver::new().with_mode(PermissionMode::BypassPermissions),
            )
            .with_hooks(hooks);

        let input = ToolInput {
            tool: "counting".into(),
            args: serde_json::json!({}),
        };
        let ctx = ToolUseContext::new("test");

        let (result, budget) = pipeline.execute("counting", input, &ctx).await.unwrap();
        assert!(!result.success, "denied tool must not succeed");
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or("")
                .contains("blocked by hook"),
            "error should carry the hook denial reason, got: {:?}",
            result.error
        );
        assert!(budget.within_limit);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "tool body must NOT run when a pre-hook denies"
        );
    }

    #[tokio::test]
    async fn test_pipeline_pre_hook_allow_executes() {
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(CountingTool {
            calls: calls.clone(),
        }));

        let pipeline = ExecutionPipeline::new()
            .with_registry(registry)
            .with_permission_resolver(
                PermissionResolver::new().with_mode(PermissionMode::BypassPermissions),
            )
            .with_hooks(HooksRegistry::new());

        let input = ToolInput {
            tool: "counting".into(),
            args: serde_json::json!({}),
        };
        let ctx = ToolUseContext::new("test");

        let (result, _) = pipeline.execute("counting", input, &ctx).await.unwrap();
        assert!(result.success, "no denying hook → tool executes");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_pipeline_hook_context_is_threaded() {
        let capture = ContextCaptureHook {
            seen_session: std::sync::Arc::new(std::sync::Mutex::new(None)),
            seen_workspace: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let seen_session = capture.seen_session.clone();
        let seen_workspace = capture.seen_workspace.clone();

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool));

        let mut hooks = HooksRegistry::new();
        hooks.register(Box::new(capture));

        let pipeline = ExecutionPipeline::new()
            .with_registry(registry)
            .with_permission_resolver(
                PermissionResolver::new().with_mode(PermissionMode::BypassPermissions),
            )
            .with_hooks(hooks)
            .with_hook_context(crate::hooks::HookContext {
                session_id: "sess-42".into(),
                turn_id: Some("turn-7".into()),
                workspace: std::path::PathBuf::from("/tmp/ws"),
            });

        let input = ToolInput {
            tool: "mock".into(),
            args: serde_json::json!({ "input": "x" }),
        };
        let ctx = ToolUseContext::new("test");
        let _ = pipeline.execute("mock", input, &ctx).await.unwrap();

        assert_eq!(
            seen_session
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_deref(),
            Some("sess-42"),
            "hook must observe the injected session id, not the placeholder"
        );
        assert_eq!(
            seen_workspace
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_deref(),
            Some("/tmp/ws"),
            "hook must observe the injected workspace, not \".\""
        );
    }
}
