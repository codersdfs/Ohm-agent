//! End-to-end Gate enforcement tests.
//!
//! Proves the wiring chain `execute_tool_inner` → `ExecutionPipeline` →
//! `GateHook` → `harness::GateEngine` actually blocks sub-threshold writes
//! BEFORE they reach disk, and preserves the negative-knowledge promotion loop.
//!
//! Env note: tests serialize on a shared async mutex because they flip
//! `OMEGA_GATE_MODE`; each scenario builds its own `AppState`, whose private
//! `OnceLock` pipeline picks up the current mode at construction.

use omega_core::commands::tools::execute_tool_inner;
use omega_core::{AppState, MutexExt};
use tool_harness::ToolRequest;

/// Serializes tests that mutate OMEGA_GATE_MODE (tokio Mutex: held across .await).
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn set_mode(mode: &str) {
    std::env::set_var("OMEGA_GATE_MODE", mode);
}

fn state_in(temp: &tempfile::TempDir, lang: harness::Language) -> AppState {
    let mut state = AppState::new(":memory:");
    state.workspace_root = temp.path().to_path_buf();
    *state.detected_language.lock_guard() = lang;
    state
}

fn write_request(path: &std::path::Path, content: &str) -> ToolRequest {
    ToolRequest {
        tool: "write".into(),
        args: serde_json::json!({
            "filePath": path.to_string_lossy(),
            "content": content,
        }),
    }
}

const CLEAN_RUST: &str = "fn main() {\n    println!(\"hello\");\n}\n";

const VIOLATING_RUST: &str = "// TODO fix this\n// FIXME broken\n// HACK nasty\nunsafe { let p: *const u8 = std::ptr::null(); }\n";

#[tokio::test]
async fn test_gate_blocks_write_with_golden_violation() {
    let _env = ENV_LOCK.lock().await;
    set_mode("block");
    let temp = tempfile::tempdir().unwrap();
    let state = state_in(&temp, harness::Language::Rust);

    let target = temp.path().join("bad.rs");
    let request = write_request(&target, VIOLATING_RUST);

    let result = execute_tool_inner(&state, request).await.unwrap();

    assert!(
        !result.success,
        "sub-threshold write must be rejected in block mode"
    );
    let err = result.error.unwrap_or_default();
    assert!(
        err.contains("blocked by hook") && err.contains("score"),
        "rejection should surface the gate verdict, got: {}",
        err
    );
    assert!(
        !target.exists(),
        "BLOCKED WRITE MUST NOT BE ON DISK — this is the core Gate guarantee"
    );
}

#[tokio::test]
async fn test_gate_allows_clean_write() {
    let _env = ENV_LOCK.lock().await;
    set_mode("block");
    let temp = tempfile::tempdir().unwrap();
    let state = state_in(&temp, harness::Language::Rust);

    let target = temp.path().join("clean.rs");
    let request = write_request(&target, CLEAN_RUST);

    let result = execute_tool_inner(&state, request).await.unwrap();

    assert!(
        result.success,
        "clean content must pass in block mode too (false-positive guard): {:?}",
        result.error
    );
    assert!(target.exists(), "approved write must reach disk");
}

#[tokio::test]
async fn test_gate_warn_mode_allows_but_reports() {
    let _env = ENV_LOCK.lock().await;
    set_mode("warn");
    let temp = tempfile::tempdir().unwrap();
    let state = state_in(&temp, harness::Language::Rust);

    let target = temp.path().join("warned.rs");
    let request = write_request(&target, VIOLATING_RUST);

    let result = execute_tool_inner(&state, request).await.unwrap();

    assert!(result.success, "warn mode must allow the write");
    assert!(target.exists(), "warn mode must let the file land on disk");
    assert!(
        result.gate_result.is_some(),
        "write results must carry a gate record for reporting"
    );
}

#[tokio::test]
async fn test_gate_violation_promoted_after_three() {
    // Negative-knowledge loop: promote_or_increment at freq >= 3 lands the
    // pattern in the RulesDatabase; run_gate's bookkeeping check then flags it.
    let _env = ENV_LOCK.lock().await;
    set_mode("warn");
    let temp = tempfile::tempdir().unwrap();
    let state = state_in(&temp, harness::Language::Rust);

    let pattern = "// TODO definitely-banned-token";
    let content = format!("{}\n{}", pattern, CLEAN_RUST);
    let target = temp.path().join("promote_me.rs");

    // Three encounters with the same violation message → auto-promotion.
    let mut db = state.rules_db.lock_guard();
    for _ in 0..3 {
        db.promote_or_increment(
            &harness::Language::Rust,
            "golden",
            pattern,
            "TODO definitely-banned-token",
            "error",
        );
    }
    drop(db);

    let flagged = state
        .rules_db
        .lock_guard()
        .check_content(&content, &harness::Language::Rust);
    assert!(
        !flagged.is_empty(),
        "pattern seen three times must be promoted into the rules DB"
    );

    // The promoted rule surfaces in subsequent write results.
    let request = write_request(&target, &content);
    let result = execute_tool_inner(&state, request).await.unwrap();
    match result.gate_result {
        Some(gate) => {
            assert!(
                gate.passed || !gate.violations.is_empty(),
                "a passing-but-flagged or failing gate record must list violations"
            );
        }
        None => panic!("write result must carry a gate record"),
    }
}
