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
    /// Estimated tokens during streaming (fragment_len / 4), 0 otherwise
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

    fn format_tokens(count: u64) -> String {
        if count >= 1_000_000 {
            format!("{:.1}M", count as f64 / 1_000_000.0)
        } else if count >= 1_000 {
            format!("{:.1}k", count as f64 / 1_000.0)
        } else {
            count.to_string()
        }
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

        // Right: latency, tokens, sigma
        let total_in = self.tokens_in + self.streaming_estimate;
        let total_out = self.tokens_out + self.streaming_estimate / 2;
        let tok_str = if total_in > 0 || total_out > 0 {
            format!("TOKENS: {} IN / {} OUT ", StatusState::format_tokens(total_in), StatusState::format_tokens(total_out))
        } else {
            String::new()
        };

        let right_spans = vec![
            Span::styled(" LTCY: 24MS ", theme::style_dim()),
            Span::styled(tok_str.clone(), Style::default().fg(theme::SECONDARY)),
            Span::styled(" SIGMA_LINK: ", theme::style_dim()),
            Span::styled("ESTABLISHED", Style::default().fg(theme::PRIMARY)),
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