use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use super::markdown;
use super::theme;



/// A single entry in the conversation transcript.
#[derive(Clone)]
pub enum TranscriptEntry {
    /// User message (plain text or markdown)
    User { content: String },
    /// Assistant message (markdown rendered, with optional thinking/reasoning prefix)
    Assistant {
        content: String,
        rendered: Option<Text<'static>>,
        is_streaming: bool,
        /// Model-internal reasoning/thinking, shown dimmed before content
        thinking: String,
    },
    /// Tool call — rendered as a bordered box (Claude Code / Pi Agent style)
    ToolCallBox { state: ToolCallState },

    /// Legacy simple inline tool call (not boxed)
    ToolCall {
        tool_name: String,
        args: String,
        result: Option<String>,
    },
    /// System notice or error
    Notice { text: String, is_error: bool },
}

impl TranscriptEntry {
    /// Render (or re-render) the entry's text content into ratatui Lines.
    pub fn render_to_text(&mut self, _width: u16, activity_tick: u64) -> Text<'static> {
        match self {
            TranscriptEntry::User { content } => {
                markdown::render_markdown(content)
            }
            TranscriptEntry::Assistant {
                content,
                rendered,
                is_streaming,
                thinking,
            } => {
                let mut all = Vec::new();

                // Reasoning text remains below the activity line.
                if !thinking.is_empty() {
                    let mut thinking_lines = markdown::render_markdown(thinking).lines;
                    for line in thinking_lines.iter_mut() {
                        let dimmed: Vec<Span> = line
                            .spans
                            .iter()
                            .map(|s| Span::styled(s.content.clone(), theme::style_dim()))
                            .collect();
                        all.push(Line::from(dimmed));
                    }
                }

                // Actual response content. Render it both live (so streamed
                // tokens appear as they arrive) and after completion.
                if !content.is_empty() {
                    let mut text = markdown::render_markdown(content);
                    all.append(&mut text.lines);
                }

                // Live response cursor uses a conventional terminal spinner.
                // Live response cursor
                if *is_streaming && !content.is_empty() {
                    all.push(Line::from(Span::styled(" █", Style::default().fg(theme::PRIMARY))));
                }

                let t = Text::from(all);
                *rendered = Some(t.clone());
                t
            }
            TranscriptEntry::ToolCall {
                tool_name,
                args,
                result,
            } => render_tool_call_box_simple(tool_name, args, result, _width),
            TranscriptEntry::ToolCallBox { state } => render_tool_call_compact(state, _width),
            TranscriptEntry::Notice { text, is_error } => {
                // Try to detect a typed error via flat-string prefix so notices
                // get the right neon chip / icon instead of always error-red bold.
                let typed = if *is_error {
                    Some(crate::error::AgentError::from_flat_string(text))
                } else {
                    None
                };
                let is_quiet = typed.as_ref().map(|e| e.is_quiet()).unwrap_or(false);

                let style = if is_quiet {
                    theme::style_dim()
                } else if *is_error {
                    let col = typed
                        .as_ref()
                        .map(|e| e.chip_color())
                        .unwrap_or(theme::ERROR);
                    Style::default().fg(col).add_modifier(Modifier::BOLD)
                } else {
                    theme::style_dim()
                };

                let prefix = match (&typed, *is_error) {
                    (Some(e), _) => format!("{} ", e.icon()),
                    (None, true) => "✗ ".to_string(),
                    (None, false) => "· ".to_string(),
                };

                // Render the chip label inline when it's a typed error.
                if let Some(e) = &typed {
                    let chip = format!("[ {} ] ", e.chip_label());
                    Text::from(Line::from(vec![
                        Span::styled(
                            prefix.clone(),
                            Style::default()
                                .fg(e.chip_color())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(chip, e.style()),
                        Span::styled(e.message(), style),
                    ]))
                } else {
                    Text::from(Line::from(vec![
                        Span::styled(prefix.to_string(), style),
                        Span::styled(text.clone(), style),
                    ]))
                }
            }
        }
    }

    /// Get the rendered text, rendering if needed.
    pub fn get_rendered(&mut self, width: u16, activity_tick: u64) -> Text<'static> {
        match self {
            TranscriptEntry::Assistant {
                rendered,
                is_streaming,
                ..
            } => {
                // Streaming entries must be regenerated so the spinner advances.
                if !*is_streaming {
                    if let Some(r) = rendered.take() {
                        return r;
                    }
                }
                self.render_to_text(width, activity_tick)
            }
            _ => self.render_to_text(width, activity_tick),
        }
    }

    /// Check if this entry is a user message containing attachments.
    /// Returns true for User entries that contain URLs, file paths, or skill references.
    pub fn has_attachments(&self) -> bool {
        let content = match self {
            TranscriptEntry::User { content } => content,
            _ => return false,
        };
        has_attachment_content(content)
    }
}
pub mod component;
pub mod preview;
pub mod render;
pub mod shell;
pub mod state;
pub mod toolbox;
#[cfg(test)]
pub mod tests;

// Cross-module items used by submodules via direct use super::<module>::....
pub use component::Transcript;
pub use state::{ScrollState, ToolCallState, ToolCallStatus};
use shell::render_tool_call_compact;
use state::has_attachment_content;
use toolbox::render_tool_call_box_simple;