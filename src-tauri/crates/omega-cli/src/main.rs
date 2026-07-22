// ── Omega Agent TUI ───────────────────────────────────────────────────────────
// Ratatui + ratata full-screen terminal UI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use ratata::prelude::*;

use omega_core::session::SessionStore;
use omega_core::tui::component::{Action, Component, UiStreamEvent};
use omega_core::tui::editor::{EditorMode, EditorState};
use omega_core::tui::spinner::{OmegaSpinner, SpinnerState};
use omega_core::tui::status::StatusState;
use omega_core::tui::theme;
use omega_core::tui::transcript::{self, Transcript, TranscriptEntry};
use omega_core::{commands, default_db_path, AppState, ChatEmitter};

// ── Event types for streaming ────────────────────────────────────────────────

/// ChatEmitter impl that sends events through an mpsc channel.
struct ChannelEmitter {
    tx: tokio::sync::mpsc::UnboundedSender<UiStreamEvent>,
}

impl ChannelEmitter {
    fn new(tx: tokio::sync::mpsc::UnboundedSender<UiStreamEvent>) -> Self {
        Self { tx }
    }
}

impl ChatEmitter for ChannelEmitter {
    fn emit_token(&self, token: &str) -> std::result::Result<(), String> {
        let _ = self.tx.send(UiStreamEvent::Token(token.to_string()));
        Ok(())
    }
    fn emit_thinking(&self, token: &str) -> std::result::Result<(), String> {
        let _ = self.tx.send(UiStreamEvent::Thinking(token.to_string()));
        Ok(())
    }
    fn emit_thinking_done(&self, _full: &str) -> std::result::Result<(), String> {
        let _ = self.tx.send(UiStreamEvent::ThinkingDone);
        Ok(())
    }
    fn emit_tool_call(&self, name: &str, args: &str) -> std::result::Result<(), String> {
        let _ = self.tx.send(UiStreamEvent::ToolCall {
            name: name.to_string(),
            args: args.to_string(),
        });
        Ok(())
    }
    fn emit_tool_result(
        &self,
        name: &str,
        success: bool,
        output: &str,
    ) -> std::result::Result<(), String> {
        let _ = self.tx.send(UiStreamEvent::ToolResult {
            name: name.to_string(),
            success,
            output: output.to_string(),
        });
        Ok(())
    }
    fn emit_done(&self, _full: &str) -> std::result::Result<(), String> {
        Ok(())
    }
    fn emit_error(&self, error: &str) -> std::result::Result<(), String> {
        let _ = self.tx.send(UiStreamEvent::Error(error.to_string()));
        Ok(())
    }
}

// ── App state ────────────────────────────────────────────────────────────────

struct App {
    // Core state
    state: Arc<AppState>,
    config: providers::ProviderConfig,

    // UI state
    transcript: Transcript,
    editor: EditorState,
    status: StatusState,

    // LLM conversation history is now inside Transcript

    // Streaming
    is_streaming: bool,
    cancel_flag: Arc<AtomicBool>,

    // Animation tick
    anim_tick: u64,
    last_tick: Instant,

    // Input history
    history: Vec<String>,
    history_index: Option<usize>,

    // Cost tracking
    session_tokens_in: u64,
    session_tokens_out: u64,
    session_messages: u64,

    // Help overlay
    show_help: bool,

    // Provider panel
    show_provider_panel: bool,
    provider_panel_state: omega_core::tui::provider_panel::ProviderPanelState,

    // Command palette
    show_command_palette: bool,
    command_palette: omega_core::tui::command_palette::CommandPaletteState,

    // Sidebar visibility
    show_sidebar: bool,

    // Global write/edit preview expansion
    tool_output_expanded: bool,

    // Should quit
    should_quit: bool,
}

impl App {
    fn new(
        config: providers::ProviderConfig,
        session: SessionStore,
        load: omega_core::session::SessionLoad,
    ) -> Self {
        let state = Arc::new(AppState::new_with_provider_config(
            &default_db_path(),
            config.clone(),
        ));
        // Single ownership: session lives on AppState (poison-safe Mutex).
        // Chat loop flushes via AppState::persist_session; /clear uses clear_session.
        let session_id = session.id.clone();
        state.set_session_store(session);
        let _model = config.model.clone();
        let _kind = format!("{}", config.kind);
        let editor = EditorState::new();
        let status = StatusState::new();
        let resumed = load.resumed;
        let msg_count = load.messages.len();
        let warnings = load.warnings.clone();

        let cfg_for_panel = config.clone();
        let mut app = Self {
            state,
            config,
            transcript: Transcript::new(),
            editor,
            status,
            is_streaming: false,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            anim_tick: 0,
            last_tick: Instant::now(),

            history: Vec::new(),
            history_index: None,

            session_tokens_in: 0,
            session_tokens_out: 0,
            session_messages: 0,

            show_help: false,
            show_provider_panel: false,
            show_command_palette: false,
            command_palette: omega_core::tui::command_palette::CommandPaletteState::new(),
            show_sidebar: true,
            tool_output_expanded: false,
            provider_panel_state: omega_core::tui::provider_panel::ProviderPanelState::from_config(
                &cfg_for_panel,
            ),

            should_quit: false,
        };

        // Welcome notice
        app.transcript.entries.push(TranscriptEntry::Notice {
            text: format!(
                "Ω v{} — {} ({}). Type a message to start.",
                env!("CARGO_PKG_VERSION"),
                app.config.model,
                app.config.kind
            ),
            is_error: false,
        });

        // Session resume / new notice
        if resumed {
            app.transcript.entries.push(TranscriptEntry::Notice {
                text: format!(
                    "Resumed session {} ({} messages)",
                    &session_id[..session_id.len().min(8)],
                    msg_count
                ),
                is_error: false,
            });
            app.transcript.load_from_session(load.messages);
        } else {
            app.transcript.entries.push(TranscriptEntry::Notice {
                text: format!("New session {}", &session_id[..session_id.len().min(8)]),
                is_error: false,
            });
        }
        for w in warnings {
            app.transcript.entries.push(TranscriptEntry::Notice {
                text: format!("Session load: {w}"),
                is_error: true,
            });
        }

        // Show setup hint when API key is needed for cloud providers
        let is_local = matches!(app.config.kind, providers::ProviderKind::Local);
        if app.config.api_key.is_none() && !is_local {
            app.transcript.entries.push(TranscriptEntry::Notice {
                text: "No API key found. Set OMEGA_API_KEY or run: omega -p local".into(),
                is_error: true,
            });
        }

        // Load MCP skills
        let (mcp_loaded, mcp_errors) = commands::mcp::load_skills();
        if mcp_loaded > 0 {
            app.transcript.entries.push(TranscriptEntry::Notice {
                text: format!("MCP: {} skills loaded", mcp_loaded),
                is_error: false,
            });
        }
        for err in &mcp_errors {
            app.transcript.entries.push(TranscriptEntry::Notice {
                text: format!("MCP: {}", err),
                is_error: true,
            });
        }

        app
    }

