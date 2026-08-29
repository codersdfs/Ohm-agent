use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::loader::{LoaderInfo, LoaderRegistry, SpinnerState};
use super::theme;

/// Status line state — what to show in the single-line footer.
///
/// Owns the pluggable [`LoaderRegistry`] plus everything renderers need:
/// current agent state, elapsed-since-transition timer, tool name during
/// tool calls, and the live streamed-token counter.
pub struct StatusState {
    pub mode: String,
    pub loader: LoaderRegistry,
    /// Agent activity state driving the loader.
    state: SpinnerState,
    /// When the current state began (elapsed display resets on transitions).
    state_started: Instant,
    /// Most recent tool name while in `ToolCall`.
    tool_name: Option<String>,
    /// Tokens streamed out during the current turn.
    turn_tokens_out: u64,
    /// Monotonic loader animation frame (incremented per UI tick).
    anim_tick: u64,
    pub hint_text: Option<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub messages_count: u64,
}

impl Default for StatusState {
    fn default() -> Self {
        Self {
            mode: "chat".into(),
            loader: LoaderRegistry::default_registry(),
            state: SpinnerState::Idle,
            state_started: Instant::now(),
            tool_name: None,
            turn_tokens_out: 0,
            anim_tick: 0,
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

    /// Set the loader state from external events (streaming, thinking, etc.).
    ///
    /// Every transition restarts the elapsed timer. Entering `Thinking`
    /// starts a new turn: the streamed-token counter resets.
    pub fn set_spinner_state(&mut self, state: SpinnerState) {
        if self.state != state {
            if state == SpinnerState::Thinking {
                self.turn_tokens_out = 0;
            }
            if state != SpinnerState::ToolCall {
                // Leaving a tool (or entering non-tool states) clears the chip;
                // ToolCall transitions refresh/keep the name via set_tool_name.
                if state == SpinnerState::Idle || state == SpinnerState::Error {
                    self.tool_name = None;
                }
            }
            self.state = state;
            self.state_started = Instant::now();
        }
    }

    /// Record streamed output tokens for the live `↑N tok` counter.
    pub fn record_tokens_out(&mut self, n: u64) {
        self.turn_tokens_out = self.turn_tokens_out.saturating_add(n);
    }

    /// Promote Idle to Thinking as a fallback while a turn is active but
    /// before any streaming event lands. Existing explicit states are kept.
    pub fn ensure_active(&mut self) {
        if self.state == SpinnerState::Idle {
            self.set_spinner_state(SpinnerState::Thinking);
        }
    }

    /// Set the tool name shown during `ToolCall` (e.g. "grep", "bash").
    pub fn set_tool_name(&mut self, name: Option<String>) {
        self.tool_name = name;
    }

    /// Advance loader animation bookkeeping.
    pub fn tick_spinner(&mut self) {
        self.anim_tick = self.anim_tick.wrapping_add(1);
    }

    /// Immutable snapshot for renderer consumption this frame.
    fn snapshot(&self) -> LoaderInfo {
        LoaderInfo {
            state: self.state,
            tool_name: self.tool_name.clone(),
            elapsed_secs: self.state_started.elapsed().as_secs(),
            tokens_out: self.turn_tokens_out,
            tick: self.anim_tick,
        }
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

    /// Compact header usage: `12.3k in / 4.5k out` (flat codex-alike header).
    /// One decimal only when needed; integral counts stay raw below 1000.
    pub fn format_usage_compact(tokens_in: u64, tokens_out: u64) -> String {
        format!(
            "{} in / {} out",
            Self::format_tokens(tokens_in),
            Self::format_tokens(tokens_out)
        )
    }
} // end impl StatusState

impl Widget for &StatusState {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 {
            return;
        }

        // Left: active loader renderer output (glyph + shimmer phrase + suffix).
        let (left, _loader_w) = self.loader.spans(&self.snapshot());
        let left_w: u16 = left.iter().map(|s| s.width() as u16).sum();

        // Right: real input/output token counts
        let tok_str = StatusState::format_token_usage(self.tokens_in, self.tokens_out);

        let right_spans = vec![Span::styled(
            format!(" {} ", tok_str),
            Style::default().fg(theme::SECONDARY),
        )];
        let right_w: u16 = right_spans.iter().map(|s| s.width() as u16).sum();

        // Fill
        let fill = area.width.saturating_sub(left_w).saturating_sub(right_w);

        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.extend(left);
        if fill > 0 {
            spans.push(Span::raw(" ".repeat(fill as usize)));
        }
        spans.extend(right_spans);

        let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(theme::BG));
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

    #[test]
    fn format_usage_compact_matches_header_format() {
        assert_eq!(
            StatusState::format_usage_compact(12_300, 4_500),
            "12.3k in / 4.5k out"
        );
        assert_eq!(StatusState::format_usage_compact(0, 0), "0 in / 0 out");
        assert_eq!(
            StatusState::format_usage_compact(340, 42),
            "340 in / 42 out"
        );
    }

    #[test]
    fn test_status_state_transition_resets_elapsed_timer() {
        let st = StatusState::new();
        // Simulate an old transition instant by poking via state changes:
        // entering Thinking starts a fresh timer.
        std::thread::sleep(std::time::Duration::from_millis(15));
        let mut st = st;
        st.set_spinner_state(SpinnerState::Thinking);
        let snap = st.snapshot();
        assert_eq!(snap.state, SpinnerState::Thinking);
        assert!(snap.elapsed_secs <= 1);
    }

    #[test]
    fn test_status_state_new_turn_resets_turn_tokens() {
        let mut st = StatusState::new();
        st.set_spinner_state(SpinnerState::Streaming);
        st.record_tokens_out(100);
        st.record_tokens_out(40);
        // Thinking marks the start of a fresh turn.
        st.set_spinner_state(SpinnerState::Thinking);
        assert_eq!(st.turn_tokens_out, 0);
        st.record_tokens_out(5);
        assert_eq!(st.turn_tokens_out, 5);
    }

    #[test]
    fn test_status_state_same_state_keeps_timer_and_tokens() {
        let mut st = StatusState::new();
        st.set_spinner_state(SpinnerState::Streaming);
        st.record_tokens_out(9);
        // Duplicate event re-setting Streaming must not reset the counter.
        st.set_spinner_state(SpinnerState::Streaming);
        st.record_tokens_out(1);
        assert_eq!(st.turn_tokens_out, 10);
    }

    #[test]
    fn test_status_state_idle_clears_tool_name() {
        let mut st = StatusState::new();
        st.set_tool_name(Some("grep".into()));
        st.set_spinner_state(SpinnerState::ToolCall);
        st.set_spinner_state(SpinnerState::Idle);
        assert!(st.tool_name.is_none());
    }

    #[test]
    fn test_status_state_tick_advances_anim_counter() {
        let mut st = StatusState::new();
        assert_eq!(st.anim_tick, 0);
        st.tick_spinner();
        st.tick_spinner();
        assert_eq!(st.anim_tick, 2);
    }
}
