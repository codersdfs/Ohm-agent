use ratatui::style::{Modifier, Style};
use super::theme;

/// Cooking/action phrases that rotate for the status line — Claude Code inspired.
const COOKING_PHRASES: &[&str] = &[
    "cooking…",
    "brewing…",
    "simmering…",
    "seasoning…",
    "tasting…",
    "plating…",
    "stirring the pot…",
    "fetching ingredients…",
    "kneading…",
    "grinding…",
    "marinating…",
    "braising…",
    "roasting…",
    "whisking…",
    "sifting…",
    "glazing…",
    "steaming…",
    "infusing…",
    "caramelizing…",
    "tempering…",
];

/// Error / problem phrases
const BURNT_PHRASES: &[&str] = &[
    "burnt toast…",
    "spilled the pot…",
    "overcooked…",
    "broke a yolk…",
    "burned the garlic…",
    "curdled…",
    "too salty…",
    "fell on the floor…",
];

/// Ω spinner animation frames — the Omega symbol itself rotates through
/// Unicode variations to create a unique spinning-Ω effect.
const OMEGA_SPINNER_FRAMES: &[char] = &['Ω', '⍥', '⍟', '⍤', '⍥', '⍟'];

/// Agent state for the spinner.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SpinnerState {
    Idle,
    Thinking,
    Streaming,
    ToolCall,
    Error,
}

/// The Omega spinner — a unique Ω-centric loading animation with cooking metaphors.
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
        // Rotate phrase every 12 ticks (~1 second at 80ms tick rate)
        if self.tick % 12 == 0 {
            match self.state {
                SpinnerState::Thinking | SpinnerState::Streaming | SpinnerState::ToolCall => {
                    self.phrase_idx = (self.phrase_idx + 1) % COOKING_PHRASES.len();
                }
                SpinnerState::Error => {
                    self.phrase_idx = (self.phrase_idx + 1) % BURNT_PHRASES.len();
                }
                SpinnerState::Idle => {
                    self.phrase_idx = 0;
                }
            }
        }
    }

    /// Get the current Ω glyph based on state and tick.
    pub fn current_glyph(&self) -> char {
        let frame = (self.tick / 3) as usize % OMEGA_SPINNER_FRAMES.len();
        match self.state {
            SpinnerState::Idle => 'Ω',
            SpinnerState::Thinking | SpinnerState::Streaming | SpinnerState::ToolCall => {
                OMEGA_SPINNER_FRAMES[frame]
            }
            SpinnerState::Error => '⍝',
        }
    }

    /// Get the current action/phrase text.
    pub fn current_phrase(&self) -> &'static str {
        match self.state {
            SpinnerState::Idle => "",
            SpinnerState::Thinking | SpinnerState::Streaming | SpinnerState::ToolCall => {
                COOKING_PHRASES[self.phrase_idx % COOKING_PHRASES.len()]
            }
            SpinnerState::Error => {
                BURNT_PHRASES[self.phrase_idx % BURNT_PHRASES.len()]
            }
        }
    }

    /// Get the accent style modifier for the current state.
    pub fn glyph_style(&self) -> Style {
        let base = Style::default().fg(theme::ACCENT);
        match self.state {
            SpinnerState::Idle => base.add_modifier(Modifier::DIM),
            SpinnerState::Thinking => base.add_modifier(
                if (self.tick / 4) % 2 == 0 { Modifier::BOLD } else { Modifier::empty() }
            ),
            SpinnerState::Streaming => base.add_modifier(
                if (self.tick / 3) % 2 == 0 { Modifier::BOLD } else { Modifier::RAPID_BLINK }
            ),
            SpinnerState::ToolCall => base.add_modifier(
                if (self.tick / 5) % 2 == 0 { Modifier::BOLD } else { Modifier::empty() }
            ),
            SpinnerState::Error => Style::default().fg(theme::ERROR).add_modifier(Modifier::BOLD),
        }
    }
}

impl Default for OmegaSpinner {
    fn default() -> Self {
        Self::new()
    }
}

/// A compact Ω glyph renderer for inline use (status bar).
pub struct CompactOmega {
    pub spinner: OmegaSpinner,
}

impl CompactOmega {
    pub fn new(spinner: OmegaSpinner) -> Self {
        Self { spinner }
    }

    /// Render the compact Ω and phrase into a string for the status bar.
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