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
    /// Context fill percentage (0–100), None if unknown
    pub ctx_pct: Option<u8>,
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
            ctx_pct: None,
        }
    }

    /// Build the context-length gauge text and style.
    fn ctx_gauge(&self) -> (String, ratatui::style::Color) {
        let pct = match self.ctx_pct {
            Some(p) => p.min(100),
            None => return (String::new(), theme::DIM),
        };

        let color = if pct > 90 {
            theme::ERROR
        } else if pct > 70 {
            theme::WARN
        } else {
            theme::DIM
        };

        // Compact bar: 10 block characters
        let filled = ((pct as f32 / 100.0) * 10.0).round() as usize;
        let filled = filled.min(10);
        let empty = 10 - filled;

        let bar: String = std::iter::repeat('▓')
            .take(filled)
            .chain(std::iter::repeat('░').take(empty))
            .collect();

        (format!(" ctx {} {}% ", bar, pct), color)
    }
}

/// Truncate a string to `max` display chars, appending an ellipsis when shortened.
fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }
    let keep = max - 1;
    let mut out: String = s.chars().take(keep).collect();
    out.push('…');
    out
}

/// Truncate from the left (for paths), keeping the trailing portion.
fn truncate_left_ellipsis(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }
    let keep = max - 1;
    let skip = char_count.saturating_sub(keep);
    let mut out = String::from("…");
    out.extend(s.chars().skip(skip));
    out
}

/// Render the header — compact, single line with a thin rule beneath.
pub fn render(area: Rect, buf: &mut Buffer, state: &HeaderState) {
    if area.height < 1 || area.width < 4 {
        return;
    }

    // Truncate long values so the header stays stable under long model names.
    let model_max = if area.width > 70 { 28 } else { 18 };
    let provider_max = if area.width > 70 { 18 } else { 12 };
    let max_cwd = if area.width > 70 { 22 } else { 14 };

    let model_text = truncate_with_ellipsis(&state.model, model_max);
    let provider_text = truncate_with_ellipsis(&state.provider, provider_max);

    let label = format!(" omega v{} ", state.app_version);

    let left_spans = vec![
        Span::styled(
            "Ω ",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(label, theme::style_dim_bold()),
        Span::styled("· ", theme::style_dim()),
        Span::styled(model_text, theme::style_dim()),
        Span::styled(" · ", theme::style_dim()),
        Span::styled(provider_text, theme::style_dim()),
    ];

    // Context gauge (right-aligned)
    let (ctx_text, ctx_color) = state.ctx_gauge();
    let ctx_span = if !ctx_text.is_empty() {
        Some(Span::styled(ctx_text, Style::default().fg(ctx_color)))
    } else {
        None
    };

    let right_text = if area.width > 60 {
        format!(" {}", truncate_left_ellipsis(&state.cwd, max_cwd))
    } else {
        String::new()
    };
    let right = Span::styled(right_text, theme::style_dim());

    // Compute widths
    let left_width: u16 = left_spans.iter().map(|s| s.width() as u16).sum();
    let ctx_width = ctx_span.as_ref().map(|s| s.width() as u16).unwrap_or(0);
    let right_width = right.width() as u16;
    let fill_width = area
        .width
        .saturating_sub(left_width)
        .saturating_sub(if ctx_span.is_some() { ctx_width } else { 0 })
        .saturating_sub(right_width);

    let mut spans = left_spans;
    if fill_width > 0 {
        spans.push(Span::raw(" ".repeat(fill_width as usize)));
    }
    if let Some(ctx) = ctx_span {
        spans.push(ctx);
    }
    spans.push(right);

    let para = Paragraph::new(Line::from(spans)).block(
        Block::default().style(Style::default().bg(theme::BG)),
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
