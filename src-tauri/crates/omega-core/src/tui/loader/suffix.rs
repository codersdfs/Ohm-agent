//! Shared helpers for loader renderers: per-state accent colors and the
//! informational suffix (`tool name · elapsed · tokens`) both the shimmer
//! and braille renderers append to their glyph/phrase spans.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::super::theme;
use super::info::{LoaderInfo, SpinnerState};

/// Accent color for a spinner state. State differentiation is color-first:
/// violet while thinking, blue while streaming, amber during tool calls,
/// red on error.
pub fn state_accent(state: SpinnerState) -> ratatui::style::Color {
    match state {
        SpinnerState::Idle => theme::DIM,
        SpinnerState::Thinking => theme::SECONDARY,
        SpinnerState::Streaming => theme::PRIMARY,
        SpinnerState::ToolCall => theme::WARN,
        SpinnerState::Error => theme::ERROR,
    }
}

/// Build the dim informational suffix for active states:
///
/// - ToolCall: `· grep("pattern") · 12s`
/// - Streaming: `· 12s · ↑340 tok`
/// - Thinking: `· 12s`
///
/// Returns `(spans, total_width)`; empty vec for Idle/Error so dormant
/// states never leak metrics into a quiet status line.
pub fn spans(info: &LoaderInfo) -> (Vec<Span<'static>>, u16) {
    let mut out: Vec<Span<'static>> = Vec::new();
    if matches!(info.state, SpinnerState::Idle | SpinnerState::Error) {
        return (out, 0);
    }

    if info.state == SpinnerState::ToolCall {
        if let Some(name) = &info.tool_name {
            out.push(Span::raw(" · "));
            out.push(Span::styled(
                name.clone(),
                Style::default().fg(theme::PRIMARY_CONTAINER),
            ));
        }
    }

    if info.elapsed_secs > 0 {
        out.push(Span::raw(" · "));
        out.push(Span::styled(
            format!("{}s", info.elapsed_secs),
            Style::default().fg(theme::DIM),
        ));
    }

    if info.state == SpinnerState::Streaming && info.tokens_out > 0 {
        out.push(Span::raw(" · "));
        out.push(Span::styled(
            format!("↑{} tok", info.tokens_out),
            Style::default()
                .fg(theme::DIM)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    let w = out.iter().map(|s| s.width() as u16).sum();
    (out, w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_accent_distinct_per_active_state() {
        let t = state_accent(SpinnerState::Thinking);
        let s = state_accent(SpinnerState::Streaming);
        let c = state_accent(SpinnerState::ToolCall);
        assert_ne!(t, s);
        assert_ne!(s, c);
        assert_ne!(t, c);
    }

    #[test]
    fn test_suffix_empty_when_idle_or_error() {
        for st in [SpinnerState::Idle, SpinnerState::Error] {
            let mut inf = LoaderInfo::idle(0);
            inf.state = st;
            inf.elapsed_secs = 9;
            inf.tool_name = Some("bash".into());
            inf.tokens_out = 12;
            let (spans, w) = spans(&inf);
            assert!(spans.is_empty());
            assert_eq!(w, 0);
        }
    }

    #[test]
    fn test_suffix_thinking_shows_only_elapsed() {
        let mut inf = LoaderInfo::idle(0);
        inf.state = SpinnerState::Thinking;
        inf.elapsed_secs = 7;
        let (spans, _) = spans(&inf);
        let text: String = spans.iter().map(|s| s.content.clone()).collect();
        assert_eq!(text, " · 7s");
    }

    #[test]
    fn test_suffix_toolcall_shows_name_and_elapsed() {
        let mut inf = LoaderInfo::idle(0);
        inf.state = SpinnerState::ToolCall;
        inf.tool_name = Some("grep".into());
        inf.elapsed_secs = 3;
        let (spans, _) = spans(&inf);
        let text: String = spans.iter().map(|s| s.content.clone()).collect();
        assert_eq!(text, " · grep · 3s");
    }

    #[test]
    fn test_suffix_streaming_shows_elapsed_and_tokens() {
        let mut inf = LoaderInfo::idle(0);
        inf.state = SpinnerState::Streaming;
        inf.elapsed_secs = 2;
        inf.tokens_out = 340;
        let (spans, _) = spans(&inf);
        let text: String = spans.iter().map(|s| s.content.clone()).collect();
        assert_eq!(text, " · 2s · ↑340 tok");
    }

    #[test]
    fn test_suffix_zero_metrics_render_nothing() {
        let mut inf = LoaderInfo::idle(0);
        inf.state = SpinnerState::Streaming;
        let (spans, w) = spans(&inf);
        assert!(spans.is_empty());
        assert_eq!(w, 0);
    }
}