    /// Handle a key event.
    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        // Global shortcuts
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.is_streaming {
                    self.cancel_streaming();
                } else if self.show_command_palette {
                    self.command_palette.close();
                    self.show_command_palette = false;
                } else {
                    self.should_quit = true;
                }
                return;
            }
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            _ => {}
        }

        if self.is_streaming {
            // Only allow Ctrl-C during streaming
            return;
        }

        // Provider panel takes over all key handling
        if self.show_provider_panel {
            let action =
                omega_core::tui::provider_panel::handle_key(&mut self.provider_panel_state, key);
            match action {
                omega_core::tui::provider_panel::PanelAction::Apply => {
                    let new_config = self.provider_panel_state.to_config(&self.config);
                    self.config = new_config.clone();
                    save_config(&self.config);
                    save_api_key(self.config.api_key.as_deref());
                    self.transcript.entries.push(TranscriptEntry::Notice {
                        text: format!(
                            "Provider set to {} ({})",
                            self.config.model, self.config.kind
                        ),
                        is_error: false,
                    });
                    self.show_provider_panel = false;
                }
                omega_core::tui::provider_panel::PanelAction::Close => {
                    self.show_provider_panel = false;
                }
                omega_core::tui::provider_panel::PanelAction::None => {}
            }
            return;
        }

        // Command palette takes over key handling
        if self.show_command_palette {
            let action = omega_core::tui::command_palette::handle_key(
                &mut self.command_palette,
                key,
            );
            match action {
                omega_core::tui::command_palette::PaletteAction::Select(id) => {
                    self.command_palette.close();
                    self.show_command_palette = false;
                    self.handle_slash_command(id);
                }
                omega_core::tui::command_palette::PaletteAction::Close => {
                    self.command_palette.close();
                    self.show_command_palette = false;
                }
                omega_core::tui::command_palette::PaletteAction::None => {}
            }
            return;
        }

        // Toggle help overlay
        if key.code == KeyCode::Char('?') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            self.show_help = !self.show_help;
            self.editor.suggestions.clear();
            return;
        }

        // Ctrl+K: open command palette
        if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_command_palette("");
            return;
        }

        // Ctrl+B: toggle sidebar visibility
        if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.show_sidebar = !self.show_sidebar;
            return;
        }

        // Ctrl+E: globally expand/collapse bounded write and edit previews.
        if key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.tool_output_expanded = !self.tool_output_expanded;
            self.transcript
                .set_tools_expanded(self.tool_output_expanded);
            return;
        }

        // Empty-buffer `/` opens the command palette instead of inserting.
        if key.code == KeyCode::Char('/')
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && self.editor.buffer.is_empty()
        {
            self.open_command_palette("/");
            return;
        }

        // Delegate to editor component (handles letters, Enter, navigation, Tab)
        let action = self.editor.handle_key(key);
        match action {
            Action::SendMessage => self.submit_message(),
            _ => {}
        }

        // Scroll keys (also handled at App level for dual history+scroll binding)
        match key.code {
            KeyCode::Up => {
                self.recall_history_up();
                transcript::scroll_up(&mut self.transcript.scroll, 3);
            }
            KeyCode::Down => {
                self.recall_history_down();
                transcript::scroll_down(
                    &mut self.transcript.scroll,
                    self.transcript.entries.len(),
                    3,
                );
            }
            KeyCode::PageUp => {
                transcript::scroll_up(&mut self.transcript.scroll, 10);
            }
            KeyCode::PageDown => {
                transcript::scroll_down(
                    &mut self.transcript.scroll,
                    self.transcript.entries.len(),
                    10,
                );
            }
            _ => {}
        }
    }

    /// Cancel the current streaming request.
    fn cancel_streaming(&mut self) {
        self.cancel_flag.store(true, Ordering::SeqCst);

        // Drop the receiver so the streaming task's tx.send() fails
        self.transcript.stream_event_rx = None;

        self.is_streaming = false;
        self.editor.state = EditorMode::Idle;
        self.editor.buffer.clear();
        self.editor.cursor = 0;
        self.status.set_spinner_state(SpinnerState::Idle);

        // Mark the pending assistant entry as stopped
        for entry in self.transcript.entries.iter_mut().rev() {
            if let TranscriptEntry::Assistant {
                ref mut is_streaming,
                ..
            } = entry
            {
                *is_streaming = false;
                break;
            }
        }

        // Show cancel notice
        self.transcript.entries.push(TranscriptEntry::Notice {
            text: "Stream cancelled".into(),
            is_error: false,
        });

        self.transcript.scroll.auto_scroll = true;
        self.transcript.streaming_fragment.clear();
    }

    /// Navigate input history: move to older entry.
    fn recall_history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                // Enter history: save current buffer
                self.history_index = Some(self.history.len() - 1);
            }
            Some(i) if i > 0 => {
                self.history_index = Some(i - 1);
            }
            _ => return,
        }
        let idx = self.history_index.unwrap();
        self.editor.buffer = self.history[idx].clone();
        self.editor.cursor = self.editor.buffer.len();
    }

    /// Navigate input history: move to newer entry.
    fn recall_history_down(&mut self) {
        match self.history_index {
            Some(i) if i + 1 < self.history.len() => {
                self.history_index = Some(i + 1);
                let idx = self.history_index.unwrap();
                self.editor.buffer = self.history[idx].clone();
                self.editor.cursor = self.editor.buffer.len();
            }
            Some(_) => {
                // Exited history back to empty buffer
                self.history_index = None;
                self.editor.buffer.clear();
                self.editor.cursor = 0;
            }
            None => {}
        }
    }

    /// Handle mouse events for scrolling.
    fn handle_mouse(&mut self, kind: MouseEventKind) {
        match kind {
            MouseEventKind::ScrollDown => {
                transcript::scroll_down(
                    &mut self.transcript.scroll,
                    self.transcript.entries.len(),
                    3,
                );
            }
            MouseEventKind::ScrollUp => {
                transcript::scroll_up(&mut self.transcript.scroll, 3);
            }
            _ => {}
        }
    }

    /// Submit the current editor buffer as a message.
    fn submit_message(&mut self) {
        let content = self.editor.take_buffer();
        if content.trim().is_empty() {
            return;
        }

        // Save to input history (deduplicate against last entry)
        if self.history.last().map(|s| s.as_str()) != Some(content.as_str()) {
            self.history.push(content.clone());
        }
        self.history_index = None;

        // Handle slash commands
        if content.starts_with('/') {
            self.handle_slash_command(&content);
            return;
        }

        // Add user message to transcript
        self.transcript.entries.push(TranscriptEntry::User {
            content: content.clone(),
        });

        // Start streaming
        self.start_streaming(content);
    }

    fn open_command_palette(&mut self, seed_query: &str) {
        if self.is_streaming || self.show_provider_panel {
            return;
        }
        self.show_help = false;
        self.command_palette.open(seed_query);
        self.show_command_palette = true;
    }

    /// Handle a slash command.
    fn handle_slash_command(&mut self, cmd: &str) {
        match cmd.to_lowercase().trim() {
            "/help" | "/?" | "/h" => {
                self.transcript.entries.push(TranscriptEntry::Notice {
                    text: "Commands: /help, /clear, /tools, /model <name>, /exit, /cost".into(),
                    is_error: false,
                });
            }
            "/clear" | "/cls" => {
                self.transcript.entries.clear();
                self.transcript.messages.clear();
                self.editor.buffer.clear();
                match self.state.clear_session() {
                    Ok(()) => {
                        self.transcript.entries.push(TranscriptEntry::Notice {
                            text: "Session cleared.".into(),
                            is_error: false,
                        });
                    }
                    Err(e) => {
                        log::error!("session clear failed: {e}");
                        self.transcript.entries.push(TranscriptEntry::Notice {
                            text: format!("Failed to clear session file: {e}"),
                            is_error: true,
                        });
                    }
                }
            }
            "/tools" => match commands::tools::list_tools() {
                Ok(tools) => {
                    let list = tools.join(", ");
                    self.transcript.entries.push(TranscriptEntry::Notice {
                        text: format!("Available tools: {}", list),
                        is_error: false,
                    });
                }
                Err(e) => {
                    self.transcript.entries.push(TranscriptEntry::Notice {
                        text: format!("Error listing tools: {}", e),
                        is_error: true,
                    });
                }
            },
            "/model" => {
                if self.is_streaming {
                    self.transcript.entries.push(TranscriptEntry::Notice {
                        text: "Can't open provider panel while streaming.".into(),
                        is_error: true,
                    });
                } else {
                    // Model-first: jump straight to model picker for current provider.
                    self.provider_panel_state =
                        omega_core::tui::provider_panel::ProviderPanelState::from_config_at(
                            &self.config,
                            omega_core::tui::provider_panel::WizardStep::Model,
                        );
                    self.show_provider_panel = true;
                }
            }
            "/provider" | "/providers" | "/p" => {
                if self.is_streaming {
                    self.transcript.entries.push(TranscriptEntry::Notice {
                        text: "Can't open provider panel while streaming.".into(),
                        is_error: true,
                    });
                } else {
                    // Provider list is step 1 — show that when user asks for providers.
                    self.provider_panel_state =
                        omega_core::tui::provider_panel::ProviderPanelState::from_config(
                            &self.config,
                        );
                    self.show_provider_panel = true;
                }
            }
            "/cost" => {
                self.transcript.entries.push(TranscriptEntry::Notice {
                    text: format!(
                        "Session tokens — {} in / {} out ({} messages)",
                        self.session_tokens_in, self.session_tokens_out, self.session_messages
                    ),
                    is_error: false,
                });
            }
            "/exit" | "/quit" => {
                self.should_quit = true;
            }
            other => {
                self.transcript.entries.push(TranscriptEntry::Notice {
                    text: format!("Unknown command: {}. Type /help for commands.", other),
                    is_error: true,
                });
            }
        }
    }

    /// Start streaming a response from the LLM.
    fn start_streaming(&mut self, content: String) {
        self.is_streaming = true;
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.editor.state = EditorMode::Thinking;
        self.status.set_spinner_state(SpinnerState::Thinking);

        // Create channel
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.transcript.stream_event_rx = Some(rx);

        // Add a placeholder assistant entry
        self.transcript.entries.push(TranscriptEntry::Assistant {
            content: String::new(),
            rendered: None,
            is_streaming: true,
            thinking: String::new(),
        });

        // Get references for the async task
        let state = self.state.clone();
        let config = self.config.clone();
        let system_prompt = commands::tools::default_system_prompt();
        let permission_mode = "off".to_string();

        // Shared message list for the task to modify
        let messages = Arc::new(tokio::sync::Mutex::new(self.transcript.messages.clone()));
        let cancel_flag = self.cancel_flag.clone();

        let event_tx = tx.clone();

        // Spawn the streaming task
        tokio::spawn(async move {
            // Check cancellation before starting
            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }

            let emitter = ChannelEmitter::new(event_tx.clone());

            let request = commands::chat::StreamMessageRequest {
                content,
                agent_type: "chat".into(),
                provider: Some(config.clone()),
                system_prompt: Some(system_prompt),
                permission_mode,
                show_progress: false,
                max_tool_loops: None,
            };

            let (tokens_in_before, tokens_out_before) =
                omega_core::commands::chat::session_token_counts();

            let (result, saved_msgs) = {
                let mut msgs = messages.lock().await;
                // Session flushes happen inside stream_message_with_history_cancel
                // (user msg, each tool round, final assistant) via AppState.
                let r = commands::chat::stream_message_with_history_cancel(
                    &state,
                    request,
                    &emitter,
                    &mut msgs,
                    Some(cancel_flag.clone()),
                )
                .await;
                // Capture the updated conversation history before releasing the lock
                (r, msgs.clone())
            };

            // Check cancellation (don't send events if cancelled)
            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }

            // Delta recorded by chat::record_cost during the stream.
            let (tokens_in_after, tokens_out_after) =
                omega_core::commands::chat::session_token_counts();
            let tokens_in = tokens_in_after.saturating_sub(tokens_in_before) as u32;
            let tokens_out = tokens_out_after.saturating_sub(tokens_out_before) as u32;

            // Send done event with result
            match result {
                Ok(full) => {
                    let _ = event_tx.send(UiStreamEvent::Done {
                        full,
                        tokens_in,
                        tokens_out,
                        messages: saved_msgs,
                    });
                }
                Err(e) => {
                    let _ = event_tx.send(UiStreamEvent::Error(e));
                }
            }
        });
    }

    /// Process streaming events from the channel.
    fn process_stream_events(&mut self) {
        let rx = self.transcript.stream_event_rx.take();
        let Some(mut rx) = rx else {
            return;
        };

        let mut done = false;

        while let Ok(event) = rx.try_recv() {
            // Update App-level state from events
            match &event {
                UiStreamEvent::Token(_) => {
                    self.editor.state = EditorMode::Streaming;
                    self.status.set_spinner_state(SpinnerState::Streaming);
                }
                UiStreamEvent::Thinking(_) => {
                    self.editor.state = EditorMode::Thinking;
                    self.status.set_spinner_state(SpinnerState::Thinking);
                }
                UiStreamEvent::ToolCall { .. } => {
                    self.status.set_spinner_state(SpinnerState::ToolCall);
                }
                UiStreamEvent::Done {
                    tokens_in,
                    tokens_out,
                    ..
                } => {
                    self.session_tokens_in += *tokens_in as u64;
                    self.session_tokens_out += *tokens_out as u64;
                    self.session_messages += 1;
                    done = true;
                }
                UiStreamEvent::Error(_) => {
                    self.status.set_spinner_state(SpinnerState::Error);
                    done = true;
                }
                _ => {}
            }

            // Delegate event processing to the transcript component
            let action = self.transcript.process_stream_event(&event);

            // Handle any actions returned by the transcript
            match action {
                Action::StreamDone { .. } | Action::StreamError => {
                    done = true;
                }
                _ => {}
            }
        }

        if done {
            self.is_streaming = false;
            self.editor.state = EditorMode::Idle;
            self.editor.buffer.clear();
            self.editor.cursor = 0;
            self.status.set_spinner_state(SpinnerState::Idle);
            self.transcript.stream_event_rx = None;
            self.transcript.streaming_fragment.clear();
            self.transcript.scroll.auto_scroll = true; // jump to bottom
        } else {
            // Put the rx back if we're still streaming
            self.transcript.stream_event_rx = Some(rx);
        }
    }

    /// Advance the spinner animation.
    fn tick_spinner(&mut self) {
        self.status.tick_spinner();
        self.transcript.tick_activity();
    }

    /// Poll the provider panel model-fetch channel.
    fn poll_provider_models(&mut self) {
        if let Some(rx) = &mut self.provider_panel_state.models_rx {
            match rx.try_recv() {
                Ok(Ok(models)) => {
                    self.provider_panel_state.models = models;
                    self.provider_panel_state.recompute_filter();
                    self.provider_panel_state.models_loading = false;
                    self.provider_panel_state.models_rx = None;
                }
                Ok(Err(e)) => {
                    self.provider_panel_state.models.clear();
                    self.provider_panel_state.models_error = Some(e);
                    self.provider_panel_state.models_loading = false;
                    self.provider_panel_state.models_rx = None;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
                Err(_) => {
                    self.provider_panel_state.models_loading = false;
                    self.provider_panel_state.models_rx = None;
                }
            }
        }

        // Trigger new fetch if needed
        if self.show_provider_panel
            && self.provider_panel_state.needs_fetch
            && self.provider_panel_state.models_rx.is_none()
        {
            self.provider_panel_state.needs_fetch = false;
            self.provider_panel_state.models_loading = true;
            self.provider_panel_state.models.clear();
            self.provider_panel_state.models_error = None;
            self.provider_panel_state.filtered.clear();
            self.provider_panel_state.selected_model = 0;
            self.provider_panel_state.model_scroll = 0;

            let all = providers::ProviderKind::all();
            let sel = self.provider_panel_state.selected_provider;
            let kind = all.get(sel).cloned().unwrap_or(self.config.kind.clone());
            let fetch_config = providers::ProviderConfig {
                kind,
                api_key: Some(self.provider_panel_state.key_buffer.clone())
                    .filter(|s| !s.is_empty())
                    .or_else(|| self.config.api_key.clone()),
                base_url: Some(self.provider_panel_state.url_buffer.clone())
                    .filter(|s| !s.is_empty()),
                model: self.config.model.clone(),
                max_tokens: self.config.max_tokens,
                temperature: self.config.temperature,
            };

            let (tx, rx) = tokio::sync::oneshot::channel();
            self.provider_panel_state.models_rx = Some(rx);

            tokio::spawn(async move {
                let result = providers::fetch_models(&fetch_config).await;
                match result {
                    Ok(list) => {
                        let names: Vec<String> = list.into_iter().map(|m| m.id).collect();
                        let _ = tx.send(Ok(names));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                    }
                }
            });
        }
    }

    /// Render the full UI using the dark neutral layout:
    /// top system bar, metrics panel, main process panel, command input, footer.
    fn render_widgets(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.size();

        // Modal overlays own the full screen — skip chat chrome so transcript /
        // metrics / tool boxes cannot bleed through a transparent panel.
        if self.show_provider_panel && !self.show_help {
            fill_area(frame, area, theme::SURFACE);
            omega_core::tui::provider_panel::render(
                area,
                frame.buffer_mut(),
                &self.provider_panel_state,
                &self.config,
            );
            return;
        }

        // ── Full-screen background ──────────────────────────────────────
        // Reset every cell so the canvas inherits the terminal background.
        fill_area(frame, area, theme::BG);

        // ── Layout: vertical stack ──────────────────────────────────────
        let top_bar_h = 1u16;
        let metrics_h = 3u16; // 3 lines: token gauge + model/context row + tool chips
        let footer_h = 1u16;
        // Input is a fixed 3-row strip: top line, content, bottom line.
        let editor_h = 3u16;

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(top_bar_h), // system bar
                Constraint::Length(metrics_h), // metrics panel
                Constraint::Min(4),            // process (transcript)
                Constraint::Length(editor_h),  // command input
                Constraint::Length(footer_h),  // footer
            ])
            .split(area);

        let top_area = vert[0];
        let metrics_area = vert[1];
        let process_area = vert[2];
        let editor_area = vert[3];
        let footer_area = vert[4];

        // ── Top system bar ──────────────────────────────────────────────
        render_top_bar(frame, top_area, self.config.model.as_str());

        // ── Metrics panel (glass-bordered) ───────────────────────────────
        render_metrics_panel(frame, metrics_area, &self.config, self.is_streaming);

        // ── Main process panel (glass-bordered) ──────────────────────────
        render_process_panel(frame, process_area, &mut self.transcript, self.show_help);

        // ── Command input (glass-bordered) ───────────────────────────────
        render_command_input(
            frame,
            editor_area,
            &self.editor,
            self.is_streaming,
            &self.status.spinner,
        );

        // ── Footer bar ──────────────────────────────────────────────────
        self.status.hint_text = Some("[CR] COMMIT | [^C] ABORT | ? help".into());
        let (tokens_in, tokens_out) = omega_core::commands::chat::session_token_counts();
        self.status.tokens_in = tokens_in;
        self.status.tokens_out = tokens_out;
        self.status.messages_count = self.session_messages;
        // Keep streaming estimates off the token display — only real provider
        // usage counts should appear as input/output.
        self.status.streaming_estimate = 0;
        frame.render_widget(&self.status, footer_area);

        // ── Overlays ────────────────────────────────────────────────────
        if self.show_help {
            omega_core::tui::help::render(area, frame.buffer_mut());
        }
    } // end render_widgets
} // end impl App

