//! TUI App — state + lifecycle (P5 split from main.rs).
//!
//! Submodules:
//! - [`input`]: key/mouse handling, history, palette shortcuts
//! - [`slash`]: slash-command dispatch
//! - [`stream`]: streaming task spawn + event processing

pub mod input;
pub mod slash;
pub mod stream;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use crossterm::event::{KeyEvent, KeyEventKind};
use ratata::prelude::*;

use omega_core::session::{SessionLoad, SessionStore};
use omega_core::tui::component::UiStreamEvent;
use omega_core::tui::editor::EditorState;
use omega_core::tui::layout::{render_full_layout, LayoutChrome};
use omega_core::tui::status::StatusState;
use omega_core::tui::transcript::Transcript;
use omega_core::{commands, default_db_path, AppState, ChatEmitter};

/// ChatEmitter impl that sends events through an mpsc channel.
pub struct ChannelEmitter {
    pub tx: tokio::sync::mpsc::UnboundedSender<UiStreamEvent>,
}

impl ChannelEmitter {
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<UiStreamEvent>) -> Self {
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

pub struct App {
    // Core state
    pub state: Arc<AppState>,
    pub config: providers::ProviderConfig,

    // UI state
    pub transcript: Transcript,
    pub editor: EditorState,
    pub status: StatusState,

    // Streaming
    pub is_streaming: bool,
    pub cancel_flag: Arc<AtomicBool>,

    // Animation tick
    pub anim_tick: u64,
    pub last_tick: Instant,

    // Input history
    pub history: Vec<String>,
    pub history_index: Option<usize>,

    // Cost tracking
    pub session_tokens_in: u64,
    pub session_tokens_out: u64,
    pub session_messages: u64,

    // Help overlay
    pub show_help: bool,

    // Provider panel
    pub show_provider_panel: bool,
    pub provider_panel_state: omega_core::tui::provider_panel::ProviderPanelState,

    // Command palette
    pub show_command_palette: bool,
    pub command_palette: omega_core::tui::command_palette::CommandPaletteState,

    // Sidebar visibility
    pub show_sidebar: bool,

    // Global write/edit preview expansion
    pub tool_output_expanded: bool,

    // Should quit
    pub should_quit: bool,
    /// Names of tools currently executing, for live header chips.
    pub running_tools: Vec<String>,
}

impl App {
    pub fn new(
        config: providers::ProviderConfig,
        session: SessionStore,
        load: SessionLoad,
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
        // Loader is config-driven: the future settings panel edits
        // `config.json`'s "loader" block; unknown styles fall back safely.
        let mut status = StatusState::new();
        status.loader = omega_core::tui::loader::LoaderRegistry::from_config(
            &crate::config_loader::load_loader_config(),
        );
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
            running_tools: Vec::new(),
        };

        // Welcome notice
        app.transcript.add_notice(
            format!(
                "Ω v{} — {} ({}). Type a message to start.",
                env!("CARGO_PKG_VERSION"),
                app.config.model,
                app.config.kind
            ),
            false,
        );

        // Session resume / new notice
        if resumed {
            app.transcript.add_notice(
                format!(
                    "Resumed session {} ({} messages)",
                    &session_id[..session_id.len().min(8)],
                    msg_count
                ),
                false,
            );
            app.transcript.load_from_session(load.messages);
        } else {
            app.transcript.add_notice(
                format!("New session {}", &session_id[..session_id.len().min(8)]),
                false,
            );
        }
        for w in warnings {
            app.transcript
                .add_notice(format!("Session load: {w}"), true);
        }

        // Show setup hint when API key is needed for cloud providers
        let is_local = matches!(app.config.kind, providers::ProviderKind::Local);
        if app.config.api_key.is_none() && !is_local {
            app.transcript.add_notice(
                "No API key found. Set OMEGA_API_KEY or run: omega -p local".into(),
                true,
            );
        }

        // Load MCP skills
        let (mcp_loaded, mcp_errors) = commands::mcp::load_skills();
        if mcp_loaded > 0 {
            app.transcript
                .add_notice(format!("MCP: {} skills loaded", mcp_loaded), false);
        }
        for err in &mcp_errors {
            app.transcript.add_notice(format!("MCP: {}", err), true);
        }

        // Load agent skills from ~/.agents/skill/
        let agent_skill_count = commands::agent_skills::init();
        if agent_skill_count > 0 {
            app.transcript.add_notice(
                format!("Skills: {} agent skill(s) available (use /skill to list)", agent_skill_count),
                false,
            );
        }

        app
    }

    /// Render the full UI using the centralized layout engine.
    fn render_widgets(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.size();
        let is_command_mode = self.show_command_palette;

        let mut chrome = LayoutChrome {
            model_name: self.config.model.as_str(),
            config: &self.config,
            transcript: &mut self.transcript,
            status: &mut self.status,
            editor: &self.editor,
            show_help: self.show_help,
            show_command_palette: self.show_command_palette,
            show_provider_panel: self.show_provider_panel,
            command_palette: &mut self.command_palette,
            provider_panel_state: &mut self.provider_panel_state,
            is_streaming: self.is_streaming,
            is_command_mode,
            anim_tick: self.anim_tick,
            running_tools: self.running_tools.clone(),
        };

        render_full_layout(frame, area, &mut chrome);
    }
}

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
            Message::Paste(text) => {
                if !self.is_streaming {
                    self.editor.paste_text(&text);
                }
                None
            }
            _ => None,
        }
    }
}
