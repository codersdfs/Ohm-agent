use ratatui::style::{Modifier, Style};
use super::theme;

/// Short, Claude Code-inspired activity words. The phrase changes slowly so
/// it adds personality without making the status line visually noisy.
const THINKING_PHRASES: &[&str] = &[
    "Cooking…",
    "Pondering…",
    "Reasoning…",
    "Planning…",
    "Considering…",
];

const STREAMING_PHRASES: &[&str] = &[
    "Writing…",
    "Composing…",
    "Shaping…",
];

const TOOL_PHRASES: &[&str] = &[
    "Working…",
    "Inspecting…",
    "Gathering…",
];

/// Conventional terminal spinner frames.
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Agent state for the spinner.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpinnerState {
    Idle,
    Thinking,
    Streaming,
    ToolCall,
    Error,
}

/// Shared activity spinner for the TUI.
pub struct OmegaSpinner {
    /// Current state of the spinner
    pub state: SpinnerState,
    /// Animation tick (incremented each frame)
    pub tick: u64,
    /// Current cooking phrase index
    pub phrase_idx: usize,
}

impl OmegaSpinner {
    pub fn new() -> Self {
        Self {
            state: SpinnerState::Idle,
            tick: 0,
            phrase_idx: 0,
        }
    }

    /// Tick the spinner — advance the animation frame.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        // Change the phrase about every two seconds at the 80ms tick rate.
        if self.tick % 24 == 0 {
            let phrase_count = match self.state {
                SpinnerState::Thinking => THINKING_PHRASES.len(),
                SpinnerState::Streaming => STREAMING_PHRASES.len(),
                SpinnerState::ToolCall => TOOL_PHRASES.len(),
                SpinnerState::Idle | SpinnerState::Error => 1,
            };
            self.phrase_idx = (self.phrase_idx + 1) % phrase_count;
        }
    }

    /// Get the current conventional spinner glyph.
    pub fn current_glyph(&self) -> char {
        match self.state {
            SpinnerState::Idle => ' ',
            SpinnerState::Thinking | SpinnerState::Streaming | SpinnerState::ToolCall => {
                SPINNER_FRAMES[self.tick as usize % SPINNER_FRAMES.len()]
            }
            SpinnerState::Error => '!',
        }
    }

    /// Get the current action/phrase text.
    pub fn current_phrase(&self) -> &'static str {
        match self.state {
            SpinnerState::Idle => "",
            SpinnerState::Thinking => THINKING_PHRASES[self.phrase_idx % THINKING_PHRASES.len()],
            SpinnerState::Streaming => STREAMING_PHRASES[self.phrase_idx % STREAMING_PHRASES.len()],
            SpinnerState::ToolCall => TOOL_PHRASES[self.phrase_idx % TOOL_PHRASES.len()],
            SpinnerState::Error => "Something went wrong",
        }
    }

    /// Get the accent style modifier for the current state.
    pub fn glyph_style(&self) -> Style {
        match self.state {
            SpinnerState::Idle => Style::default().fg(theme::DIM),
            SpinnerState::Thinking | SpinnerState::Streaming | SpinnerState::ToolCall => {
                Style::default().fg(theme::PRIMARY_CONTAINER)
            }
            SpinnerState::Error => Style::default().fg(theme::ERROR).add_modifier(Modifier::BOLD),
        }
    }
}

impl Default for OmegaSpinner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_states_use_a_normal_spinner() {
        let mut spinner = OmegaSpinner::new();
        spinner.state = SpinnerState::Thinking;
        assert_eq!(spinner.current_glyph(), '⠋');
        spinner.tick();
        assert_eq!(spinner.current_glyph(), '⠙');
        assert!(SPINNER_FRAMES.contains(&spinner.current_glyph()));
    }

    #[test]
    fn thinking_uses_activity_words_instead_of_thinking_label() {
        let mut spinner = OmegaSpinner::new();
        spinner.state = SpinnerState::Thinking;
        assert_eq!(spinner.current_phrase(), "Cooking…");
        for _ in 0..24 { spinner.tick(); }
        assert_eq!(spinner.current_phrase(), "Pondering…");
        assert!(!spinner.current_phrase().to_lowercase().contains("thinking"));
    }

    #[test]
    fn state_changes_use_distinct_activity_vocabulary() {
        let mut spinner = OmegaSpinner::new();
        spinner.state = SpinnerState::Streaming;
        assert_eq!(spinner.current_phrase(), "Writing…");
        spinner.state = SpinnerState::ToolCall;
        assert_eq!(spinner.current_phrase(), "Working…");
        spinner.state = SpinnerState::Error;
        assert_eq!(spinner.current_glyph(), '!');
    }
}

/// A compact spinner renderer for inline use.
pub struct CompactOmega {
    pub spinner: OmegaSpinner,
}

impl CompactOmega {
    pub fn new(spinner: OmegaSpinner) -> Self {
        Self { spinner }
    }

    /// Render the spinner and activity phrase into a string.
    pub fn render_inline(&self) -> (String, Style) {
        let glyph = self.spinner.current_glyph();
        let phrase = self.spinner.current_phrase();
        let style = self.spinner.glyph_style();
        let text = if phrase.is_empty() {
            format!(" {}", glyph)
        } else {
            format!(" {} {}", glyph, phrase)
        };
        (text, style)
    }
}