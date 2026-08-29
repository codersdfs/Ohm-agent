//! Hook lifecycle expansion — 8-method Hook trait, Gate-as-hook, shell hooks.
//!
//! Expands the hook system from pre/post tool use to full agent lifecycle:
//! session start/end, prompt start/end, tool pre/post, subagent start/end,
//! checkpoint, stop. Gate runs as a hook on write/edit tools.

use crate::{ToolInput, ToolResult};
use async_trait::async_trait;
use std::sync::Arc;

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

    fn on_tool_pre(
        &self,
        _ctx: &HookContext,
        _tool_name: &str,
        _input: &ToolInput,
    ) -> HookDecision {
        HookDecision::Allow
    }

    fn on_tool_post(
        &self,
        _ctx: &HookContext,
        _tool_name: &str,
        _result: &ToolResult,
    ) -> Result<(), String> {
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
        Self { hooks: Vec::new() }
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

    pub fn run_post_tool(
        &self,
        ctx: &HookContext,
        tool: &str,
        result: &ToolResult,
    ) -> Result<(), String> {
        for hook in &self.hooks {
            hook.on_tool_post(ctx, tool, result)?;
        }
        Ok(())
    }
}

/// Scoring callback: given file content and tool args, returns (score, violations).
/// The score is on the 0–100 scale (pass ≥ 80, per harness scoring.rs).
pub type GateScorer = Arc<dyn Fn(&str, &str, &ToolInput) -> (u32, Vec<String>) + Send + Sync>;

/// Gate-as-hook implementation.
///
/// Runs Gate scoring on write/edit operations before they proceed. The scorer
/// is injected via [`GateHook::with_scorer`] so `tool-harness` stays free of
/// a hard dependency on the `harness` crate (which provides `GateEngine`).
/// `omega-core` wires in the real implementation.
pub struct GateHook {
    mode: GateHookMode,
    scorer: Option<GateScorer>,
    pass_threshold: u32,
}

#[derive(Debug, Clone)]
pub enum GateHookMode {
    /// Gate blocks the write if score < threshold.
    Block,
    /// Gate allows the write but injects advice as a system message.
    Warn,
    /// Gate runs but only logs — never blocks.
    AdviceOnly,
}

impl GateHook {
    pub fn new(mode: GateHookMode) -> Self {
        Self {
            mode,
            scorer: None,
            pass_threshold: 80,
        }
    }

    /// Attach a scorer function (typically backed by `harness::engine::GateEngine`).
    pub fn with_scorer(mut self, scorer: GateScorer) -> Self {
        self.scorer = Some(scorer);
        self
    }

    /// Set the pass threshold (default 80, per harness scoring.rs).
    pub fn with_pass_threshold(mut self, threshold: u32) -> Self {
        self.pass_threshold = threshold;
        self
    }

