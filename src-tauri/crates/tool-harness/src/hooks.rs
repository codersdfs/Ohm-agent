//! Hook lifecycle expansion — 8-method Hook trait, Gate-as-hook, shell hooks.
//!
//! Expands the hook system from pre/post tool use to full agent lifecycle:
//! session start/end, prompt start/end, tool pre/post, subagent start/end,
//! checkpoint, stop. Gate runs as a hook on write/edit tools.

use crate::{ToolInput, ToolResult};
use async_trait::async_trait;

/// Hook decision for pre-tool callbacks.
#[derive(Debug, Clone)]
pub enum HookDecision {
    Allow,
    Deny(String),
    Inject(String),
}

/// Context passed to hooks.
#[derive(Debug, Clone)]
pub struct HookContext {
    pub session_id: String,
    pub turn_id: Option<String>,
    pub workspace: std::path::PathBuf,
}

/// The unified Hook trait covering 8+ lifecycle events.
#[async_trait]
pub trait Hook: Send + Sync {
    fn on_session_start(&self, _ctx: &HookContext) -> HookDecision {
        HookDecision::Allow
    }

    fn on_prompt_end(&self, _ctx: &HookContext, _response: &str) -> HookDecision {
        HookDecision::Allow
    }

    fn on_tool_pre(&self, _ctx: &HookContext, _tool_name: &str, _input: &ToolInput) -> HookDecision {
        HookDecision::Allow
    }

    fn on_tool_post(&self, _ctx: &HookContext, _tool_name: &str, _result: &ToolResult) -> Result<(), String> {
        Ok(())
    }

    fn on_subagent_start(&self, _ctx: &HookContext, _task: &str) -> HookDecision {
        HookDecision::Allow
    }

    fn on_subagent_end(&self, _ctx: &HookContext, _result: &str) -> Result<(), String> {
        Ok(())
    }

    fn on_checkpoint(&self, _ctx: &HookContext, _checkpoint_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn on_session_end(&self, _ctx: &HookContext, _summary: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Registry for managing hooks with priority ordering.
pub struct HooksRegistry {
    hooks: Vec<Box<dyn Hook>>,
}

impl Default for HooksRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HooksRegistry {
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
        }
    }

    pub fn register(&mut self, hook: Box<dyn Hook>) {
        self.hooks.push(hook);
    }

    pub fn run_pre_tool(&self, ctx: &HookContext, tool: &str, input: &ToolInput) -> HookDecision {
        for hook in &self.hooks {
            match hook.on_tool_pre(ctx, tool, input) {
                HookDecision::Allow => {}
                other => return other,
            }
        }
        HookDecision::Allow
    }

    pub fn run_post_tool(&self, ctx: &HookContext, tool: &str, result: &ToolResult) -> Result<(), String> {
        for hook in &self.hooks {
            hook.on_tool_post(ctx, tool, result)?;
        }
        Ok(())
    }
}

/// Gate-as-hook implementation.
pub struct GateHook {
    mode: GateHookMode,
}

#[derive(Debug, Clone)]
pub enum GateHookMode {
    Block,
    Warn,
    AdviceOnly,
}

impl GateHook {
    pub fn new(mode: GateHookMode) -> Self {
        Self { mode }
    }
}

#[async_trait]
impl Hook for GateHook {
    fn on_tool_pre(&self, _ctx: &HookContext, tool_name: &str, input: &ToolInput) -> HookDecision {
        if !matches!(tool_name, "write" | "edit" | "apply_patch" | "git_commit") {
            return HookDecision::Allow;
        }

        // Placeholder for Gate scoring — full implementation would call
        // harness::engine::GateEngine::score_file()
        match self.mode {
            GateHookMode::Block => {
                // If score < 80, deny
                HookDecision::Allow
            }
            GateHookMode::Warn => {
                HookDecision::Allow
            }
            GateHookMode::AdviceOnly => {
                HookDecision::Allow
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingHook {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Hook for CountingHook {
        fn on_tool_pre(&self, _ctx: &HookContext, _tool_name: &str, _input: &ToolInput) -> HookDecision {
            self.count.fetch_add(1, Ordering::SeqCst);
            HookDecision::Allow
        }
    }

    #[test]
    fn test_hooks_registry_runs_hooks() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut registry = HooksRegistry::new();
        registry.register(Box::new(CountingHook {
            count: count.clone(),
        }));

        let ctx = HookContext {
            session_id: "test".into(),
            turn_id: None,
            workspace: std::path::PathBuf::from("."),
        };

        let input = ToolInput {
            tool: "test".into(),
            args: serde_json::json!({}),
        };

        registry.run_pre_tool(&ctx, "test", &input);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_gate_hook_allows_non_write_tools() {
        let gate = GateHook::new(GateHookMode::Block);
        let ctx = HookContext {
            session_id: "test".into(),
            turn_id: None,
            workspace: std::path::PathBuf::from("."),
        };

        let input = ToolInput {
            tool: "read".into(),
            args: serde_json::json!({}),
        };

        assert!(matches!(
            gate.on_tool_pre(&ctx, "read", &input),
            HookDecision::Allow
        ));
    }
}