impl ratata::screen::Screen for App {
    fn render(&mut self, f: &mut ratatui::Frame) {
        self.render_widgets(f);
    }

    fn update(&mut self, message: Message) -> Option<Command> {
        match message {
            Message::Tick => {
                self.process_stream_events();
                self.poll_provider_models();
                self.tick_spinner();
                self.anim_tick = self.anim_tick.wrapping_add(1);
                self.last_tick = Instant::now();
                None
            }
            Message::Key(msg) => {
                let key = KeyEvent {
                    code: msg.code,
                    modifiers: msg.modifiers,
                    kind: KeyEventKind::Press,
                    state: msg.state,
                };
                self.handle_key(key);
                if self.should_quit {
                    Some(Command::Quit)
                } else {
                    None
                }
            }
            Message::Mouse(event) => {
                self.handle_mouse(event.kind);
                None
            }
            Message::Resize(_, _) => None,
            _ => None,
        }
    }
}

// ── Layout helpers ──────────────────────────────────────────────────────────

/// Fill an entire rect with a solid background color.
fn fill_area(frame: &mut ratatui::Frame, area: Rect, color: Color) {
    let style = Style::default().bg(color);
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = frame.buffer_mut().get_mut(x, y);
            cell.set_bg(color);
            cell.set_style(style);
        }
    }
}

