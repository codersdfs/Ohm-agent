// ── Omega Agent TUI ───────────────────────────────────────────────────────────
// Ratatui-based full-screen terminal UI with minimal, conversation-first design.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Terminal;

use omega_core::tui::editor::{EditorMode, EditorState};
use omega_core::tui::header::HeaderState;
use omega_core::tui::status::StatusState;
use omega_core::tui::omega_mark::{self, AgentState, AnimationPhase};
use omega_core::tui::theme;
use omega_core::tui::transcript::{self, TranscriptEntry, ScrollState};
use omega_core::{commands, AppState, ChatEmitter, default_db_path};

// ── Event types for streaming ────────────────────────────────────────────────

/// Events sent from the streaming task to the UI event loop.
#[derive(Debug, Clone)]
enum UiStreamEvent {
    Token(String),
    Done { full: String, tokens_in: u32, tokens_out: u32 },
    Error(String),
}

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
    fn emit_done(&self, _full: &str) -> std::result::Result<(), String> {
        // Done is sent after the full response is collected in the task
        Ok(())
    }
    fn emit_error(&self, error: &str) -> std::result::Result<(), String> {
        let _ = self.tx.send(UiStreamEvent::Error(error.to_string()));
        Ok(())
    }
}

// ── App state ────────────────────────────────────────────────────────────────

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

struct App {
    // Core state
    state: Arc<AppState>,
    config: providers::ProviderConfig,

    // UI state
    header: HeaderState,
    entries: Vec<TranscriptEntry>,
    scroll: ScrollState,
    editor: EditorState,
    status: StatusState,

    // LLM conversation history
    messages: Vec<providers::ChatMessage>,

    // Streaming
    is_streaming: bool,
    stream_event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<UiStreamEvent>>,
    streaming_fragment: String,
    cancel_flag: Arc<AtomicBool>,

    // Spinner / animation
    spinner_idx: usize,
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

    // Should quit
    should_quit: bool,
}

