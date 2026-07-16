pub mod commands;
pub mod pipeline;
pub mod tui;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

// ─── Poison-safe Mutex extension ─────────────────────────────────────────────

pub trait MutexExt<T> {
    fn lock_guard(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_guard(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                log::error!("Mutex poisoned — recovering");
                poisoned.into_inner()
            }
        }
    }
}

// ─── ChatEmitter Trait ────────────────────────────────────────────────────────

/// Abstraction over where chat tokens get written.
/// CLI uses TerminalPrinter (print! to stdout), Tauri uses TauriEmitter (events).
pub trait ChatEmitter: Send + Sync {
    fn emit_token(&self, token: &str) -> Result<(), String>;
    fn emit_done(&self, full: &str) -> Result<(), String>;
    fn emit_error(&self, error: &str) -> Result<(), String>;

    /// Called when the model emits a thinking/reasoning token.
    fn emit_thinking(&self, _token: &str) -> Result<(), String> { Ok(()) }
    /// Called when thinking is complete. `full` is the entire thinking text.
    fn emit_thinking_done(&self, _full: &str) -> Result<(), String> { Ok(()) }
    /// Called when a tool call starts. `args` is the JSON arguments string.
    fn emit_tool_call(&self, _name: &str, _args: &str) -> Result<(), String> { Ok(()) }
    /// Called when a tool call completes. `success` and `output` describe the result.
    fn emit_tool_result(&self, _name: &str, _success: bool, _output: &str) -> Result<(), String> { Ok(()) }
}

/// CLI emitter — streams tokens live, ensures a final newline on done.
pub struct TerminalPrinter;

impl TerminalPrinter {
    pub fn new() -> Self {
        Self
    }
}

impl ChatEmitter for TerminalPrinter {
    fn emit_token(&self, token: &str) -> Result<(), String> {
        use std::io::Write;
        print!("{}", token);
        std::io::stdout().flush().map_err(|e| e.to_string())
    }
    fn emit_done(&self, full: &str) -> Result<(), String> {
        // Already printed token-by-token; just ensure a final newline
        if !full.ends_with('\n') {
            println!();
        }
        Ok(())
    }
    fn emit_error(&self, error: &str) -> Result<(), String> {
        eprintln!("{}", error);
        Ok(())
    }
    fn emit_tool_call(&self, name: &str, args: &str) -> Result<(), String> {
        eprintln!("  ▶ {} {}", name, args);
        Ok(())
    }
    fn emit_tool_result(&self, name: &str, success: bool, output: &str) -> Result<(), String> {
        if success {
            eprintln!("  ✓ {} → {}", name, output);
        } else {
            eprintln!("  ✗ {} → {}", name, output);
        }
        Ok(())
    }
}

// ─── Permission Event ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionEvent {
    pub request_id: String,
    pub tool: String,
    pub args: serde_json::Value,
    pub reason: String,
    pub step_id: u32,
    pub step_description: String,
}

// ─── AppState ─────────────────────────────────────────────────────────────────

pub struct AppState {
    pub pipeline: Arc<tokio::sync::Mutex<pipeline::PipelineState>>,
    pub provider_config: Mutex<providers::ProviderConfig>,
    pub review_config: Mutex<pipeline::ReviewConfig>,
    pub rules_db: Mutex<harness::rules::RulesDatabase>,
    pub detected_language: Mutex<harness::Language>,
    pub db_path: String,
    pub build_config: Mutex<pipeline::BuildConfig>,
    pub pending_permissions: Mutex<HashSet<String>>,
    pub permission_results: Mutex<HashMap<String, bool>>,
    pub session_log: Mutex<Vec<pipeline::build::BuildSessionEntry>>,
    pub memory_store: Mutex<memory::MemoryStore>,
    /// Broadcast channel for permission requests (Tauri forwards to frontend).
    pub permission_tx: tokio::sync::broadcast::Sender<PermissionEvent>,

    /// Shared tool-execution pipeline, initialized once.
    pub tool_pipeline: OnceLock<tool_harness::ExecutionPipeline>,
}

impl AppState {
    pub fn new(db_path: &str) -> Self {
        Self::new_with_provider_config(db_path, providers::ProviderConfig::default())
    }

    pub fn new_with_provider_config(db_path: &str, provider_config: providers::ProviderConfig) -> Self {
        let task_id = uuid::Uuid::new_v4().to_string();
        let memory_store =
            memory::MemoryStore::new(db_path).expect("Failed to initialise memory store");
        let (permission_tx, _) = tokio::sync::broadcast::channel(32);
        Self {
            pipeline: Arc::new(tokio::sync::Mutex::new(pipeline::PipelineState::new(
                task_id,
            ))),
            provider_config: Mutex::new(provider_config),
            review_config: Mutex::new(pipeline::ReviewConfig::default()),
            rules_db: Mutex::new(harness::rules::RulesDatabase::new()),
            detected_language: Mutex::new(harness::Language::TypeScriptReact),
            db_path: db_path.to_string(),
            build_config: Mutex::new(pipeline::BuildConfig::default()),
            pending_permissions: Mutex::new(HashSet::new()),
            permission_results: Mutex::new(HashMap::new()),
            tool_pipeline: OnceLock::new(),
            session_log: Mutex::new(vec![]),
            memory_store: Mutex::new(memory_store),
            permission_tx,
        }
    }
}

pub fn default_db_path() -> String {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "omega", "omega-agent") {
        let data_dir = proj_dirs.data_dir();
        let path = data_dir.join("memory.db");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        path.to_string_lossy().to_string()
    } else {
        let path = std::path::PathBuf::from(".").join("memory.db");
        path.to_string_lossy().to_string()
    }
}