/// Draw a horizontal rule line.

/// Draw a vertical line (for glass sidebar edge).

/// Render a glass-style bordered block helper: fills inner bg, draws box-drawing border.
fn render_glass_block(
    frame: &mut ratatui::Frame,
    area: Rect,
    inner_bg: Color,
    border_color: Color,
    title: &str,
    title_color: Color,
) -> Rect {
    let inner_pad = 1u16;
    let inner_area = if area.width > 2 && area.height > 2 {
        Rect::new(
            area.x + inner_pad,
            area.y + inner_pad,
            area.width.saturating_sub(inner_pad * 2),
            area.height.saturating_sub(inner_pad * 2),
        )
    } else {
        area
    };

    // Fill inner
    fill_area(frame, inner_area, inner_bg);

    // Draw border using box-drawing characters ┌─…─┐ │  │ └─…─┘
    let _border_style = Style::default().fg(border_color);
    let bw = area.width;
    let bh = area.height;

    // Top border ┌─ title ────────┐
    let title_chars: Vec<char> = title.chars().collect();
    let dash_count = bw
        .saturating_sub(title_chars.len() as u16)
        .saturating_sub(4);
    {
        let y = area.y;
        // ┌
        frame
            .buffer_mut()
            .get_mut(area.x, y)
            .set_symbol("┌")
            .set_fg(border_color);
        // title
        for (i, ch) in title_chars.iter().enumerate() {
            let cx = area.x + 2 + i as u16;
            if cx < area.x + bw {
                frame
                    .buffer_mut()
                    .get_mut(cx, y)
                    .set_char(*ch)
                    .set_fg(title_color);
            }
        }
        // ─ fill
        for i in 0..dash_count {
            let cx = area.x + 2 + title_chars.len() as u16 + i;
            if cx < area.x + bw - 1 {
                frame
                    .buffer_mut()
                    .get_mut(cx, y)
                    .set_symbol("─")
                    .set_fg(border_color);
            }
        }
        // ┐
        frame
            .buffer_mut()
            .get_mut(area.x + bw - 1, y)
            .set_symbol("┐")
            .set_fg(border_color);
    }

    // Sides │ … │
    for dy in 1..bh.saturating_sub(1) {
        let y = area.y + dy;
        frame
            .buffer_mut()
            .get_mut(area.x, y)
            .set_symbol("│")
            .set_fg(border_color);
        frame
            .buffer_mut()
            .get_mut(area.x + bw - 1, y)
            .set_symbol("│")
            .set_fg(border_color);
    }

    // Bottom border └──────────────┘
    {
        let y = area.y + bh - 1;
        frame
            .buffer_mut()
            .get_mut(area.x, y)
            .set_symbol("└")
            .set_fg(border_color);
        for i in 1..bw.saturating_sub(1) {
            let cx = area.x + i;
            frame
                .buffer_mut()
                .get_mut(cx, y)
                .set_symbol("─")
                .set_fg(border_color);
        }
        frame
            .buffer_mut()
            .get_mut(area.x + bw - 1, y)
            .set_symbol("┘")
            .set_fg(border_color);
    }

    inner_area
}

