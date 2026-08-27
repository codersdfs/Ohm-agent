//! Shimmer loader renderer — the default.
//!
//! A color-coded braille glyph leads, the activity phrase shimmers with a
//! comet-like highlight sweeping left-to-right, and an informational suffix
//! (tool name, elapsed seconds, streamed token count) renders in dim style.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::super::theme;
use super::info::{LoaderInfo, SpinnerState};
use super::renderer::LoaderRenderer;
use super::suffix::state_accent;

/// Shimmer highlight trail length (chars lit behind the head).
const TRAIL: usize = 2;

/// Default activity phrases when no override is configured.
pub const THINKING_PHRASES: &[&str] = &[
    "Cooking…",
    "Pondering…",
    "Reasoning…",
    "Planning…",
    "Considering…",
];

pub const STREAMING_PHRASES: &[&str] = &["Writing…", "Composing…", "Shaping…"];

pub const TOOL_PHRASES: &[&str] = &["Working…", "Inspecting…", "Gathering…"];

const ERROR_PHRASE: &str = "Something went wrong";

/// Chars between sweep passes — a short pause at full base style.
const SWEEP_GAP: usize = 6;

/// The default loader: shimmering phrase + lead glyph + info suffix.
pub struct ShimmerRenderer {
    /// Phrase bank override from config, else empty to use defaults.
    phrases_override: Option<Vec<String>>,
}

impl ShimmerRenderer {
    pub fn new(phrases_override: Option<Vec<String>>) -> Self {
        Self { phrases_override }
    }

    /// Rotate phrase every ~2s at 80ms ticks. Inactive states show nothing
    /// (`Idle`) or a fixed error string (`Error`).
    fn current_phrase(&self, info: &LoaderInfo) -> String {
        match info.state {
            SpinnerState::Idle => String::new(),
            SpinnerState::Error => ERROR_PHRASE.to_string(),
            SpinnerState::Thinking | SpinnerState::Streaming | SpinnerState::ToolCall => {
                let bank: Vec<&str> = self.bank(info.state);
                let idx = (info.tick as usize / 24) % bank.len();
                // Override applies only while it still describes this state;
                // overrides replace the whole rotating vocabulary.
                if let Some(ovr) = &self.phrases_override {
                    ovr[idx % ovr.len()].clone()
                } else {
                    bank[idx].to_string()
                }
            }
        }
    }

    fn bank(&self, state: SpinnerState) -> Vec<&'static str> {
        match state {
            SpinnerState::Thinking => THINKING_PHRASES.to_vec(),
            SpinnerState::Streaming => STREAMING_PHRASES.to_vec(),
            SpinnerState::ToolCall => TOOL_PHRASES.to_vec(),
            SpinnerState::Idle | SpinnerState::Error => vec![],
        }
    }
}

impl LoaderRenderer for ShimmerRenderer {
    fn name(&self) -> &'static str {
        "shimmer"
    }

    fn tick(&mut self) {}

    fn spans(&self, info: &LoaderInfo) -> (Vec<Span<'static>>, u16) {
        let accent = state_accent(info.state);
        let mut spans: Vec<Span<'static>> = Vec::new();
        if info.state == SpinnerState::Idle {
            return (spans, 0);
        }

        // Lead glyph: braille spinner cycles; Error is a bold '!'.
        let glyph_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
        if info.state == SpinnerState::Error {
            spans.push(Span::styled("!".to_string(), glyph_style));
        } else {
            let frame = BRAILLE_FRAME[(info.tick as usize) % BRAILLE_FRAME.len()];
            spans.push(Span::styled(frame.to_string(), glyph_style));
        }
        spans.push(Span::raw(" "));

        // Phrase with sweeping shimmer highlight.
        let phrase = self.current_phrase(info);
        if !phrase.is_empty() {
            let chars: Vec<char> = phrase.chars().collect();
            let len = chars.len();
            let cycle = len + SWEEP_GAP;
            let pos = (info.tick as usize) % cycle;
            for (i, ch) in chars.iter().enumerate() {
                let dist = pos.abs_diff(i);
                let style = if dist == 0 {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else if dist <= TRAIL {
                    Style::default().fg(theme::PRIMARY_CONTAINER)
                } else {
                    Style::default().fg(theme::DIM)
                };
                spans.push(Span::styled(ch.to_string(), style));
            }
        }

        let (mut suffix, w) = super::suffix::spans(info);
        let width: u16 = spans.iter().map(|s| s.width() as u16).sum::<u16>() + w;
        spans.append(&mut suffix);
        (spans, width)
    }
}

/// Conventional braille spinner frames shared by renderers.
const BRAILLE_FRAME: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

#[cfg(test)]
mod tests {
    use super::*;

    fn info(state: SpinnerState, tick: u64) -> LoaderInfo {
        LoaderInfo {
            state,
            tool_name: None,
            elapsed_secs: 0,
            tokens_out: 0,
            tick,
        }
    }

    #[test]
    fn test_shimmer_idle_renders_nothing() {
        let r = ShimmerRenderer::new(None);
        let (spans, w) = r.spans(&info(SpinnerState::Idle, 0));
        assert!(spans.is_empty());
        assert_eq!(w, 0);
    }

    #[test]
    fn test_shimmer_thinking_leads_with_braille_glyph() {
        let r = ShimmerRenderer::new(None);
        let (spans, _) = r.spans(&info(SpinnerState::Thinking, 0));
        assert_eq!(spans[0].content, "⠋");
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_shimmer_phrase_rotates_every_24_ticks() {
        let r = ShimmerRenderer::new(None);
        let early = r.current_phrase(&info(SpinnerState::Thinking, 0));
        let later = r.current_phrase(&info(SpinnerState::Thinking, 48));
        assert_eq!(early, THINKING_PHRASES[0]);
        assert_eq!(later, THINKING_PHRASES[2]);
    }

    #[test]
    fn test_shimmer_override_phrases_take_precedence() {
        let r = ShimmerRenderer::new(Some(vec!["Hacking…".to_string()]));
        assert_eq!(
            r.current_phrase(&info(SpinnerState::Thinking, 0)),
            "Hacking…"
        );
        assert_eq!(
            r.current_phrase(&info(SpinnerState::ToolCall, 500)),
            "Hacking…"
        );
    }

    #[test]
    fn test_shimmer_error_is_bold_exclamation() {
        let r = ShimmerRenderer::new(None);
        let (spans, _) = r.spans(&info(SpinnerState::Error, 3));
        assert_eq!(spans[0].content, "!");
        assert_eq!(
            r.current_phrase(&info(SpinnerState::Error, 0)),
            ERROR_PHRASE
        );
    }

    #[test]
    fn test_shimmer_width_matches_span_widths() {
        let r = ShimmerRenderer::new(None);
        let mut inf = info(SpinnerState::ToolCall, 5);
        inf.tool_name = Some("grep".to_string());
        inf.elapsed_secs = 4;
        let (spans, w) = r.spans(&inf);
        let actual: u16 = spans.iter().map(|s| s.width() as u16).sum();
        assert_eq!(actual, w);
    }
}
