use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

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
    ToolCallBox {
        state: ToolCallState,
    },

    /// Legacy simple inline tool call (not boxed)
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
                // Legacy rendering — render as compact inline
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
            TranscriptEntry::ToolCallBox { state } => {
                render_tool_call_box(state)
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

/// Render a boxed tool call entry with bordered box, collapsible args, and inline result preview.
fn render_tool_call_box(state: &ToolCallState) -> Text<'static> {
    let border_color = match state.status {
        ToolCallStatus::Pending => theme::DIM,
        ToolCallStatus::Running => theme::TOOL_BOX_BORDER,
        ToolCallStatus::Completed => theme::SUCCESS,
        ToolCallStatus::Errored => theme::ERROR,
    };
    let border_style = Style::default().fg(border_color);
    let title_text = state.title();
    let title_style = match state.status {
        ToolCallStatus::Completed => theme::style_tool_box_ok(),
        ToolCallStatus::Errored => theme::style_tool_box_err(),
        _ => theme::style_tool_box_title(),
    };

    let mut lines: Vec<Line<'static>> = Vec::new();

    // ── Arguments section (collapsible) ────────────────────────────────
    if state.expanded {
        if !state.args_kv.is_empty() {
            lines.push(Line::from(Span::styled(
                " Arguments ",
                theme::style_dim(),
            )));
            for (k, v) in &state.args_kv {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(k.clone(), Style::default().fg(theme::FG).add_modifier(Modifier::BOLD)),
                    Span::styled(": ", theme::style_dim()),
                    Span::styled(v.clone(), Style::default().fg(theme::DIM)),
                ]));
            }
        }

        // ── Result section ─────────────────────────────────────────────
        match (&state.result_preview, state.status) {
            (Some(preview), ToolCallStatus::Completed) => {
                lines.push(Line::from(Span::styled(
                    " Result ",
                    theme::style_dim(),
                )));
                // Show the first few lines of the result
                for (i, line) in preview.lines().enumerate().take(6) {
                    let truncated = if line.len() > 100 {
                        format!("{}…", &line[..97])
                    } else {
                        line.to_string()
                    };
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(truncated, Style::default().fg(theme::FG)),
                    ]));
                }
                if preview.lines().count() > 6 {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("… {} more lines", preview.lines().count() - 6),
                            theme::style_dim(),
                        ),
                    ]));
                }
            }
            (Some(preview), ToolCallStatus::Errored) => {
                lines.push(Line::from(Span::styled(
                    " Error ",
                    theme::style_error(),
                )));
                let first_line = preview.lines().next().unwrap_or("");
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(first_line, theme::style_error()),
                ]));
            }
            (None, ToolCallStatus::Running) => {
                lines.push(Line::from(Span::styled(
                    "  ⏳ running…",
                    theme::style_dim(),
                )));
            }
            (None, ToolCallStatus::Pending) => {
                lines.push(Line::from(Span::styled(
                    "  ⋯ queued",
                    theme::style_dim(),
                )));
            }
            _ => {}
        }
    } else {
        // Collapsed: show a compact summary
        match (&state.result_preview, state.status) {
            (Some(preview), ToolCallStatus::Completed) => {
                let summary = preview.lines().next().unwrap_or("");
                let truncated = if summary.len() > 80 {
                    format!("{}…", &summary[..77])
                } else {
                    summary.to_string()
                };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(truncated, Style::default().fg(theme::DIM)),
                ]));
            }
            (Some(_), ToolCallStatus::Errored) => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("Error — see details", theme::style_error()),
                ]));
            }
            (None, ToolCallStatus::Running) => {
                lines.push(Line::from(Span::styled(
                    "  ⏳ running…",
                    theme::style_dim(),
                )));
            }
            (None, ToolCallStatus::Pending) => {
                lines.push(Line::from(Span::styled(
                    "  ⋯ queued",
                    theme::style_dim(),
                )));
            }
            _ => {}
        }
    }

    // ── Hints in collapsed state ───────────────────────────────────────
    if !state.expanded && state.status == ToolCallStatus::Completed {
        lines.push(Line::from(Span::styled(
            "  Ctrl+E to expand",
            theme::style_dim(),
        )));
    }

    // Build the bordered block
    let inner_text = Text::from(lines);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Line::from(Span::styled(title_text, title_style)));

    let para = Paragraph::new(inner_text)
        .block(block)
        .style(Style::default().bg(theme::BG));

    // Render into a temporary buffer and extract lines
    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 1, 1));
    // We need to render the paragraph to figure out the output
    // Instead, let's construct the text manually with box-drawing chars

    // Actually, let's use a simpler approach: just return the paragraph rendered as text
    // by constructing the box-drawing manually.

    let mut output_lines: Vec<Line<'static>> = Vec::new();

    // Determine the width for the box
    // We'll use a fixed approach: construct inline text with box chars
    // Top border with title
    let title = state.title();
    let title_len = title.len() as u16;
    let min_width = title_len + 6; // padding
    let box_width = 60u16.max(min_width); // will be clamped by the transcript render

    // Top border: ┌─ <title> ─...─┐
    let title_chars: Vec<char> = title.chars().collect();
    let title_width = title_chars.len() as u16;
    let dash_count = box_width.saturating_sub(title_width).saturating_sub(4);
    let top_line: String = format!(
        "┌─{}─{}\u{2500}┐",
        title_chars.iter().collect::<String>(),
        "─".repeat(dash_count as usize),
    );
    output_lines.push(Line::from(Span::styled(top_line, border_style)));

    // Inner content lines with side borders
    for line in inner_text.lines.iter() {
        let content_width = line.width() as u16;
        let padding = box_width.saturating_sub(content_width).saturating_sub(2);
        let mut spans = vec![Span::styled("│", border_style)];
        spans.extend(line.spans.clone());
        if padding > 0 {
            spans.push(Span::raw(" ".repeat(padding as usize)));
        }
        spans.push(Span::styled("│", border_style));
        output_lines.push(Line::from(spans));
    }

    // Bottom border: └─...─┘
    let bottom_line = format!("└{:─>width$}┘", "", width = box_width as usize - 1);
    // Actually: └ + ──...── + ┘
    let bottom = format!(
        "└{}\u{2500}┘",
        "─".repeat((box_width - 1) as usize),
    );
    // Let me just make a simple bottom line
    let bottom_simple = format!("└{:─<width$}┘", "", width = box_width as usize - 1);
    output_lines.push(Line::from(Span::styled(bottom_simple, border_style)));

    Text::from(output_lines)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_kv_empty() {
        let kv = parse_args_kv("");
        assert!(kv.is_empty());
    }

    #[test]
    fn test_parse_args_kv_json_object() {
        let kv = parse_args_kv(r#"{"filePath": "src/main.rs", "limit": 2000}"#);
        assert!(kv.len() >= 2);
        assert!(kv.iter().any(|(k, _)| k == "filePath"));
        assert!(kv.iter().any(|(k, _)| k == "limit"));
    }

    #[test]
    fn test_tool_call_state_title_pending() {
        let state = ToolCallState::new("read".into(), r#"{}"#.into());
        let title = state.title();
        assert!(title.contains("read"));
        assert!(title.contains("▶"));
    }
}

/// Tool call execution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCallStatus {
    Pending,
    Running,
    Completed,
    Errored,
}

