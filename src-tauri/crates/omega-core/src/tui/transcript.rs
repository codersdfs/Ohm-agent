use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

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
    /// Tool call — name + summarized args
    ToolCall {
        tool_name: String,
        args: String,
        result: Option<String>,
    },
    /// System notice or error
    Notice {
        text: String,
        is_error: bool,
    },
}

impl TranscriptEntry {
    /// Render (or re-render) the entry's text content into ratatui Lines.
    pub fn render_to_text(&mut self) -> Text<'static> {
        match self {
            TranscriptEntry::User { content } => {
                let mut text = markdown::render_markdown(content);
                // Prepend user marker
                let marker = Line::from(vec![
                    Span::styled("┃ ", Style::default().fg(theme::USER_MARKER)),
                ]);
                let mut all = vec![marker];
                all.append(&mut text.lines);
                Text::from(all)
            }
            TranscriptEntry::Assistant { content, rendered, is_streaming, thinking } => {
                let mut all = Vec::new();

                // Agent marker
                all.push(Line::from(vec![
                    Span::styled("▸ ", Style::default().fg(theme::AGENT_MARKER)),
                ]));

                // Thinking/reasoning block (dimmed, before actual content)
                if !thinking.is_empty() || (*is_streaming && content.is_empty()) {
                    let label = if thinking.is_empty() {
                        "thinking…"
                    } else {
                        "reasoning"
                    };
                    all.push(Line::from(vec![
                        Span::styled(format!("  {} ", label), theme::style_dim()),
                    ]));
                    if !thinking.is_empty() {
                        let mut thinking_lines = markdown::render_markdown(thinking).lines;
                        for line in thinking_lines.iter_mut() {
                            let dimmed: Vec<Span> = line.spans.iter().map(|s| {
                                Span::styled(s.content.clone(), theme::style_dim())
                            }).collect();
                            all.push(Line::from(dimmed));
                        }
                    }
                }

                // Actual response content
                if !content.is_empty() {
                    let mut text = markdown::render_markdown(content);
                    all.append(&mut text.lines);
                }

                if *is_streaming {
                    all.push(Line::from(Span::styled(
                        " ⠋",
                        Style::default().fg(theme::ACCENT),
                    )));
                }
                let t = Text::from(all);
                *rendered = Some(t.clone());
                t
            }
            TranscriptEntry::ToolCall { tool_name, args, result } => {
                let mut lines = Vec::new();
                let args_preview: String = if args.len() > 80 {
                    format!("{}…", &args[..77])
                } else {
                    args.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default().fg(theme::TOOL_MARKER)),
                    Span::styled("▶", Style::default().fg(theme::TOOL_MARKER)),
                    Span::styled(format!(" {}", tool_name), Style::default().fg(theme::TOOL_MARKER).add_modifier(ratatui::style::Modifier::BOLD)),
                    Span::styled(format!(" {}", args_preview), Style::default().fg(theme::DIM)),
                ]));
                if let Some(r) = result {
                    // Show first line of result
                    let first_line = r.lines().next().unwrap_or("");
                    if !first_line.is_empty() {
                        let preview = if first_line.len() > 80 {
                            format!("{}…", &first_line[..77])
                        } else {
                            first_line.to_string()
                        };
                        lines.push(Line::from(vec![
                            Span::styled("  └─ ", Style::default().fg(theme::DIM)),
                            Span::styled(preview, Style::default().fg(theme::DIM)),
                        ]));
                    }
                }
                Text::from(lines)
            }
            TranscriptEntry::Notice { text, is_error } => {
                let style = if *is_error {
                    Style::default().fg(theme::ERROR)
                } else {
                    Style::default().fg(theme::DIM)
                };
                Text::from(Line::from(vec![
                    Span::styled("  ", style),
                    Span::styled(if *is_error { "⚠ " } else { "· " }, style),
                    Span::styled(text.clone(), style),
                ]))
            }
        }
    }

    /// Get the rendered text, rendering if needed.
    pub fn get_rendered(&mut self) -> Text<'static> {
        match self {
            TranscriptEntry::Assistant { rendered, .. } => {
                if let Some(r) = rendered.take() {
                    return r;
                }
                self.render_to_text()
            }
            _ => self.render_to_text(),
        }
    }
}

/// Scroll state for the transcript.
pub struct ScrollState {
    pub offset: usize,       // Scroll offset in lines from top
    pub auto_scroll: bool,   // Whether to follow new content
}

impl Default for ScrollState {
    fn default() -> Self {
        Self { offset: 0, auto_scroll: true }
    }
}

/// Render the transcript area.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    entries: &mut [TranscriptEntry],
    scroll: &mut ScrollState,
) {
    if area.height < 1 || area.width < 2 {
        return;
    }

    // Build the full rendered text from all entries
    let mut all_lines: Vec<Line<'static>> = Vec::new();
    for entry in entries.iter_mut() {
        let rendered = entry.get_rendered();
        all_lines.extend(rendered.lines);
    }

    let total_lines = all_lines.len();
    let view_height = area.height as usize;

    // Auto-scroll to bottom
    if scroll.auto_scroll && total_lines > view_height {
        scroll.offset = total_lines.saturating_sub(view_height);
    }

    // Clamp scroll offset
    if total_lines > view_height {
        scroll.offset = scroll.offset.min(total_lines.saturating_sub(view_height));
    } else {
        scroll.offset = 0;
    }

    // Visible slice
    let visible: Vec<Line<'static>> = if total_lines > scroll.offset {
        all_lines[scroll.offset..].to_vec()
    } else {
        all_lines.clone()
    };

    let text = Text::from(visible);

    let para = Paragraph::new(text)
        .block(Block::default().style(Style::default().bg(theme::BG)))
        .style(Style::default().bg(theme::BG))
        .wrap(Wrap { trim: false });
    para.render(area, buf);
}

/// Scroll up by `delta` lines.
pub fn scroll_up(scroll: &mut ScrollState, delta: usize) {
    scroll.auto_scroll = false;
    scroll.offset = scroll.offset.saturating_sub(delta);
}

/// Scroll down by `delta` lines.
pub fn scroll_down(scroll: &mut ScrollState, total_lines_hint: usize, delta: usize) {
    let max_offset = total_lines_hint.saturating_sub(1);
    if scroll.offset + delta >= max_offset {
        scroll.auto_scroll = true;
        scroll.offset = 0;
    } else {
        scroll.offset = scroll.offset.saturating_add(delta);
    }
}

/// Scroll to top.
pub fn scroll_top(scroll: &mut ScrollState) {
    scroll.auto_scroll = false;
    scroll.offset = 0;
}

/// Scroll to bottom.
pub fn scroll_bottom(scroll: &mut ScrollState) {
    scroll.auto_scroll = true;
    scroll.offset = 0;
}
