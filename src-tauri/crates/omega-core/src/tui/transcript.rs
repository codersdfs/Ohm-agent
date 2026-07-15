use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use super::markdown;
use super::theme;
use super::theme::tool_color;

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
    pub fn render_to_text(&mut self, _width: u16) -> Text<'static> {
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
                render_tool_call_box(tool_name, args, result, _width)
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
    pub fn get_rendered(&mut self, width: u16) -> Text<'static> {
        match self {
            TranscriptEntry::Assistant { rendered, .. } => {
                if let Some(r) = rendered.take() {
                    return r;
                }
                self.render_to_text(width)
            }
            _ => self.render_to_text(width),
        }
    }
}

/// Render a tool call box — Claude Code inspired: compact, colored borders,
/// tool name embedded in the top border, args dimmed, result truncated.
///
/// ```text
/// ┌─ read ─────────────────────────────────┐
/// │ src/main.rs                             │
/// ├─ 23 lines ─────────────────────────────┤
/// │ pub fn main() { ...                     │
/// └────────────────────────────────────────┘
/// ```
fn render_tool_call_box(tool_name: &str, args: &str, result: &Option<String>, avail_width: u16) -> Text<'static> {
    let color = tool_color(tool_name);
    let mut lines: Vec<Line<'static>> = Vec::new();

    let arg_lines: Vec<&str> = if args.trim().is_empty() {
        Vec::new()
    } else {
        args.lines().collect()
    };
    let max_arg = arg_lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    let result_lines: Vec<&str> = result
        .as_ref()
        .map(|r| r.lines().collect())
        .unwrap_or_default();
    let max_res = result_lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    let name_len = tool_name.chars().count();

    let (divider_text, is_err) = match result.as_ref().filter(|r| !r.trim().is_empty()) {
        Some(r) if r.starts_with("ERROR") => (" Error".to_string(), true),
        Some(_) => {
            let n = result_lines.len();
            if n > 0 { (format!(" {}L", n), false) }
            else { (" Result".to_string(), false) }
        }
        _ => (" Result".to_string(), false),
    };
    let divider_len = divider_text.chars().count();

    let inner = [max_arg + 1, max_res + 1, divider_len + 2]
        .iter().max().copied().unwrap_or(0);
    let box_w = inner
        .min(76)
        .min((avail_width as usize).saturating_sub(2).max(20))
        .max(name_len + 6);

    // Top border with tool name embedded: ┌─ read ──────────────┐
    let right_dashes = box_w.saturating_sub(name_len + 3); // 2 for "┌─", 1 for space
    lines.push(Line::from(vec![
        Span::styled("┌─", Style::default().fg(color)),
        Span::styled(tool_name.to_string(), Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" {}", "─".repeat(right_dashes)),
            Style::default().fg(color),
        ),
        Span::styled("┐", Style::default().fg(color)),
    ]));

    for al in &arg_lines {
        lines.extend(box_line(al, box_w, color, theme::DIM, Modifier::empty(), true));
    }

    match result {
        Some(r) if r.trim().is_empty() => {}
        Some(_) => {
            let res_color = if is_err { theme::ERROR } else { theme::SUCCESS };
            push_divider(&mut lines, box_w, color, &divider_text, res_color);
            let show = if result_lines.len() > 6 { &result_lines[..5] } else { &result_lines };
            for rl in show {
                lines.extend(box_line(rl, box_w, color, res_color, Modifier::empty(), true));
            }
            if result_lines.len() > 6 {
                let rest = result_lines.len() - 5;
                let sfx = if rest != 1 { " more lines" } else { " more line" };
                lines.extend(box_line(&format!("… {}{}", rest, sfx), box_w, color, theme::DIM, Modifier::empty(), true));
            }
        }
        None => {
            lines.extend(box_line("running…", box_w, color, theme::DIM, Modifier::empty(), true));
        }
    }

    // Bottom border
    lines.push(Line::from(vec![
        Span::styled("└", Style::default().fg(color)),
        Span::styled("─".repeat(box_w), Style::default().fg(color)),
        Span::styled("┘", Style::default().fg(color)),
    ]));

    Text::from(lines)
}

/// Build one interior line of the box: `│ <content><pad>│`.
///
/// If `content` exceeds the interior width it is **wrapped** into multiple
/// lines so the box borders stay intact (long results don't bleed out).
fn box_line(
    content: &str,
    box_w: usize,
    border_color: Color,
    content_color: Color,
    content_mod: Modifier,
    indent: bool,
) -> Vec<Line<'static>> {
    let prefix = if indent { " " } else { "" };
    let max_body = box_w;

    let mut out = Vec::new();
    let mut remaining = content;

    loop {
        if remaining.is_empty() {
            break;
        }
        // Chars available for actual content on this row.
        let is_first = out.is_empty();
        let lead = if is_first { prefix.len() } else { 1 }; // continuation lines always have a leading space
        let max_chunk = max_body.saturating_sub(lead);

        // Take up to max_body chars, respecting char boundaries.
        let chunk: String = remaining.chars().take(max_chunk).collect();
        let chunk_len = chunk.chars().count();

        let body = if is_first {
            format!("{}{}", prefix, &chunk)
        } else {
            format!(" {}", &chunk)
        };
        let pad = max_body.saturating_sub(body.chars().count());
        out.push(Line::from(vec![
            Span::styled("│", Style::default().fg(border_color)),
            Span::styled(body, Style::default().fg(content_color).add_modifier(content_mod)),
            Span::styled(" ".repeat(pad), Style::default().fg(border_color)),
            Span::styled("│", Style::default().fg(border_color)),
        ]));

        if chunk_len >= remaining.chars().count() {
            break;
        }
        remaining = &remaining[chunk.len()..];
    }

    if out.is_empty() {
        // Empty content — emit one empty line.
        let pad = max_body.saturating_sub(prefix.len());
        out.push(Line::from(vec![
            Span::styled("│", Style::default().fg(border_color)),
            Span::styled(prefix.to_string(), Style::default().fg(content_color).add_modifier(content_mod)),
            Span::styled(" ".repeat(pad), Style::default().fg(border_color)),
            Span::styled("│", Style::default().fg(border_color)),
        ]));
    }

    out
}