// ── Top system bar ──────────────────────────────────────────────────────────

fn render_top_bar(frame: &mut ratatui::Frame, area: Rect, model: &str) {
    if area.height < 1 || area.width < 30 {
        return;
    }

    let left_spans = vec![
        Span::styled(
            " OMEGA_AGENT ",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("v", theme::style_dim()),
        Span::styled(env!("CARGO_PKG_VERSION"), theme::style_dim()),
        Span::styled(" · ", theme::style_dim()),
        Span::styled("SYS_STATUS: ", theme::style_dim()),
        Span::styled("ONLINE", Style::default().fg(theme::PRIMARY)),
        Span::styled(" · ", theme::style_dim()),
        Span::styled("UPTIME: ", theme::style_dim()),
        Span::styled(model, Style::default().fg(theme::SECONDARY)),
    ];

    let right_hint = format!(" [F1] HELP [F2] LOGS [F3] NET [F10] EXIT ");
    let right_w = right_hint.len() as u16;
    let left_w: u16 = left_spans.iter().map(|s| s.width() as u16).sum();
    let fill = area.width.saturating_sub(left_w).saturating_sub(right_w);

    let mut out = vec![];
    out.extend(left_spans);
    if fill > 0 {
        out.push(Span::raw(" ".repeat(fill as usize)));
    }
    out.push(Span::styled(right_hint, theme::style_dim()));

    let para = Paragraph::new(Line::from(out));
    para.render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());

    // separator rule under top bar
    let _rule_y = area.y + area.height;
    // actually draw it at the bottom of this area
    if area.height > 1 {
        for x in area.x..area.x + area.width {
            let cell = frame.buffer_mut().get_mut(x, area.y + area.height - 1);
            cell.set_symbol("─");
            cell.set_fg(theme::OUTLINE);
        }
    }
}

