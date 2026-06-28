pub mod commands;
pub mod pipeline;
pub mod tui;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

// ─── ChatEmitter Trait ────────────────────────────────────────────────────────

/// Abstraction over where chat tokens get written.
/// CLI uses TerminalPrinter (print! to stdout), Tauri uses TauriEmitter (events).
pub trait ChatEmitter: Send + Sync {
    fn emit_token(&self, token: &str) -> Result<(), String>;
    fn emit_done(&self, full: &str) -> Result<(), String>;
    fn emit_error(&self, error: &str) -> Result<(), String>;
}

/// CLI emitter — buffers tokens and renders markdown on completion.
pub struct TerminalPrinter {
    buffer: Mutex<String>,
}

impl TerminalPrinter {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(String::new()),
        }
    }
}

impl ChatEmitter for TerminalPrinter {
    fn emit_token(&self, token: &str) -> Result<(), String> {
        self.buffer.lock().map_err(|e| e.to_string())?.push_str(token);
        Ok(())
    }
    fn emit_done(&self, _full: &str) -> Result<(), String> {
        let text = self.buffer.lock().map_err(|e| e.to_string())?.clone();
        self.buffer.lock().map_err(|e| e.to_string())?.clear();
        let rendered = tui::markdown::render_markdown(&text);
        print!("{}", rendered);
        use std::io::Write;
        std::io::stdout().flush().map_err(|e| e.to_string())
    }
    fn emit_error(&self, error: &str) -> Result<(), String> {
        self.buffer.lock().map_err(|e| e.to_string())?.clear();
        eprintln!("{}", error);
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
