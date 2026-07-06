use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use super::theme;

/// Data displayed in the header bar.
pub struct HeaderState {
    pub app_version: String,
    pub model: String,
    pub provider: String,
    pub cwd: String,
}

impl HeaderState {
    pub fn new(model: String, provider: String) -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            model,
            provider,
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "?".into()),
        }
    }
}

/// Render the header — compact, single line with a thin rule beneath.
pub fn render(area: Rect, buf: &mut Buffer, state: &HeaderState) {
    if area.height < 1 || area.width < 4 {
        return;
    }

    let label = format!(" omega v{} ", state.app_version);

    let left = Line::from(vec![
        Span::styled("Ω ", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(label, theme::style_dim_bold()),
        Span::styled("· ", theme::style_dim()),
        Span::styled(&state.model, theme::style_dim()),
        Span::styled(" · ", theme::style_dim()),
        Span::styled(&state.provider, theme::style_dim()),
    ]);

    let right_text = if area.width > 60 {
        let cwd = if state.cwd.len() > 30 {
            format!("…{}", &state.cwd[state.cwd.len().saturating_sub(29)..])
        } else {
            state.cwd.clone()
        };
        format!(" {}", cwd)
    } else {
        String::new()
    };
    let right = Span::styled(right_text, theme::style_dim());

    // Build the line with left aligned and right aligned text
    let left_width = left.width() as u16;
    let right_width = right.width() as u16;
    let fill_width = area.width.saturating_sub(left_width).saturating_sub(right_width);

    let spans = if fill_width > 0 {
        let mut parts = left.spans;
        parts.push(Span::raw(" ".repeat(fill_width as usize)));
        parts.push(right);
        parts
    } else {
        left.spans
    };

    let para = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .style(Style::default().bg(theme::BG)),
    );
    para.render(area, buf);

    // Thin rule beneath the header
    if area.height >= 2 {
        let rule_y = area.y + 1;
        let rule_color = theme::DIM;
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, rule_y)) {
                cell.set_symbol("─");
                cell.set_fg(rule_color);
            }
        }
    }
}