    /// Extract the file content from a write/edit tool's input args.
    fn extract_content(tool_name: &str, input: &ToolInput) -> String {
        match tool_name {
            "write" => input
                .args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            "edit" => input
                .args
                .get("newText")
                .or_else(|| input.args.get("newString"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            "apply_patch" => input
                .args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            "git_commit" => input
                .args
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            _ => String::new(),
        }
    }

    /// Resolve the file path from a write/edit tool's input args.
    fn extract_path(input: &ToolInput) -> String {
        input
            .args
            .get("filePath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }
}

#[async_trait]
impl Hook for GateHook {
    fn on_tool_pre(&self, ctx: &HookContext, tool_name: &str, input: &ToolInput) -> HookDecision {
        if !matches!(tool_name, "write" | "edit" | "apply_patch" | "git_commit") {
            return HookDecision::Allow;
        }

        // Without a scorer attached, pass through (no-op gate).
        let scorer = match &self.scorer {
            Some(s) => s,
            None => return HookDecision::Allow,
        };

        let content = Self::extract_content(tool_name, input);
        let path = Self::extract_path(input);
        let workspace = ctx.workspace.to_string_lossy().to_string();
        let full_path = if path.is_empty() {
            workspace.clone()
        } else if std::path::Path::new(&path).is_absolute() {
            path
        } else {
            format!("{}/{}", workspace.trim_end_matches('/'), path)
        };

        let (score, violations) = scorer(&full_path, &content, input);

        match self.mode {
            GateHookMode::Block => {
                if score < self.pass_threshold {
                    let msg = format!(
                        "Gate score {} < threshold {}: {}",
                        score,
                        self.pass_threshold,
                        violations.join("; ")
                    );
                    HookDecision::Deny(msg)
                } else {
                    HookDecision::Allow
                }
            }
            GateHookMode::Warn => {
                if score < self.pass_threshold {
                    let advice = format!(
                        "# Gate feedback (score {}/{}):\n{}\n",
                        score,
                        self.pass_threshold,
                        violations.join("\n")
                    );
                    HookDecision::Inject(advice)
                } else {
                    HookDecision::Allow
                }
            }
            GateHookMode::AdviceOnly => {
                if score < self.pass_threshold {
                    log::info!(
                        "Gate advice: score {}/{} for {} — {}",
                        score,
                        self.pass_threshold,
                        tool_name,
                        violations.join("; ")
                    );
                }
                HookDecision::Allow
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingHook {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Hook for CountingHook {
        fn on_tool_pre(
            &self,
            _ctx: &HookContext,
            _tool_name: &str,
            _input: &ToolInput,
        ) -> HookDecision {
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

    #[test]
    fn test_gate_hook_blocks_on_low_score() {
        let scorer = Arc::new(
            |_path: &str, _content: &str, _input: &ToolInput| -> (u32, Vec<String>) {
                (40, vec!["too many violations".into()])
            },
        );

        let gate = GateHook::new(GateHookMode::Block)
            .with_scorer(scorer)
            .with_pass_threshold(80);

        let ctx = HookContext {
            session_id: "test".into(),
            turn_id: None,
            workspace: std::path::PathBuf::from("."),
        };

        let input = ToolInput {
            tool: "write".into(),
            args: serde_json::json!({
                "filePath": "test.rs",
                "content": "fn main() {}"
            }),
        };

        let decision = gate.on_tool_pre(&ctx, "write", &input);
        assert!(matches!(
            decision,
            HookDecision::Deny(msg) if msg.contains("40") && msg.contains("80")
        ));
    }

    #[test]
    fn test_gate_hook_allows_on_high_score() {
        let scorer = Arc::new(
            |_path: &str, _content: &str, _input: &ToolInput| -> (u32, Vec<String>) {
                (95, vec![])
            },
        );

        let gate = GateHook::new(GateHookMode::Block)
            .with_scorer(scorer)
            .with_pass_threshold(80);

        let ctx = HookContext {
            session_id: "test".into(),
            turn_id: None,
            workspace: std::path::PathBuf::from("."),
        };

        let input = ToolInput {
            tool: "write".into(),
            args: serde_json::json!({
                "filePath": "test.rs",
                "content": "fn main() {}"
            }),
        };

        assert!(matches!(
            gate.on_tool_pre(&ctx, "write", &input),
            HookDecision::Allow
        ));
    }

    #[test]
    fn test_gate_hook_allows_without_scorer() {
        let gate = GateHook::new(GateHookMode::Block);
        let ctx = HookContext {
            session_id: "test".into(),
            turn_id: None,
            workspace: std::path::PathBuf::from("."),
        };

        let input = ToolInput {
            tool: "write".into(),
            args: serde_json::json!({
                "filePath": "test.rs",
                "content": "fn main() {}"
            }),
        };

        // Without a scorer, the gate is a no-op (allows everything)
        assert!(matches!(
            gate.on_tool_pre(&ctx, "write", &input),
            HookDecision::Allow
        ));
    }
}
