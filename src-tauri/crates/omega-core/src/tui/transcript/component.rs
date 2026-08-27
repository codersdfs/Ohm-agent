//! Transcript component (P5 split).

use ratatui::layout::Rect;

use super::render::{render, scroll_down, scroll_up};
use super::shell::shorten;
use super::state::{ScrollState, ToolCallState, ToolCallStatus};
use super::TranscriptEntry;
use crate::tui::component::{Action, UiStreamEvent};

use crate::ui::permission_panel::PermissionPanelState;

// ─── Transcript Component ────────────────────────────────────────────────────

/// Aggregated transcript state: entries, scroll, conversation history, streaming channel.
pub struct Transcript {
    pub entries: Vec<TranscriptEntry>,
    pub scroll: ScrollState,
    pub messages: Vec<providers::ChatMessage>,
    pub stream_event_rx:
        Option<tokio::sync::mpsc::UnboundedReceiver<super::component::UiStreamEvent>>,
    pub streaming_fragment: String,
    pub tools_expanded: bool,
    pub activity_tick: u64,
    pub permission_state: PermissionPanelState,
}

impl Transcript {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            scroll: ScrollState::default(),
            messages: Vec::new(),
            stream_event_rx: None,
            streaming_fragment: String::new(),
            tools_expanded: false,
            activity_tick: 0,
            permission_state: PermissionPanelState::default(),
        }
    }

    /// Restore provider history and UI entries from a loaded session.
    /// Exact ChatMessage history is preserved for the LLM; UI entries are approximate.
    pub fn load_from_session(&mut self, messages: Vec<providers::ChatMessage>) {
        self.messages = messages;
        let ui = crate::session::messages_to_transcript_entries(&self.messages);
        self.entries.extend(ui);
        self.scroll.auto_scroll = true;
    }

    pub fn tick_activity(&mut self) {
        self.activity_tick = self.activity_tick.wrapping_add(1);
    }

    /// Globally expand or collapse all structured tool executions.
    pub fn set_tools_expanded(&mut self, expanded: bool) {
        self.tools_expanded = expanded;
        for entry in &mut self.entries {
            if let TranscriptEntry::ToolCallBox { state } = entry {
                state.expanded = expanded;
            }
        }
        self.scroll.auto_scroll = true;
    }

    /// Add a notice entry to the transcript.
    pub fn add_notice(&mut self, text: String, is_error: bool) {
        self.entries
            .push(TranscriptEntry::Notice { text, is_error });
    }

    /// Whether any real conversation content exists (user message, assistant
    /// reply, or tool execution). Startup notices do not count — the splash
    /// banner stays up until actual interaction begins.
    pub fn has_conversation(&self) -> bool {
        self.entries.iter().any(|e| {
            matches!(
                e,
                TranscriptEntry::User { .. }
                    | TranscriptEntry::Assistant { .. }
                    | TranscriptEntry::ToolCallBox { .. }
                    | TranscriptEntry::ToolCall { .. }
            )
        })
    }

    /// Add a user message entry to the transcript.
    pub fn add_user_message(&mut self, content: String) {
        self.entries.push(TranscriptEntry::User { content });
    }

    /// Add an assistant entry to the transcript (for streaming).
    pub fn add_assistant_entry(&mut self) {
        self.entries.push(TranscriptEntry::Assistant {
            content: String::new(),
            rendered: None,
            is_streaming: true,
            thinking: String::new(),
        });
    }

    /// Clear all entries and messages (for /clear command).
    pub fn clear_transcript(&mut self) {
        self.entries.clear();
        self.messages.clear();
    }

    /// Set the auto-scroll flag on the scroll state.
    pub fn set_scroll_auto(&mut self, auto: bool) {
        self.scroll.auto_scroll = auto;
    }

    /// Clear the streaming fragment buffer.
    pub fn clear_streaming_fragment(&mut self) {
        self.streaming_fragment.clear();
    }

    /// Set the stream event receiver.
    pub fn set_stream_rx(
        &mut self,
        rx: tokio::sync::mpsc::UnboundedReceiver<super::component::UiStreamEvent>,
    ) {
        self.stream_event_rx = Some(rx);
    }

    /// Take the stream event receiver (returns None if already taken).
    pub fn take_stream_rx(
        &mut self,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<super::component::UiStreamEvent>> {
        self.stream_event_rx.take()
    }

    /// Drop the stream event receiver (sets to None).
    pub fn drop_stream_rx(&mut self) {
        self.stream_event_rx = None;
    }

    /// Mark the last streaming assistant entry as stopped.
    pub fn mark_streaming_stopped(&mut self) {
        for entry in self.entries.iter_mut().rev() {
            if let TranscriptEntry::Assistant {
                ref mut is_streaming,
                ..
            } = entry
            {
                *is_streaming = false;
                break;
            }
        }
    }

    /// Returns the number of entries in the transcript.
    pub fn entries_len(&self) -> usize {
        self.entries.len()
    }

    /// Scroll up by `delta` lines.
    pub fn scroll_up(&mut self, delta: usize) {
        scroll_up(&mut self.scroll, delta);
    }

    /// Scroll down by `delta` lines.
    pub fn scroll_down(&mut self, delta: usize) {
        let total = self.entries.len();
        scroll_down(&mut self.scroll, total, delta);
    }

    /// Returns a clone of the messages for the streaming task.
    pub fn messages_clone(&self) -> Vec<providers::ChatMessage> {
        self.messages.clone()
    }

    /// Process one streaming event from the channel. Returns an action for the caller.
    pub fn process_stream_event(&mut self, event: &super::component::UiStreamEvent) -> Action {
        match event {
            super::component::UiStreamEvent::Token(t) => {
                self.streaming_fragment.push_str(t);
                let follows_tool = matches!(
                    self.entries.last(),
                    Some(TranscriptEntry::ToolCall { .. } | TranscriptEntry::ToolCallBox { .. })
                );
                if follows_tool {
                    let drop_idx = if self.entries.len() >= 2 {
                        let i = self.entries.len() - 2;
                        match &self.entries[i] {
                            TranscriptEntry::Assistant { content, .. } if content.is_empty() => {
                                Some(i)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(i) = drop_idx {
                        self.entries.remove(i);
                    }
                    self.add_assistant_entry();
                }
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant {
                        content,
                        rendered,
                        is_streaming,
                        ..
                    } = entry
                    {
                        content.push_str(t);
                        *rendered = None;
                        *is_streaming = true;
                        break;
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::Thinking(t) => {
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant {
                        ref mut thinking,
                        ref mut rendered,
                        is_streaming,
                        ..
                    } = entry
                    {
                        thinking.push_str(t);
                        *rendered = None;
                        *is_streaming = true;
                        break;
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::ThinkingDone => {
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant {
                        ref mut is_streaming,
                        ..
                    } = entry
                    {
                        *is_streaming = false;
                        break;
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::ToolCall { name, args } => {
                // Parse once while the complete JSON is available. ToolCallState
                // retains only bounded display data; large write source is dropped.
                let mut state = ToolCallState::new(name.clone(), args.clone());
                state.expanded = self.tools_expanded;
                self.entries.push(TranscriptEntry::ToolCallBox { state });
                Action::Noop
            }
            super::component::UiStreamEvent::ToolResult {
                name,
                success,
                output,
            } => {
                // Char-safe truncation (byte slicing can panic on multibyte UTF-8).
                // `shorten` appends the ellipsis itself when it truncates.
                let preview: String = if output.chars().count() > 200 {
                    shorten(output, 200)
                } else {
                    output.clone()
                };
                for entry in self.entries.iter_mut().rev() {
                    match entry {
                        TranscriptEntry::ToolCallBox { state } => {
                            state.result = Some(preview.clone());
                            state.result_preview = Some(preview.clone());
                            if *success {
                                state.status = ToolCallStatus::Completed;
                            } else {
                                let flat = format!("ERROR: {}", preview);
                                let typed = crate::error::AgentError::from_flat_string(&flat)
                                    .typed_tool_error()
                                    .unwrap_or_else(|| {
                                        crate::error::ToolCallError::new(
                                            name.clone(),
                                            crate::error::ToolErrorKind::ExecutionFailed,
                                            preview.trim_start_matches("ERROR:").trim().to_string(),
                                        )
                                    });
                                state.status = ToolCallStatus::Errored;
                                state.error = Some(typed);
                            }
                            break;
                        }
                        TranscriptEntry::ToolCall { result, .. } => {
                            *result = Some(if *success {
                                preview.clone()
                            } else {
                                format!("ERROR: {}", preview)
                            });
                            break;
                        }
                        _ => {}
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::Done {
                full: _,
                tokens_in,
                tokens_out,
                messages,
            } => {
                // Flip the trailing assistant entry out of streaming mode so
                // its content renders (and the live cursor disappears).
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant {
                        is_streaming,
                        rendered,
                        ..
                    } = entry
                    {
                        *is_streaming = false;
                        *rendered = None;
                        break;
                    }
                }
                self.messages = messages.clone();
                Action::StreamDone {
                    tokens_in: *tokens_in,
                    tokens_out: *tokens_out,
                }
            }
            super::component::UiStreamEvent::Error(e) => {
                self.add_notice(e.clone(), true);
                if let Some(last) = self.entries.last() {
                    if let TranscriptEntry::Assistant { content, .. } = last {
                        if content.is_empty() {
                            self.entries.pop();
                        }
                    }
                }
                Action::StreamError
            }
            super::component::UiStreamEvent::PermissionRequest {
                prompt,
                options,
                default_idx,
            } => Action::PermissionRequest {
                prompt: prompt.clone(),
                options: options.clone(),
                default_idx: *default_idx,
            },
            super::component::UiStreamEvent::PermissionResponse(_allowed) => Action::Noop,
            super::component::UiStreamEvent::PermissionCancel => Action::Noop,
        }
    }
}

impl Transcript {
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        use crossterm::event::{KeyCode, KeyEventKind};
        if key.kind != KeyEventKind::Press {
            return Action::Noop;
        }
        match key.code {
            KeyCode::Up => Action::ScrollUp(3),
            KeyCode::Down => Action::ScrollDown(3),
            KeyCode::PageUp => Action::ScrollUp(10),
            KeyCode::PageDown => Action::ScrollDown(10),
            _ => Action::Noop,
        }
    }

    pub fn render(&mut self, f: &mut ratatui::Frame, area: Rect) {
        // Split area: transcript gets most space, permission panel gets bottom when visible
        if self.permission_state.visible {
            let panel_height = 4u16;
            if area.height > panel_height {
                let transcript_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: area.height.saturating_sub(panel_height),
                };
                render(
                    transcript_area,
                    f.buffer_mut(),
                    &mut self.entries,
                    &mut self.scroll,
                    self.activity_tick,
                );
                // Render permission panel at the bottom
                crate::ui::permission_panel::render_permission_panel(
                    area,
                    f.buffer_mut(),
                    &self.permission_state,
                );
            } else {
                // Not enough space for both, just show transcript
                render(
                    area,
                    f.buffer_mut(),
                    &mut self.entries,
                    &mut self.scroll,
                    self.activity_tick,
                );
            }
        } else {
            render(
                area,
                f.buffer_mut(),
                &mut self.entries,
                &mut self.scroll,
                self.activity_tick,
            );
        }
    }
}
