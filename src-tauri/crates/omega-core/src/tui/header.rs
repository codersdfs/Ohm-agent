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

    fn ctx_gauge_color(&self) -> Option<(f32, ratatui::style::Color)> {
        let pct = match self.ctx_pct {
            Some(p) => p.min(100) as f32,
            None => return None,
        };
        let color = if pct > 90.0 {
            theme::ERROR
        } else if pct > 70.0 {
            theme::WARN
        } else {
            theme::DIM
        };
        Some((pct / 100.0, color))
    }
}

impl Widget for &HeaderState {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 20 {
            return;
        }

        let top_line_y = area.y;
        let bottom_line_y = area.y + 1;

        // ── Line 1: Ω label, model/provider, context gauge ──────────────
        let label = format!(" omega v{} ", self.app_version);

        let left_spans = vec![
            Span::styled("Ω ", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(label, theme::style_dim_bold()),
        ];

        // Right side: context gauge as a thin bar if available
        let right_gauge = self.ctx_gauge_color().map(|(pct, color)| {
            let bar_width = 10u16;
            let filled = (pct * bar_width as f32).round() as u16;
            let filled = filled.min(bar_width);
            let bar: String = std::iter::repeat('█')
                .take(filled as usize)
                .chain(std::iter::repeat('░').take((bar_width - filled) as usize))
                .collect();
            let pct_text = format!(" {:.0}% ", pct * 100.0);
            Span::styled(
                format!("{} {}", bar, pct_text),
                Style::default().fg(color),
            )
        });

        let left_width: u16 = left_spans.iter().map(|s| s.width() as u16).sum();
        let right_width = right_gauge.as_ref().map(|s| s.width() as u16).unwrap_or(0);
        let fill = area.width.saturating_sub(left_width).saturating_sub(right_width + 1);

        let mut line1_spans = left_spans;
        if fill > 0 {
            line1_spans.push(Span::raw(" ".repeat(fill as usize)));
        }
        if let Some(gauge) = right_gauge {
            line1_spans.push(gauge);
        }

        let line1 = Paragraph::new(Line::from(line1_spans))
            .block(Block::default().style(Style::default().bg(theme::BG)));
        line1.render(Rect::new(area.x, top_line_y, area.width, 1), buf);

        // ── Line 2: model · provider · cwd ─────────────────────────────
        let cwd_display = if self.cwd.len() > 35 {
            format!("…{}", &self.cwd[self.cwd.len().saturating_sub(34)..])
        } else {
            self.cwd.clone()
        };

        let line2_spans = vec![
            Span::styled("model: ", theme::style_dim()),
            Span::styled(&self.model, theme::style_dim()),
            Span::styled(" · provider: ", theme::style_dim()),
            Span::styled(&self.provider, theme::style_dim()),
            Span::styled(" · cwd: ", theme::style_dim()),
            Span::styled(cwd_display, theme::style_dim()),
        ];

        let line2 = Paragraph::new(Line::from(line2_spans))
            .block(Block::default().style(Style::default().bg(theme::BG)));
        line2.render(Rect::new(area.x, bottom_line_y, area.width, 1), buf);

        // ── Thin separator line below the two-line header ━━━━━━━━━━━━━
        let rule_y = area.y + 2;
        if rule_y < area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = theme::buf_cell_mut(buf, x, rule_y) {
                    cell.set_symbol("─");
                    cell.set_fg(theme::DIM);
                }
            }
        }
    }
}