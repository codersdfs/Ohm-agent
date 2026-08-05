//! Concrete GateHook wiring — bridges tool-harness's GateHook to the real
//! harness::engine::GateEngine.
//!
//! `tool-harness` provides the `GateHook` trait and `GateScorer` callback type,
//! but does not depend on `harness` (to avoid a hard dependency). This module
//! provides the real scorer that calls `GateEngine::check_file()`.

use std::path::PathBuf;
use std::sync::Mutex;

use harness::{GateEngine, Language};
use tool_harness::{GateHook, GateHookMode, GateScorer};

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
    // outlives a single call, we store the engine in a shared Mutex.
    //
    // Each call clones the Arc and accesses the shared engine.
    let engine_holder: std::sync::Arc<Mutex<Option<GateEngine>>> = std::sync::Arc::new(Mutex::new(None));
    let workspace_str = workspace.to_string_lossy().to_string();

    let scorer: GateScorer = std::sync::Arc::new(move |path, content, _input| {
        let mut guard = engine_holder.lock().unwrap();
        if guard.is_none() {
            *guard = Some(GateEngine::new(workspace_str.clone(), language.clone()));
        }
        let engine = guard.as_mut().unwrap();

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
