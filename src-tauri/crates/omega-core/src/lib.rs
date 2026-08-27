pub mod code_search;
pub mod commands;
pub mod context_manager;
pub mod error;
pub mod events;
pub mod gate_hook;
pub mod learning;
pub mod memory_injector;
pub mod memory_retriever;
pub mod memory_summarizer;
pub mod pipeline;
pub mod session;
pub mod subagent;
pub mod tui;

// ui module for permission panel and related UI components
pub mod ui;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
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
    fn emit_thinking(&self, _token: &str) -> Result<(), String> {
        Ok(())
    }
    /// Called when thinking is complete. `full` is the entire thinking text.
    fn emit_thinking_done(&self, _full: &str) -> Result<(), String> {
        Ok(())
    }
    /// Called when a tool call starts. `args` is the JSON arguments string.
    fn emit_tool_call(&self, _name: &str, _args: &str) -> Result<(), String> {
        Ok(())
    }
    /// Called when a tool call completes. `success` and `output` describe the result.
    fn emit_tool_result(&self, _name: &str, _success: bool, _output: &str) -> Result<(), String> {
        Ok(())
    }

    /// Whether shared command code may write diagnostics directly to the
    /// process terminal. Full-screen TUI emitters must keep this false because
    /// stdout/stderr writes bypass Ratatui and corrupt the alternate screen.
    fn allows_direct_terminal_output(&self) -> bool {
        false
    }
}

/// CLI emitter — streams tokens live, ensures a final newline on done.
pub struct TerminalPrinter;

impl TerminalPrinter {
    pub fn new() -> Self {
        Self
    }
}

impl ChatEmitter for TerminalPrinter {
    fn allows_direct_terminal_output(&self) -> bool {
        true
    }

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

// ─── Context-manager cache ────────────────────────────────────────────────────

/// A context manager cached across turns so the workspace repo-map is parsed
/// (tree-sitter) once per session rather than on every user message.
pub struct CachedContextManager {
    pub model: String,
    pub window: u64,
    pub manager: crate::context_manager::ContextManager,
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
    /// Cached context manager keyed by (model, window). Reused across turns so
    /// the repo-map index is built once per session, not per user message.
    pub context_cache: Mutex<Option<CachedContextManager>>,
    /// Broadcast channel for permission requests (Tauri forwards to frontend).
    pub permission_tx: tokio::sync::broadcast::Sender<PermissionEvent>,

    /// Shared tool-execution pipeline, initialized once.
    pub tool_pipeline: OnceLock<tool_harness::ExecutionPipeline>,

    /// Optional conversation session store (CLI / TUI sets this at startup).
    /// Headless/API paths leave it empty so persistence is a no-op.
    pub session_store: Mutex<Option<session::SessionStore>>,

    /// Tools currently executing (chat loop), for live header chips.
    /// Pushed before execution, removed after; cleared at turn boundaries.
    pub running_tools: Mutex<Vec<String>>,

    /// Canonical workspace root. The Gate hook, HookContext, and context
    /// assembly must all score/index against this single root — deriving it
    /// independently per call site risks the Gate watching the wrong tree.
    pub workspace_root: PathBuf,
}

impl AppState {
    pub fn new(db_path: &str) -> Self {
        Self::new_with_provider_config(db_path, providers::ProviderConfig::default())
    }

    pub fn new_with_provider_config(
        db_path: &str,
        provider_config: providers::ProviderConfig,
    ) -> Self {
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
            context_cache: Mutex::new(None),
            permission_tx,
            session_store: Mutex::new(None),
            running_tools: Mutex::new(Vec::new()),
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// Attach a conversation session store (CLI startup). Replaces any previous handle.
    pub fn set_session_store(&self, store: session::SessionStore) {
        *self.session_store.lock_guard() = Some(store);
    }

    /// Persist a conversation snapshot if a session store is attached.
    /// Sync I/O under a short lock; never held across await points by callers.
    pub fn persist_session(&self, messages: &[providers::ChatMessage]) -> Result<(), String> {
        let mut guard = self.session_store.lock_guard();
        match guard.as_mut() {
            Some(store) => store.persist_messages(messages),
            None => Ok(()),
        }
    }

    /// Durably clear the attached session file (if any).
    pub fn clear_session(&self) -> Result<(), String> {
        let mut guard = self.session_store.lock_guard();
        match guard.as_mut() {
            Some(store) => store.clear(),
            None => Ok(()),
        }
    }

    /// Current session id, if a store is attached.
    pub fn session_id(&self) -> Option<String> {
        self.session_store
            .lock_guard()
            .as_ref()
            .map(|s| s.id.clone())
    }

    /// Build (or reuse) a context manager for `model`/`window` and prepare
    /// `messages` for a provider call — injecting the repo-map and JIT Hermes
    /// memory. The manager is cached across turns so the repo-map is indexed
    /// once per session (mirroring the per-run construction in subagent.rs).
    ///
    /// `window`/`model` must be stable within a session for the cache to hit.
    pub fn assemble_context(
        &self,
        workspace_root: impl std::convert::AsRef<std::path::Path>,
        window: u64,
        model: &str,
        messages: &mut Vec<providers::ChatMessage>,
        user_message: &str,
    ) -> Result<crate::context_manager::AssembledContext, String> {
        const KEEP_LAST_TURNS: usize = 6;
        let store = self.memory_store.lock_guard();
        let mut guard = self.context_cache.lock_guard();

        match guard.as_ref() {
            // Reuse the cached manager (repo-map already indexed).
            Some(cached) if cached.window == window && cached.model == model => {
                cached.manager.prepare(messages, Some(&store), user_message)
            }
            // New provider/model/window: build a fresh manager and cache it.
            _ => {
                let manager = crate::context_manager::ContextManager::new(
                    workspace_root,
                    window,
                    model,
                    KEEP_LAST_TURNS,
                );
                let assembled = manager.prepare(messages, Some(&store), user_message)?;
                *guard = Some(CachedContextManager {
                    model: model.to_string(),
                    window,
                    manager,
                });
                Ok(assembled)
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assemble_context_caches_and_reuses_manager() {
        let state = AppState::new(":memory:");
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("a.rs"), "pub fn alpha() {}\n").unwrap();
        let root = temp.path();

        let mut msgs: Vec<providers::ChatMessage> = vec![];
        let _ = state
            .assemble_context(root, 128_000, "gpt-4o", &mut msgs, "hi")
            .unwrap();

        // First call populates the cache.
        {
            let _guard = state.context_cache.lock_guard();
            let cached = _guard.as_ref();
            assert!(
                cached.is_some(),
                "cache should populate after first assemble"
            );
            assert_eq!(cached.map(|c| c.window), Some(128_000));
        }

        // Same model+window reuses the cached manager (no error, cache kept).
        let _ = state
            .assemble_context(root, 128_000, "gpt-4o", &mut msgs, "again")
            .unwrap();
        {
            let _guard = state.context_cache.lock_guard();
            let cached_after = _guard.as_ref();
            assert_eq!(cached_after.map(|c| c.window), Some(128_000));
        }

        // A different window forces a rebuild and updates the cache.
        let _ = state
            .assemble_context(root, 32_000, "gpt-4o", &mut msgs, "again2")
            .unwrap();
        {
            let _guard = state.context_cache.lock_guard();
            let cached_after_change = _guard.as_ref();
            assert_eq!(cached_after_change.map(|c| c.window), Some(32_000));
        }
    }
}
