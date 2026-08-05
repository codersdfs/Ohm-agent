//! App key/mouse handling + input history (P5 split from main.rs).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};
use omega_core::tui::component::Action;
use omega_core::tui::editor::EditorMode;
use omega_core::tui::spinner::SpinnerState;

use super::App;
use crate::config_loader::{save_api_key, save_config};

impl App {
    /// Handle a key event.
    pub fn handle_key(&mut self, key: KeyEvent) {
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
                    self.transcript.add_notice(format!(
                            "Provider set to {} ({})",
                            self.config.model, self.config.kind
                        ), false);
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
                self.transcript.scroll_up(3);
            }
            KeyCode::Down => {
                self.recall_history_down();
                self.transcript.scroll_down(3);
            }
            KeyCode::PageUp => {
                self.transcript.scroll_up(10);
            }
            KeyCode::PageDown => {
                self.transcript.scroll_down(10);
            }
            _ => {}
        }
    }

    /// Cancel the current streaming request.
    pub fn cancel_streaming(&mut self) {
        self.cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);

        // Drop the receiver so the streaming task's tx.send() fails
        self.transcript.drop_stream_rx();

        self.is_streaming = false;
        self.editor.state = EditorMode::Idle;
        self.editor.buffer.clear();
        self.editor.cursor = 0;
        self.status.set_spinner_state(SpinnerState::Idle);

        // Mark the pending assistant entry as stopped
        self.transcript.mark_streaming_stopped();

        // Show cancel notice
        self.transcript.add_notice("Stream cancelled".into(), false);

        self.transcript.set_scroll_auto(true);
        self.transcript.clear_streaming_fragment();
    }

    /// Navigate input history: move to older entry.
    pub fn recall_history_up(&mut self) {
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
    pub fn recall_history_down(&mut self) {
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
    pub fn handle_mouse(&mut self, kind: MouseEventKind) {
        match kind {
            MouseEventKind::ScrollDown => {
                self.transcript.scroll_down(3);
            }
            MouseEventKind::ScrollUp => {
                self.transcript.scroll_up(3);
            }
            _ => {}
        }
    }

    /// Submit the current editor buffer as a message.
    pub fn submit_message(&mut self) {
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
        self.transcript.add_user_message(content.clone());

        // Start streaming
        self.start_streaming(content);
    }

    pub fn open_command_palette(&mut self, seed_query: &str) {
        if self.is_streaming || self.show_provider_panel {
            return;
        }
        self.show_help = false;
        self.command_palette.open(seed_query);
        self.show_command_palette = true;
    }

    /// Read a file by path, or fall back to the editor buffer if path is empty.
    /// Returns Ok((path_display, content)) or Err with an error message.
    pub fn read_file_or_buffer(&self, cmd: &str, prefix: &str) -> anyhow::Result<(String, String), String> {
        let path = cmd.trim_start_matches(prefix).trim();
        let content = if path.is_empty() {
            self.editor.buffer.clone()
        } else {
            std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read {path}: {e}"))?
        };
        if content.is_empty() {
            return Err("File is empty".into());
        }
        Ok((path.to_string(), content))
    }
}
