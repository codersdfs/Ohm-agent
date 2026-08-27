//! Streaming task spawn + event processing (P5 split from main.rs).

use std::sync::atomic::Ordering;
use std::sync::Arc;

use omega_core::commands;
use omega_core::tui::component::{Action, UiStreamEvent};
use omega_core::tui::editor::EditorMode;
use omega_core::tui::spinner::SpinnerState;

use super::{App, ChannelEmitter};

impl App {
    /// Start streaming a response from the LLM.
    pub fn start_streaming(&mut self, content: String) {
        self.is_streaming = true;
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.editor.state = EditorMode::Thinking;
        self.status.set_spinner_state(SpinnerState::Thinking);

        // Create channel
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.transcript.set_stream_rx(rx);

        // Add a placeholder assistant entry
        self.transcript.add_assistant_entry();

        // Get references for the async task
        let state = self.state.clone();
        let config = self.config.clone();
        let system_prompt = commands::tools::default_system_prompt();
        let permission_mode = "off".to_string();

        // Shared message list for the task to modify
        let messages = Arc::new(tokio::sync::Mutex::new(self.transcript.messages_clone()));
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
                omega_core::commands::cost_tracker::session_token_counts();

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
                omega_core::commands::cost_tracker::session_token_counts();
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
    pub fn process_stream_events(&mut self) {
        let rx = self.transcript.take_stream_rx();
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
                UiStreamEvent::ToolCall { name, .. } => {
                    self.status.set_spinner_state(SpinnerState::ToolCall);
                    // Track running tool
                    if !self.running_tools.contains(name) {
                        self.running_tools.push(name.clone());
                    }
                }
                UiStreamEvent::ToolResult { name, .. } => {
                    // Remove from running tools if present
                    self.running_tools.retain(|t| t != name);
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
            self.transcript.drop_stream_rx();
            self.transcript.clear_streaming_fragment();
            self.transcript.set_scroll_auto(true); // jump to bottom
        } else {
            // Put the rx back if we're still streaming
            self.transcript.set_stream_rx(rx);
        }
    }

    /// Advance the spinner animation.
    pub fn tick_spinner(&mut self) {
        self.status.tick_spinner();
        self.transcript.tick_activity();
    }

    /// Poll the provider panel model-fetch channel.
    pub fn poll_provider_models(&mut self) {
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
            self.provider_panel_state.reset_filter_state();

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
                max_concurrent_tools: self.config.max_concurrent_tools,
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
}
