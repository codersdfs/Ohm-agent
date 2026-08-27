//! Concrete GateHook wiring — bridges tool-harness's GateHook to the real
//! harness::engine::GateEngine.
//!
//! `tool-harness` provides the `GateHook` trait and `GateScorer` callback type,
//! but does not depend on `harness` (to avoid a hard dependency). This module
//! provides the real scorer that calls `GateEngine::check_file()`.

use std::path::PathBuf;

use harness::{GateEngine, Language};
use tool_harness::{GateHook, GateHookMode, GateScorer};

use crate::{AppState, MutexExt};

/// Resolve the gate mode from the environment.
///
/// `OMEGA_GATE_MODE` accepts `warn` (default), `block`, or `advice`.
/// This is the escape hatch: if the Gate ever blocks a legitimate edit,
/// set `OMEGA_GATE_MODE=warn` without rebuilding.
pub fn gate_mode_from_env() -> GateHookMode {
    match std::env::var("OMEGA_GATE_MODE")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "block" => GateHookMode::Block,
        "advice" => GateHookMode::AdviceOnly,
        _ => GateHookMode::Warn,
    }
}

/// Build a `GateHook` with a real scorer backed by `GateEngine`.
///
/// The scorer creates a `GateEngine` lazily (on first call) with the given
/// workspace root and detected language, caches it in `engine_holder`,
/// and calls `check_file(path, content)` for each write/edit.
pub fn build_gate_hook(
    mode: GateHookMode,
    workspace: PathBuf,
    language: Language,
    pass_threshold: u32,
) -> GateHook {
    // The GateScorer callback must be Fn + Send + Sync, but we need interior
    // mutability for the lazily-constructed GateEngine. Since the closure
    // outlives a single call, we store the engine in a shared Mutex
    // (poison-safe via `MutexExt::lock_guard`).
    let engine_holder: std::sync::Arc<std::sync::Mutex<Option<GateEngine>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let workspace_str = workspace.to_string_lossy().to_string();
    let lang_for_engine = language.clone();

    let scorer: GateScorer = std::sync::Arc::new(move |path, content, _input| {
        let mut guard = engine_holder.lock_guard();
        if guard.is_none() {
            *guard = Some(GateEngine::new(
                workspace_str.clone(),
                lang_for_engine.clone(),
            ));
        }
        let Some(engine) = guard.as_mut() else {
            return (100, vec!["gate engine unavailable".into()]);
        };

        let result = engine.check_file(path, content);
        let violations: Vec<String> = result
            .violations
            .iter()
            .map(|v| v.message.clone())
            .collect();

        (result.score, violations)
    });

    GateHook::new(mode)
        .with_scorer(scorer)
        .with_pass_threshold(pass_threshold)
}

/// Build the live enforcement hook from `AppState`.
///
/// Reads the detected language and workspace root off shared state so every
/// caller scores against one consistent root. Mode comes from
/// `OMEGA_GATE_MODE` (default `warn`); threshold 80 per harness scoring.
pub fn gate_hook_from_state(state: &AppState) -> GateHook {
    let language = state.detected_language.lock_guard().clone();
    let workspace = state.workspace_root.clone();
    build_gate_hook(gate_mode_from_env(), workspace, language, 80)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_mode_from_env_defaults_to_warn() {
        // Env var absent in test harness → Warn must be the default so gate
        // activation is observable before it becomes enforcing.
        std::env::remove_var("OMEGA_GATE_MODE");
        assert!(matches!(gate_mode_from_env(), GateHookMode::Warn));
    }

    #[test]
    fn test_build_gate_hook_scorer_scores_clean_content_100() {
        let dir = tempfile::tempdir().unwrap();
        let hook = build_gate_hook(
            GateHookMode::Block,
            dir.path().to_path_buf(),
            Language::Rust,
            80,
        );
        // Exercise the scorer through on_tool_pre with clean Rust content.
        let ctx = tool_harness::HookContext {
            session_id: "t".into(),
            turn_id: None,
            workspace: dir.path().to_path_buf(),
        };
        let input = tool_harness::ToolInput {
            tool: "write".into(),
            args: serde_json::json!({"filePath": "clean.rs", "content": "fn main() { println!(\"hi\"); }\n"}),
        };
        let decision = tool_harness::Hook::on_tool_pre(&hook, &ctx, "write", &input);
        assert!(matches!(decision, tool_harness::HookDecision::Allow));
    }

    #[test]
    fn test_build_gate_hook_blocks_golden_violation() {
        let dir = tempfile::tempdir().unwrap();
        let hook = build_gate_hook(
            GateHookMode::Block,
            dir.path().to_path_buf(),
            Language::Rust,
            80,
        );
        let ctx = tool_harness::HookContext {
            session_id: "t".into(),
            turn_id: None,
            workspace: dir.path().to_path_buf(),
        };
        // Multiple golden violations drive the score below 80.
        let input = tool_harness::ToolInput {
            tool: "write".into(),
            args: serde_json::json!({
                "filePath": "bad.rs",
                "content": "// TODO fix\n// FIXME now\n// HACK hack\nfn CamelCase() {}\n"
            }),
        };
        let decision = tool_harness::Hook::on_tool_pre(&hook, &ctx, "write", &input);
        assert!(
            matches!(decision, tool_harness::HookDecision::Deny(ref m) if m.contains("score")),
            "block mode must deny sub-threshold content, got {:?}",
            decision
        );
    }
}
