//! Braille loader renderer — the refined classic.
//!
//! Modernisation of the original `OmegaSpinner`: same conventional braille
//! frames and rotating activity phrases, now with per-state accent colors
//! (thinking violet, streaming blue, tool calls amber) and the shared info
//! suffix (tool name, elapsed seconds, token count).

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::info::{LoaderInfo, SpinnerState};
use super::renderer::LoaderRenderer;
use super::shimmer::{STREAMING_PHRASES, THINKING_PHRASES, TOOL_PHRASES};
use super::suffix::state_accent;

const ERROR_PHRASE: &str = "Something went wrong";

/// Conventional terminal spinner frames.
const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Phrase rotation cadence in ticks (~2s at the default 80ms tick).
const PHRASE_TICKS: u64 = 24;

/// Refined braille spinner with state-colored glyphs and activity phrases.
pub struct BrailleRenderer {
    phrases_override: Option<Vec<String>>,
}

impl BrailleRenderer {
    pub fn new(phrases_override: Option<Vec<String>>) -> Self {
        Self { phrases_override }
    }

    fn bank(&self, state: SpinnerState) -> Vec<&'static str> {
        match state {
            SpinnerState::Thinking => THINKING_PHRASES.to_vec(),
            SpinnerState::Streaming => STREAMING_PHRASES.to_vec(),
            SpinnerState::ToolCall => TOOL_PHRASES.to_vec(),
            SpinnerState::Idle | SpinnerState::Error => vec![],
        }
    }

    fn current_phrase(&self, info: &LoaderInfo) -> String {
        match info.state {
            SpinnerState::Idle => String::new(),
            SpinnerState::Error => ERROR_PHRASE.to_string(),
            _ => {
                let idx = (info.tick as usize / PHRASE_TICKS as usize)
                    % self.bank(info.state).len().max(1);
                if let Some(ovr) = &self.phrases_override {
                    ovr[idx % ovr.len()].clone()
                } else {
                    let bank = self.bank(info.state);
                    bank[idx % bank.len()].to_string()
                }
            }
        }
    }
}

impl LoaderRenderer for BrailleRenderer {
    fn name(&self) -> &'static str {
        "braille"
    }

    fn tick(&mut self) {}

    fn spans(&self, info: &LoaderInfo) -> (Vec<Span<'static>>, u16) {
        if info.state == SpinnerState::Idle {
            return (Vec::new(), 0);
        }

        let accent = state_accent(info.state);
        let glyph_style = if info.state == SpinnerState::Error {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(accent)
        };

        let glyph = if info.state == SpinnerState::Error {
            '!'.to_string()
        } else {
            FRAMES[(info.tick as usize) % FRAMES.len()].to_string()
        };

        let mut spans = vec![Span::styled(glyph, glyph_style)];
        let phrase = self.current_phrase(info);
        if !phrase.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                phrase,
                Style::default().fg(theme_colors_for(info.state)),
            ));
        }

        let (mut suffix, w) = super::suffix::spans(info);
        let width: u16 = spans.iter().map(|s| s.width() as u16).sum::<u16>() + w;
        spans.append(&mut suffix);
        (spans, width)
    }
}

/// Phrase text color: slightly dimmer than the glyph accent for hierarchy.
fn theme_colors_for(state: SpinnerState) -> ratatui::style::Color {
    match state {
        SpinnerState::Error => crate::tui::theme::ERROR,
        _ => crate::tui::theme::PRIMARY_CONTAINER,
    }
}

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
    fn test_braille_idle_renders_nothing() {
        let r = BrailleRenderer::new(None);
        let (spans, w) = r.spans(&info(SpinnerState::Idle, 0));
        assert!(spans.is_empty());
        assert_eq!(w, 0);
    }

    #[test]
    fn test_braille_frames_cycle_in_order() {
        let r = BrailleRenderer::new(None);
        assert_eq!(r.spans(&info(SpinnerState::Thinking, 0)).0[0].content, "⠋");
        assert_eq!(r.spans(&info(SpinnerState::Thinking, 1)).0[0].content, "⠙");
        assert_eq!(r.spans(&info(SpinnerState::Streaming, 2)).0[0].content, "⠹");
    }

    #[test]
    fn test_braille_phrases_rotate_by_bank() {
        let r = BrailleRenderer::new(None);
        assert_eq!(
            r.current_phrase(&info(SpinnerState::ToolCall, 0)),
            TOOL_PHRASES[0]
        );
        assert_eq!(
            r.current_phrase(&info(SpinnerState::ToolCall, PHRASE_TICKS)),
            TOOL_PHRASES[1]
        );
    }

    #[test]
    fn test_braille_error_is_bold_bang() {
        let r = BrailleRenderer::new(None);
        let (spans, _) = r.spans(&info(SpinnerState::Error, 9));
        assert_eq!(spans[0].content, "!");
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_braille_suffix_appended_with_width() {
        let r = BrailleRenderer::new(None);
        let mut inf = info(SpinnerState::ToolCall, 30);
        inf.tool_name = Some("bash".into());
        inf.elapsed_secs = 5;
        let (spans, w) = r.spans(&inf);
        let joined: String = spans.iter().map(|s| s.content.clone()).collect();
        assert!(joined.contains("bash"));
        assert!(joined.contains("5s"));
        let actual: u16 = spans.iter().map(|s| s.width() as u16).sum();
        assert_eq!(actual, w);
    }
}