// ── Metrics panel ────────────────────────────────────────────────────────────
// Glass-bordered box: real token usage, model, active tool chips.

fn render_metrics_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    config: &providers::ProviderConfig,
    _is_streaming: bool,
) {
    if area.height < 3 || area.width < 40 {
        return;
    }

    let inner = render_glass_block(
        frame,
        area,
        theme::SURFACE_LOW,
        theme::OUTLINE,
        " SYSTEM METRICS & ACTIVE TOOLS ",
        theme::PRIMARY,
    );

    // Row 0: real session input/output usage (no fake context-window gauge)
    let gauge_y = inner.y;
    let (tokens_in, tokens_out) = omega_core::commands::chat::session_token_counts();
    let usage = omega_core::tui::status::StatusState::format_token_usage(tokens_in, tokens_out);
    let gauge_spans = vec![
        Span::styled("TOKENS ", theme::style_dim()),
        Span::styled(usage, Style::default().fg(theme::PRIMARY)),
    ];
    Paragraph::new(Line::from(gauge_spans)).render(
        Rect::new(inner.x + 1, gauge_y, inner.width.saturating_sub(2), 1),
        frame.buffer_mut(),
    );

    // Row 1: model only
    let row1_y = gauge_y + 1;
    let model_str = config.model.clone();
    let model_spans = vec![
        Span::styled("MODEL ", theme::style_dim()),
        Span::styled(model_str, Style::default().fg(theme::SECONDARY)),
    ];
    Paragraph::new(Line::from(model_spans)).render(
        Rect::new(inner.x + 1, row1_y, inner.width.saturating_sub(2), 1),
        frame.buffer_mut(),
    );

    // Row 2: active tool chips
    if inner.height > 2 {
        let chips_y = row1_y + 1;
        let chips = vec![
            ("[ BROWSER ]", theme::TOOL_BROWSER),
            ("[ SHELL ]", theme::TOOL_SHELL),
            ("[ FILE_SYS ]", theme::TOOL_FILE_SYS),
            ("[ SEARCH ]", theme::TOOL_SEARCH),
        ];
        let mut chip_spans: Vec<Span> = Vec::new();
        for (i, (label, col)) in chips.iter().enumerate() {
            if i > 0 {
                chip_spans.push(Span::raw(" "));
            }
            chip_spans.push(Span::styled(*label, Style::default().fg(*col)));
        }
        Paragraph::new(Line::from(chip_spans))
            .alignment(Alignment::Right)
            .render(
                Rect::new(inner.x + 1, chips_y, inner.width.saturating_sub(2), 1),
                frame.buffer_mut(),
            );
    }
}

