use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::theme;

/// Status line state — what to show in the single-line footer.
pub struct StatusState {
    pub mode: String,
    pub spinner: Option<String>,
    pub action_text: String,
    pub hint_text: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub messages_count: u64,
}

impl Default for StatusState {
    fn default() -> Self {
        Self {
            mode: "chat".into(),
            spinner: None,
            action_text: String::new(),
            hint_text: None,
            tokens_in: 0,
            tokens_out: 0,
            messages_count: 0,
        }
    }
}

impl StatusState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Render the status line — a single line under the editor.
///
/// Layout (left to right):
///   [spinner] [action]  ...padding...  [messages] [tokens]
pub fn render(area: Rect, buf: &mut Buffer, state: &StatusState) {
    if area.width < 4 {
        return;
    }

    let mut spans: Vec<Span<'static>> = Vec::new();

    // Left side: spinner + action
    if let Some(ref spinner) = state.spinner {
        spans.push(Span::styled(
            format!(" {} ", spinner),
            Style::default().fg(theme::ACCENT),
        ));
    }

    if !state.action_text.is_empty() {
        spans.push(Span::styled(
            format!("{} ", state.action_text),
            theme::style_dim(),
        ));
    }

    // Show keybinding hints when idle
    if spans.is_empty() {
        if let Some(ref hint) = state.hint_text {
            spans.push(Span::styled(
                format!(" {} ", hint),
                Style::default().fg(theme::DIM),
            ));
        } else {
            spans.push(Span::styled(
                " · ",
                Style::default().fg(theme::DIM),
            ));
        }
    }

    // Right side: tokens and message count
    let right_parts = if state.tokens_in > 0 || state.tokens_out > 0 {
        format!(
            " {} msg · {} in / {} out ",
            state.messages_count, state.tokens_in, state.tokens_out
        )
    } else if state.messages_count > 0 {
        format!(" {} msgs ", state.messages_count)
    } else {
        String::new()
    };

    let right_width = right_parts.len() as u16;
    let left_width: u16 = spans.iter().map(|s| s.width() as u16).sum();
    let fill = area.width.saturating_sub(left_width).saturating_sub(right_width);

    if fill > 0 {
        spans.push(Span::raw(" ".repeat(fill as usize)));
    }
    spans.push(Span::styled(right_parts, theme::style_dim()));

    let para = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme::BG));
    para.render(area, buf);
}