/// State for a boxed tool call entry.
#[derive(Clone)]
pub struct ToolCallState {
    pub tool_name: String,
    /// Raw arguments JSON string
    pub args: String,
    /// Parsed key-value lines for display (computed from args)
    pub args_kv: Vec<(String, String)>,
    /// Full result text, if available
    pub result: Option<String>,
    /// Preview snippet of the result (first N chars)
    pub result_preview: Option<String>,
    /// Whether arguments are expanded (Ctrl+E to toggle)
    pub expanded: bool,
    /// Execution status
    pub status: ToolCallStatus,
    /// Duration string like "12ms"
    pub duration: Option<String>,
}

impl ToolCallState {
    pub fn new(tool_name: String, args: String) -> Self {
        let args_kv = parse_args_kv(&args);
        Self {
            tool_name,
            args,
            args_kv,
            result: None,
            result_preview: None,
            expanded: true, // auto-expand on create
            status: ToolCallStatus::Running,
            duration: None,
        }
    }

    /// Compute the title string for the box border.
    pub fn title(&self) -> String {
        let icon = match self.status {
            ToolCallStatus::Pending => "⋯",
            ToolCallStatus::Running => "▶",
            ToolCallStatus::Completed => "✓",
            ToolCallStatus::Errored => "✗",
        };
        let dur = self.duration.as_deref().unwrap_or("");
        if self.expanded {
            format!(" {} {} {} ", icon, self.tool_name, dur)
        } else {
            let kv_count = self.args_kv.len();
            let dur_suffix = if !dur.is_empty() { format!(" {}", dur) } else { String::new() };
            if self.result_preview.is_some() {
                format!(" {} {} ({} args){}", icon, self.tool_name, kv_count, dur_suffix)
            } else {
                format!(" {} {} ({} args){}", icon, self.tool_name, kv_count, dur_suffix)
            }
        }
    }
}

/// Parse a JSON arguments string into key-value pairs for clean display.
fn parse_args_kv(args: &str) -> Vec<(String, String)> {
    if args.trim().is_empty() {
        return Vec::new();
    }
    // Try to parse as JSON object
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(args) {
        if let Some(obj) = val.as_object() {
            let mut pairs: Vec<(String, String)> = Vec::new();
            for (k, v) in obj {
                let v_str = match v {
                    serde_json::Value::String(s) => {
                        if s.len() > 80 {
                            format!("\"{}…\"", &s[..77])
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    other => other.to_string(),
                };
                pairs.push((k.clone(), v_str));
            }
            return pairs;
        }
    }
    // Fallback: show raw args as a single entry
    let preview = if args.len() > 100 {
        format!("{}…", &args[..97])
    } else {
        args.to_string()
    };
    vec![("args".to_string(), preview)]
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