// ── Main process panel ───────────────────────────────────────────────────────

fn render_process_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    transcript: &mut Transcript,
    _show_help: bool,
) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    // No frame around the transcript — just the content on the terminal canvas.
    fill_area(frame, area, theme::BG);
    transcript.render(frame, area);
}

// ── Command input ────────────────────────────────────────────────────────────

fn render_command_input(
    frame: &mut ratatui::Frame,
    area: Rect,
    editor: &EditorState,
    is_streaming: bool,
    spinner: &OmegaSpinner,
) {
    if area.height < 3 || area.width < 4 {
        return;
    }

    // Inherit the terminal background — no dark recessed fill.
    fill_area(frame, area, theme::BG);

    let line_style = Style::default().fg(theme::OUTLINE);
    let top_y = area.y;
    let content_y = area.y + 1;
    let bottom_y = area.y + 2;

    // Top and bottom rules only — no side borders, no labels.
    for x in area.x..area.x + area.width {
        frame
            .buffer_mut()
            .get_mut(x, top_y)
            .set_symbol("─")
            .set_style(line_style);
        frame
            .buffer_mut()
            .get_mut(x, bottom_y)
            .set_symbol("─")
            .set_style(line_style);
    }

    let content_x = area.x + 1;
    let content_w = area.width.saturating_sub(2);
    if content_w == 0 {
        return;
    }

    if is_streaming && editor.buffer.is_empty() {
        let activity = format!("{} {}", spinner.current_glyph(), spinner.current_phrase());
        Paragraph::new(Line::from(Span::styled(activity, spinner.glyph_style()))).render(
            Rect::new(content_x, content_y, content_w, 1),
            frame.buffer_mut(),
        );
    } else if !editor.buffer.is_empty() {
        // Single-line display: keep the tail of multi-line text visible.
        let display = editor.buffer.lines().last().unwrap_or("");
        let shown = if display.chars().count() > content_w as usize {
            let skip = display.chars().count().saturating_sub(content_w as usize);
            display.chars().skip(skip).collect::<String>()
        } else {
            display.to_string()
        };
        Paragraph::new(Line::from(Span::styled(
            shown,
            Style::default().fg(theme::FG),
        )))
        .render(
            Rect::new(content_x, content_y, content_w, 1),
            frame.buffer_mut(),
        );
    }
    // Empty idle: just the two horizontal lines.
}

// ── Config helpers ──────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct CliConfig {
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
}

fn config_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("com", "omega", "omega-agent")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn load_config() -> CliConfig {
    let path = config_dir().join("config.json");
    let _ = std::fs::create_dir_all(config_dir());
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(CliConfig {
            provider: None,
            model: None,
            base_url: None,
        })
}

fn save_config(config: &providers::ProviderConfig) {
    let cli = CliConfig {
        provider: Some(config.kind.to_string()),
        model: Some(config.model.clone()),
        base_url: config.base_url.clone(),
    };
    let path = config_dir().join("config.json");
    if let Ok(json) = serde_json::to_string_pretty(&cli) {
        let _ = std::fs::write(&path, json);
    }
}

