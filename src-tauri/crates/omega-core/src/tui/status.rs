use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::theme;
use super::spinner::{OmegaSpinner, SpinnerState};

/// Status line state — what to show in the single-line footer.
pub struct StatusState {
    pub mode: String,
    pub spinner: OmegaSpinner,
    pub hint_text: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub messages_count: u64,
    /// Estimated tokens during streaming. Kept for compatibility; the token
    /// display intentionally ignores estimates and only shows provider usage.
    pub streaming_estimate: u64,
}

impl Default for StatusState {
    fn default() -> Self {
        Self {
            mode: "chat".into(),
            spinner: OmegaSpinner::new(),
            hint_text: None,
            tokens_in: 0,
            tokens_out: 0,
            messages_count: 0,
            streaming_estimate: 0,
        }
    }
}

impl StatusState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the spinner state from external state (streaming, thinking, etc.)
    pub fn set_spinner_state(&mut self, state: SpinnerState) {
        self.spinner.state = state;
    }

    /// Tick the spinner animation.
    pub fn tick_spinner(&mut self) {
        self.spinner.tick();
    }

    /// Compact token count: raw, k, or M.
    pub fn format_tokens(count: u64) -> String {
        if count >= 1_000_000 {
            let m = count as f64 / 1_000_000.0;
            if m.fract().abs() < f64::EPSILON {
                format!("{:.0}M", m)
            } else {
                format!("{:.1}M", m)
            }
        } else if count >= 1_000 {
            let k = count as f64 / 1_000.0;
            if k.fract().abs() < f64::EPSILON {
                format!("{:.0}k", k)
            } else {
                format!("{:.1}k", k)
            }
        } else {
            count.to_string()
        }
    }

    /// Real session usage: `input:↓1.2k  output:↑340`
    pub fn format_token_usage(tokens_in: u64, tokens_out: u64) -> String {
        format!(
            "input:↓{}  output:↑{}",
            Self::format_tokens(tokens_in),
            Self::format_tokens(tokens_out),
        )
    }
}  // end impl StatusState

impl Widget for &StatusState {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 {
            return;
        }

        let mut spans: Vec<Span<'static>> = Vec::new();

        // Left: copyright + mode
        let left = vec![
            Span::styled(" © OMEGA_ORCH ", theme::style_dim()),
            Span::styled("MODE: INTERACTIVE_REPL ", theme::style_dim()),
        ];
        let left_w: u16 = left.iter().map(|s| s.width() as u16).sum();

        // Right: real input/output token counts (no fake latency / estimates)
        let tok_str = StatusState::format_token_usage(self.tokens_in, self.tokens_out);

        let right_spans = vec![
            Span::styled(format!(" {} ", tok_str), Style::default().fg(theme::SECONDARY)),
        ];
        let right_w: u16 = right_spans.iter().map(|s| s.width() as u16).sum();

        // Fill
        let fill = area.width.saturating_sub(left_w).saturating_sub(right_w);

        spans.extend(left);
        if fill > 0 { spans.push(Span::raw(" ".repeat(fill as usize))); }
        spans.extend(right_spans);

        let para = Paragraph::new(Line::from(spans))
            .style(Style::default().bg(theme::BG));
        para.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_uses_raw_k_and_m() {
        assert_eq!(StatusState::format_tokens(42), "42");
        assert_eq!(StatusState::format_tokens(1_200), "1.2k");
        assert_eq!(StatusState::format_tokens(12_000), "12k");
        assert_eq!(StatusState::format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn format_token_usage_uses_input_output_arrows() {
        assert_eq!(
            StatusState::format_token_usage(1_200, 340),
            "input:↓1.2k  output:↑340"
        );
    }
}