/// Push a centered `├─ Result ─┤` style divider into `lines`.
fn push_divider(lines: &mut Vec<Line<'static>>, box_w: usize, border_color: Color, title: &str, title_color: Color) {
    let title_len = title.chars().count();
    let fill = box_w.saturating_sub(title_len);
    let left = fill / 2;
    let right = fill - left;
    lines.push(Line::from(vec![
        Span::styled("├", Style::default().fg(border_color)),
        Span::styled("─".repeat(left), Style::default().fg(border_color)),
        Span::styled(title.to_string(), Style::default().fg(title_color).add_modifier(Modifier::BOLD)),
        Span::styled("─".repeat(right), Style::default().fg(border_color)),
        Span::styled("┤", Style::default().fg(border_color)),
    ]));
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
        let rendered = entry.get_rendered(area.width);
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


use crate::tui::component::{Action, Component};

/// Aggregated transcript state: entries, scroll, conversation history, streaming channel.
pub struct Transcript {
    pub entries: Vec<TranscriptEntry>,
    pub scroll: ScrollState,
    pub messages: Vec<providers::ChatMessage>,
    pub stream_event_rx: Option<tokio::sync::mpsc::UnboundedReceiver<super::component::UiStreamEvent>>,
    pub streaming_fragment: String,
}

impl Transcript {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            scroll: ScrollState::default(),
            messages: Vec::new(),
            stream_event_rx: None,
            streaming_fragment: String::new(),
        }
    }

    /// Process one streaming event from the channel. Returns an action for the caller.
    pub fn process_stream_event(&mut self, event: &super::component::UiStreamEvent) -> Action {
        match event {
            super::component::UiStreamEvent::Token(t) => {
                self.streaming_fragment.push_str(t);
                // Append to the current assistant entry, creating a new bubble
                // after a tool call (so the post-tool answer renders below the box).
                let follows_tool = matches!(self.entries.last(), Some(TranscriptEntry::ToolCall { .. }));
                if follows_tool {
                    let drop_idx = if self.entries.len() >= 2 {
                        let i = self.entries.len() - 2;
                        match &self.entries[i] {
                            TranscriptEntry::Assistant { content, .. } if content.is_empty() => Some(i),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(i) = drop_idx {
                        self.entries.remove(i);
                    }
                    self.entries.push(TranscriptEntry::Assistant {
                        content: String::new(),
                        rendered: None,
                        is_streaming: true,
                        thinking: String::new(),
                    });
                }
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant { content, rendered, is_streaming, .. } = entry {
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
                    if let TranscriptEntry::Assistant { ref mut thinking, ref mut rendered, is_streaming, .. } = entry {
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
                    if let TranscriptEntry::Assistant { ref mut is_streaming, .. } = entry {
                        *is_streaming = false;
                        break;
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::ToolCall { name, args } => {
                let args_preview: String = if args.len() > 120 {
                    format!("{}…", &args[..117])
                } else {
                    args.clone()
                };
                self.entries.push(TranscriptEntry::ToolCall {
                    tool_name: name.clone(),
                    args: args_preview,
                    result: None,
                });
                Action::Noop
            }
            super::component::UiStreamEvent::ToolResult { name: _, success, output } => {
                let preview: String = if output.len() > 200 {
                    format!("{}…", &output[..197])
                } else {
                    output.clone()
                };
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::ToolCall { result, .. } = entry {
                        *result = Some(if *success {
                            preview.clone()
                        } else {
                            format!("ERROR: {}", preview)
                        });
                        break;
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::Done { full: _, tokens_in, tokens_out, messages } => {
                self.messages = messages.clone();
                Action::StreamDone {
                    tokens_in: *tokens_in,
                    tokens_out: *tokens_out,
                }
            }
            super::component::UiStreamEvent::Error(e) => {
                self.entries.push(TranscriptEntry::Notice {
                    text: e.clone(),
                    is_error: true,
                });
                // Remove empty assistant entry
                if let Some(last) = self.entries.last() {
                    if let TranscriptEntry::Assistant { content, .. } = last {
                        if content.is_empty() {
                            self.entries.pop();
                        }
                    }
                }
                Action::StreamError
            }
        }
    }
}

impl Component for Transcript {
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
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

    fn render(&mut self, f: &mut ratatui::Frame, area: Rect) {
        render(area, f.buffer_mut(), &mut self.entries, &mut self.scroll);
    }
}