/// Persist API key to `~/.config/omega-agent/.env` (plain key body).
/// Empty / None removes the file so load falls back to env-only.
fn save_api_key(api_key: Option<&str>) {
    let path = config_dir().join(".env");
    let _ = std::fs::create_dir_all(config_dir());
    match api_key.map(str::trim).filter(|s| !s.is_empty()) {
        Some(key) => {
            let _ = std::fs::write(&path, format!("{key}\n"));
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn load_provider_config(
    override_provider: Option<String>,
    override_model: Option<String>,
    override_base_url: Option<String>,
) -> providers::ProviderConfig {
    let mut cli_cfg = load_config();

    // Apply CLI overrides on top of config file
    if let Some(p) = override_provider {
        cli_cfg.provider = Some(p);
    }
    if let Some(m) = override_model {
        cli_cfg.model = Some(m);
    }
    if let Some(b) = override_base_url {
        cli_cfg.base_url = Some(b);
    }

    // Resolve provider kind
    let kind = cli_cfg
        .provider
        .as_deref()
        .map(providers::ProviderKind::from_str)
        .unwrap_or_else(|| {
            // Auto-detect: if API key is set, use OpenAI; otherwise Local (Ollama)
            let has_api_key =
                std::env::var("OMEGA_API_KEY").is_ok() || config_dir().join(".env").exists();
            if has_api_key {
                providers::ProviderKind::OpenAI
            } else {
                providers::ProviderKind::Local
            }
        });

    // Resolve model
    let model = cli_cfg
        .model
        .or_else(|| std::env::var("OMEGA_MODEL").ok())
        .unwrap_or_else(|| match kind {
            providers::ProviderKind::OpenAI => "gpt-4o-mini".into(),
            providers::ProviderKind::Anthropic => "claude-sonnet-4-20250514".into(),
            providers::ProviderKind::Google => "gemini-2.0-flash".into(),
            providers::ProviderKind::Local => "llama3.1:8b".into(),
            providers::ProviderKind::Custom => "custom-model".into(),
            _ => "gpt-4o-mini".into(),
        });

    // Resolve base URL
    let base_url = cli_cfg
        .base_url
        .or_else(|| std::env::var("OMEGA_BASE_URL").ok());

    // Resolve API key
    let api_key = std::env::var("OMEGA_API_KEY").ok().or_else(|| {
        let p = config_dir().join(".env");
        std::fs::read_to_string(&p)
            .ok()
            .map(|s| s.trim().to_string())
    });

    providers::ProviderConfig {
        kind,
        api_key,
        base_url,
        model,
        max_tokens: 4096,
        temperature: 0.7,
    }
}

// ── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "omega",
    version,
    about = "Omega Agent TUI — AI coding assistant"
)]
struct Cli {
    #[arg(
        short = 'p',
        long,
        help = "Provider (openai, anthropic, google, local, ollama, groq, etc.)"
    )]
    provider: Option<String>,

    #[arg(
        short = 'm',
        long,
        help = "Model name (e.g. gpt-4o-mini, llama3.1:8b, claude-sonnet-4)"
    )]
    model: Option<String>,

    #[arg(short = 'b', long, help = "Base URL for the provider API")]
    base_url: Option<String>,

    /// Resume a specific conversation session by id
    #[arg(long = "session", value_name = "ID", help = "Resume session <id>")]
    session: Option<String>,

    /// Force a brand-new conversation session (ignore last-session marker)
    #[arg(
        long = "new-session",
        help = "Start a new session instead of resuming the last one"
    )]
    new_session: bool,
}

// entry point
fn main() -> Result<()> {
    // Full-screen TUI owns stdout/stderr via the alternate screen. Log output at
    // `info` would write behind Ratatui and corrupt the layout, so default to
    // `error` unless the caller explicitly exports RUST_LOG.
    let default_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "error".to_string());
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&default_filter))
        .init();
    let cli = Cli::parse();
    let config = load_provider_config(cli.provider, cli.model, cli.base_url);

    let model = config.model.clone();
    let kind = config.kind.to_string();

    let (session_store, session_load) = SessionStore::resolve(cli.session, cli.new_session)
        .map_err(|e| anyhow::anyhow!("session: {e}"))?;
    let session_id = session_store.id.clone();

    let app = App::new(config, session_store, session_load);

    // Create a tokio runtime for background streaming tasks
    let rt = tokio::runtime::Runtime::new()?;
    let _guard = rt.enter();

    let backend = CrosstermBackend::new(std::io::stdout());

    Application::new()
        .tick_rate(Duration::from_millis(80))
        .screen(app)
        .on_startup(|| {
            Command::Batch(vec![
                Command::EnableRawMode,
                Command::crossterm(crossterm::terminal::EnterAlternateScreen),
            ])
        })
        .on_shutdown(|| {
            Command::Batch(vec![
                Command::crossterm(crossterm::terminal::LeaveAlternateScreen),
                Command::DisableRawMode,
            ])
        })
        .build(std::io::stdout(), backend)?
        .run::<App>()?;

    // Session summary (tokens from global statics, config captured before run)
    let (tokens_in, tokens_out) = omega_core::commands::chat::session_token_counts();
    println!();
    println!("Ω Omega Agent — session summary");
    println!("  Model:     {}", model);
    println!("  Provider:  {}", kind);
    println!("  Session:   {}", session_id);
    println!("  Tokens:    {} in / {} out", tokens_in, tokens_out);
    println!();

    Ok(())
}