impl App {
    fn new(config: providers::ProviderConfig) -> Self {
        let state = Arc::new(AppState::new_with_provider_config(&default_db_path(), config.clone()));
        let model = config.model.clone();
        let kind_str = format!("{}", config.kind);
        let header = HeaderState::new(model, kind_str);
        let editor = EditorState::new();
        let status = StatusState::new();

        let cfg_for_panel = config.clone();
        let mut app = Self {
            state,
            config,
            header,
            entries: Vec::new(),
            scroll: ScrollState::default(),
            editor,
            status,
            messages: Vec::new(),
            is_streaming: false,
            stream_event_rx: None,
            streaming_fragment: String::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            spinner_idx: 0,
            anim_tick: 0,
            last_tick: Instant::now(),

            history: Vec::new(),
            history_index: None,

            session_tokens_in: 0,
            session_tokens_out: 0,
            session_messages: 0,

            show_help: false,
            show_provider_panel: false,
            provider_panel_state: omega_core::tui::provider_panel::ProviderPanelState::from_config(&cfg_for_panel),

            should_quit: false,
        };

        // Welcome notice
        app.entries.push(TranscriptEntry::Notice {
            text: format!("Ω v{} — {} ({}). Type a message to start.", env!("CARGO_PKG_VERSION"), app.config.model, app.config.kind),
            is_error: false,
        });

        // Show setup hint when API key is needed for cloud providers
        let is_local = matches!(app.config.kind, providers::ProviderKind::Local);
        if app.config.api_key.is_none() && !is_local {
            app.entries.push(TranscriptEntry::Notice {
                text: "No API key found. Set OMEGA_API_KEY or run: omega -p local".into(),
                is_error: true,
            });
        }

        // Load MCP skills
        let (mcp_loaded, mcp_errors) = commands::mcp::load_skills();
        if mcp_loaded > 0 {
            app.entries.push(TranscriptEntry::Notice {
                text: format!("MCP: {} skills loaded", mcp_loaded),
                is_error: false,
            });
        }
        for err in &mcp_errors {
            app.entries.push(TranscriptEntry::Notice {
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
            let action = omega_core::tui::provider_panel::handle_key(
                &mut self.provider_panel_state, key,
            );
            match action {
                omega_core::tui::provider_panel::PanelAction::Apply => {
                    let new_config = self.provider_panel_state.to_config(&self.config);
                    self.config = new_config.clone();
                    self.header = omega_core::tui::header::HeaderState::new(
                        self.config.model.clone(),
                        format!("{}", self.config.kind),
                    );
                    save_config(&self.config);
                    self.entries.push(TranscriptEntry::Notice {
                        text: format!("Provider set to {} ({})", self.config.model, self.config.kind),
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

        // Toggle help overlay
        if key.code == KeyCode::Char('?') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            self.show_help = !self.show_help;
            self.editor.suggestions.clear();
            return;
        }

        // Editor input
        match key.code {
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.editor.newline();
                } else {
                    self.submit_message();
                }
            }
            KeyCode::Char(c) => {
                self.editor.insert_char(c);
            }
            KeyCode::Backspace => {
                self.editor.backspace();
            }
            KeyCode::Delete => {
                self.editor.delete();
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.editor.cursor_home();
                } else {
                    self.editor.cursor_left();
                }
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.editor.cursor_end();
                } else {
                    self.editor.cursor_right();
                }
            }
            KeyCode::Home => self.editor.cursor_home(),
            KeyCode::End => self.editor.cursor_end(),
            KeyCode::Tab => {
                // Cycle through suggestions
                if !self.editor.suggestions.is_empty() {
                    self.editor.selected_suggestion = (self.editor.selected_suggestion + 1)
                        % self.editor.suggestions.len();
                }
            }
            KeyCode::Up => {
                self.recall_history_up();
                transcript::scroll_up(&mut self.scroll, 3);
            }
            KeyCode::Down => {
                self.recall_history_down();
                transcript::scroll_down(&mut self.scroll, self.entries.len(), 3);
            }
            KeyCode::PageUp => {
                transcript::scroll_up(&mut self.scroll, 10);
            }
            KeyCode::PageDown => {
                transcript::scroll_down(&mut self.scroll, self.entries.len(), 10);
            }
            _ => {}
        }
    }

    /// Cancel the current streaming request.
    fn cancel_streaming(&mut self) {
        self.cancel_flag.store(true, Ordering::SeqCst);

        // Drop the receiver so the streaming task's tx.send() fails
        self.stream_event_rx = None;

        self.is_streaming = false;
        self.editor.state = EditorMode::Idle;
        self.status.spinner = None;
        self.status.action_text = String::new();

        // Mark the pending assistant entry as stopped
        for entry in self.entries.iter_mut().rev() {
            if let TranscriptEntry::Assistant { ref mut is_streaming, .. } = entry {
                *is_streaming = false;
                break;
            }
        }

        // Show cancel notice
        self.entries.push(TranscriptEntry::Notice {
            text: "Stream cancelled".into(),
            is_error: false,
        });

        self.scroll.auto_scroll = true;
        self.streaming_fragment.clear();
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
                transcript::scroll_down(&mut self.scroll, self.entries.len(), 3);
            }
            MouseEventKind::ScrollUp => {
                transcript::scroll_up(&mut self.scroll, 3);
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
        self.entries.push(TranscriptEntry::User {
            content: content.clone(),
        });

        // Start streaming
        self.start_streaming(content);
    }

    /// Handle a slash command.
    fn handle_slash_command(&mut self, cmd: &str) {
        match cmd.to_lowercase().trim() {
            "/help" | "/?" | "/h" => {
                self.entries.push(TranscriptEntry::Notice {
                    text: "Commands: /help, /clear, /tools, /model <name>, /exit, /cost".into(),
                    is_error: false,
                });
            }
            "/clear" | "/cls" => {
                self.entries.clear();
                self.messages.clear();
                self.editor.buffer.clear();
            }
            "/tools" => {
                match commands::tools::list_tools() {
                    Ok(tools) => {
                        let list = tools.join(", ");
                        self.entries.push(TranscriptEntry::Notice {
                            text: format!("Available tools: {}", list),
                            is_error: false,
                        });
                    }
                    Err(e) => {
                        self.entries.push(TranscriptEntry::Notice {
                            text: format!("Error listing tools: {}", e),
                            is_error: true,
                        });
                    }
                }
            }
            "/model" | "/provider" | "/providers" | "/p" => {
                if self.is_streaming {
                    self.entries.push(TranscriptEntry::Notice {
                        text: "Can't open provider panel while streaming.".into(),
                        is_error: true,
                    });
                } else {
                    self.provider_panel_state = omega_core::tui::provider_panel::ProviderPanelState::from_config(&self.config);
                    self.show_provider_panel = true;
                }
            }
            "/cost" => {
                self.entries.push(TranscriptEntry::Notice {
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
                self.entries.push(TranscriptEntry::Notice {
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
        self.status.action_text = "thinking…".into();
        self.status.spinner = Some("⠋".into());

        // Create channel
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.stream_event_rx = Some(rx);

        // Add a placeholder assistant entry
        self.entries.push(TranscriptEntry::Assistant {
            content: String::new(),
            rendered: None,
            is_streaming: true,
        });

        // Get references for the async task
        let state = self.state.clone();
        let config = self.config.clone();
        let system_prompt = commands::tools::default_system_prompt();
        let permission_mode = "off".to_string();

        // Shared message list for the task to modify
        let messages = Arc::new(tokio::sync::Mutex::new(self.messages.clone()));
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
            };

            let result = {
                let mut msgs = messages.lock().await;
                commands::chat::stream_message_with_history(
                    &state,
                    request,
                    &emitter,
                    &mut msgs,
                )
                .await
            };

            // Check cancellation (don't send events if cancelled)
            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }

            // Send done event with result
            match result {
                Ok(full) => {
                    let _ = event_tx.send(UiStreamEvent::Done {
                        full,
                        tokens_in: 0,
                        tokens_out: 0,
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
        let Some(rx) = &mut self.stream_event_rx else {
            return;
        };

        let mut done = false;

        while let Ok(event) = rx.try_recv() {
            match event {
                UiStreamEvent::Token(t) => {
                    self.streaming_fragment.push_str(&t);
                    self.editor.state = EditorMode::Streaming;
                    self.status.action_text = "streaming…".into();
                    self.status.spinner = Some("⠙".into());

                    // Update the last assistant entry with content
                    if let Some(last) = self.entries.last_mut() {
                        if let TranscriptEntry::Assistant { content, rendered, is_streaming } = last {
                            content.push_str(&t);
                            *rendered = None; // invalidate cache
                            *is_streaming = true;
                        }
                    }
                }

                UiStreamEvent::Done { full: _full, tokens_in, tokens_out } => {
                    self.session_tokens_in += tokens_in as u64;
                    self.session_tokens_out += tokens_out as u64;
                    self.session_messages += 1;

                    // Update assistant entry: mark not streaming
                    for entry in self.entries.iter_mut().rev() {
                        if let TranscriptEntry::Assistant { ref mut is_streaming, ref mut rendered, .. } = entry {
                            *is_streaming = false;
                            *rendered = None;
                            break;
                        }
                    }

                    done = true;
                }
                UiStreamEvent::Error(e) => {
                    self.entries.push(TranscriptEntry::Notice {
                        text: e,
                        is_error: true,
                    });
                    done = true;

                    // Remove the failed assistant entry if it has no content
                    if let Some(last) = self.entries.last() {
                        if let TranscriptEntry::Assistant { content, .. } = last {
                            if content.is_empty() {
                                self.entries.pop();
                            }
                        }
                    }
                }
            }
        }

        if done {
            self.is_streaming = false;
            self.editor.state = EditorMode::Idle;
            self.status.spinner = None;
            self.status.action_text = String::new();
            self.stream_event_rx = None;
            self.streaming_fragment.clear();
            self.scroll.auto_scroll = true; // jump to bottom
        }
    }

    /// Advance the spinner frame.
    fn tick_spinner(&mut self) {
        self.spinner_idx = (self.spinner_idx + 1) % SPINNER_FRAMES.len();
        if let Some(spinner) = &mut self.status.spinner {
            *spinner = SPINNER_FRAMES[self.spinner_idx].to_string();
        }
    }

    /// Render the full UI.
    fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();

        // ── Layout ──────────────────────────────────────────────────────────
        let header_height = 2u16; // 1 line + 1 rule
        let status_height = 1u16;
        let editor_min_height = 3u16; // border + 1 line + padding
        let editor_lines = self.editor.buffer.lines().count().max(1).min(8) as u16;
        let editor_height = editor_min_height + editor_lines.saturating_sub(1);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height),
                Constraint::Min(1),
                Constraint::Length(editor_height),
                Constraint::Length(status_height),
            ])
            .split(area);

        let header_area = chunks[0];
        let transcript_area = chunks[1];
        let editor_area = chunks[2];
        let status_area = chunks[3];

        // ── Compute context fill ───────────────────────────────────────────
        let (tok_in, tok_out) = omega_core::commands::chat::session_token_counts();
        let ctx_window = self.config.kind.context_window();
        let total_tokens = tok_in + tok_out;
        self.header.ctx_pct = if ctx_window > 0 && total_tokens > 0 {
            Some(((total_tokens * 100) / ctx_window) as u8)
        } else {
            None
        };

        // ── Render widgets ──────────────────────────────────────────────────
        omega_core::tui::header::render(header_area, frame.buffer_mut(), &self.header);

        // Render transcript first (so startup messages show at top)
        let is_scrolled_up = self.scroll.offset > 0;
        transcript::render(transcript_area, frame.buffer_mut(), &mut self.entries, &mut self.scroll);

        // Big Omega mark OVERLAY on empty transcript area (rendered after transcript
        // so it overwrites the empty space below any startup notices)
        let has_conversation = self.entries.iter().any(|e| matches!(e, TranscriptEntry::User { .. } | TranscriptEntry::Assistant { .. }));
        if !has_conversation && !self.show_help {
            let agent_state = if self.is_streaming {
                AgentState::Streaming
            } else if self.editor.state == omega_core::tui::editor::EditorMode::Thinking {
                AgentState::Thinking
            } else {
                AgentState::Idle
            };
            let phase = AnimationPhase {
                tick: self.anim_tick,
                agent: agent_state,
            };
            omega_mark::render(transcript_area, frame.buffer_mut(), &phase);
        }

        // Draw a scroll-up indicator as an overlay at the bottom of transcript
        if is_scrolled_up && !self.show_help && !self.is_streaming {
            let indicator_text = format!(" ↑ {} more ", self.scroll.offset);
            let indicator = Paragraph::new(Line::from(Span::styled(
                indicator_text,
                theme::style_dim(),
            )))
            .style(Style::default())
            .alignment(Alignment::Right);
            let indicator_area = Rect::new(
                transcript_area.right().saturating_sub(16),
                transcript_area.bottom().saturating_sub(1),
                16,
                1,
            );
            indicator.render(indicator_area, frame.buffer_mut());
        }

        // Editor — show suggestions if typing a slash command
        if self.editor.buffer.starts_with('/') && !self.is_streaming {
            let suggestions = self.get_slash_suggestions();
            omega_core::tui::editor::render_suggestions(
                editor_area,
                frame.buffer_mut(),
                &suggestions,
                self.editor.selected_suggestion,
            );
        }

        omega_core::tui::editor::render(editor_area, frame.buffer_mut(), &self.editor);

        // Set keybinding hints when idle
        if !self.is_streaming && !self.show_help {
            self.status.hint_text = Some("Ctrl+Q quit · ? help".into());
        } else if !self.show_help {
            self.status.hint_text = None;
        }

        // Status — read tokens from globals, estimate streaming live
        let (tokens_in, tokens_out) = omega_core::commands::chat::session_token_counts();
        self.status.tokens_in = tokens_in;
        self.status.tokens_out = tokens_out;
        self.status.messages_count = self.session_messages;
        if self.is_streaming {
            self.status.streaming_estimate = (self.streaming_fragment.len() / 4) as u64;
        } else {
            self.status.streaming_estimate = 0;
        }
        omega_core::tui::status::render(status_area, frame.buffer_mut(), &self.status);

        // Provider panel overlay (drawn on top, before help)
        if self.show_provider_panel && !self.show_help {
            omega_core::tui::provider_panel::render(
                area,
                frame.buffer_mut(),
                &self.provider_panel_state,
                &self.config,
            );
        }

        // Help overlay (drawn last, on top of everything)
        if self.show_help {
            omega_core::tui::help::render(area, frame.buffer_mut());
        }
    }

    fn get_slash_suggestions(&self) -> Vec<String> {
        let input = self.editor.buffer.to_lowercase();
        let all_commands = vec![
            "/help".to_string(),
            "/clear".to_string(),
            "/tools".to_string(),
            "/model".to_string(),
            "/provider".to_string(),
            "/cost".to_string(),
            "/exit".to_string(),
        ];
        if input == "/" {
            all_commands
        } else {
            all_commands.into_iter().filter(|c| c.starts_with(&input)).collect()
        }
    }
}



// ── Config helpers (from old main.rs, kept minimal) ──────────────────────────

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
        .unwrap_or(CliConfig { provider: None, model: None, base_url: None })
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
            let has_api_key = std::env::var("OMEGA_API_KEY").is_ok()
                || config_dir().join(".env").exists();
            if has_api_key {
                providers::ProviderKind::OpenAI
            } else {
                providers::ProviderKind::Local
            }
        });

    // Resolve model
    let model = cli_cfg.model
        .or_else(|| std::env::var("OMEGA_MODEL").ok())
        .unwrap_or_else(|| match kind {
            providers::ProviderKind::OpenAI => "gpt-4o-mini".into(),
            providers::ProviderKind::Anthropic => "claude-sonnet-4-20250514".into(),
            providers::ProviderKind::Google => "gemini-2.0-flash".into(),
            providers::ProviderKind::Local => "llama3.1:8b".into(),
            _ => "gpt-4o-mini".into(),
        });

    // Resolve base URL
    let base_url = cli_cfg.base_url
        .or_else(|| std::env::var("OMEGA_BASE_URL").ok());

    // Resolve API key
    let api_key = std::env::var("OMEGA_API_KEY").ok().or_else(|| {
        let p = config_dir().join(".env");
        std::fs::read_to_string(&p).ok().map(|s| s.trim().to_string())
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
#[command(name = "omega", version, about = "Omega Agent TUI — AI coding assistant")]
struct Cli {
    #[arg(short = 'p', long, help = "Provider (openai, anthropic, google, local, ollama, groq, etc.)")]
    provider: Option<String>,

    #[arg(short = 'm', long, help = "Model name (e.g. gpt-4o-mini, llama3.1:8b, claude-sonnet-4)")]
    model: Option<String>,

    #[arg(short = 'b', long, help = "Base URL for the provider API")]
    base_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let config = load_provider_config(cli.provider, cli.model, cli.base_url);

    // ── Setup ratatui ──────────────────────────────────────────────────────
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(config);
    let tick_rate = Duration::from_millis(80);

    // ── Event loop ─────────────────────────────────────────────────────────
    loop {
        // Process streaming events without blocking
        app.process_stream_events();

        // ── Model fetch for provider panel ────────────────────────────────
        // Check if fetch result is ready
        if let Some(rx) = &mut app.provider_panel_state.models_rx {
            match rx.try_recv() {
                Ok(Ok(models)) => {
                    app.provider_panel_state.models = models;
                    app.provider_panel_state.models_loading = false;
                    app.provider_panel_state.models_rx = None;
                }
                Ok(Err(e)) => {
                    app.provider_panel_state.models.clear();
                    app.provider_panel_state.models_error = Some(e);
                    app.provider_panel_state.models_loading = false;
                    app.provider_panel_state.models_rx = None;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
                Err(_) => {
                    app.provider_panel_state.models_loading = false;
                    app.provider_panel_state.models_rx = None;
                }
            }
        }

        // Trigger new fetch if needed
        if app.show_provider_panel
            && app.provider_panel_state.needs_fetch
            && app.provider_panel_state.models_rx.is_none()
        {
            app.provider_panel_state.needs_fetch = false;
            app.provider_panel_state.models_loading = true;
            app.provider_panel_state.models.clear();
            app.provider_panel_state.models_error = None;
            app.provider_panel_state.show_dropdown = false;

            let all = providers::ProviderKind::all();
            let sel = app.provider_panel_state.selected_provider;
            let kind = all.get(sel).cloned().unwrap_or(app.config.kind.clone());
            let fetch_config = providers::ProviderConfig {
                kind,
                api_key: app.config.api_key.clone(),
                base_url: Some(app.provider_panel_state.url_buffer.clone())
                    .filter(|s| !s.is_empty()),
                model: app.config.model.clone(),
                max_tokens: app.config.max_tokens,
                temperature: app.config.temperature,
            };

            let (tx, rx) = tokio::sync::oneshot::channel();
            app.provider_panel_state.models_rx = Some(rx);

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

        // Draw
        terminal.draw(|frame| app.render(frame))?;

        // Tick-based spinner advancement + animation tick
        let now = Instant::now();
        if now.duration_since(app.last_tick) >= tick_rate {
            if app.is_streaming {
                app.tick_spinner();
            }
            app.anim_tick = app.anim_tick.wrapping_add(1);
            app.last_tick = now;
        }

        // Check for quit
        if app.should_quit {
            break;
        }

        // Poll for input with a short timeout (so streaming events get processed)
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse.kind),
                Event::Resize(_w, _h) => {
                    // Terminal resize is handled automatically by ratatui
                }
                _ => {}
            }
        }
    }

    // ── Cleanup ────────────────────────────────────────────────────────────
    terminal::disable_raw_mode()?;
    let mut stdout = std::io::stdout();
    stdout.execute(LeaveAlternateScreen)?;

    // Print a summary on exit
    println!();
    println!("Ω Omega Agent — session summary");
    println!("  Model:     {}", app.config.model);
    println!("  Provider:  {}", app.config.kind);
    println!("  Messages:  {}", app.session_messages);
    println!("  Tokens:    {} in / {} out", app.session_tokens_in, app.session_tokens_out);
    println!();

    Ok(())
}
