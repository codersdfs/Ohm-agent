//! Loader information passed from app state to registered renderers.
//!
//! Renderers consume a [`LoaderInfo`] snapshot each frame; the renderer trait
//! is intentionally narrow (`tick` + `spans`) so any rendering style —
//! shimmer text, color-coded braille, indeterminate bar — can plug in
//! without modifying callers.

/// What the agent is doing right now. Determines both the activity phrase
/// vocabulary the renderer can draw from and any state-specific coloring.
///
/// This is the canonical enum used by all loader renderers and replaces the
/// `SpinnerState` values previously owned by `tui::spinner::OmegaSpinner`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpinnerState {
    Idle,
    Thinking,
    Streaming,
    ToolCall,
    Error,
}

/// Snapshot of everything a loader renderer might want to draw this frame.
///
/// Renderers may ignore fields they don't use. The struct is built once per
/// frame by `StatusState` and consumed by the active `LoaderRenderer`.
///
/// Phrase selection is intentionally *not* here: activity vocabulary is
/// presentation detail, so each renderer owns its own phrase bank (which
/// additionally allows user overrides via `LoaderConfig.phrases`).
#[derive(Clone, Debug)]
pub struct LoaderInfo {
    /// What the agent is doing — phrase vocab and accent color depend on it.
    pub state: SpinnerState,
    /// Name of the tool currently running, when state is `ToolCall`.
    /// A single name when one tool is running (multi-tool queues collapse
    /// to the most recently started).
    pub tool_name: Option<String>,
    /// Seconds elapsed since the current state began. Reset on every
    /// state transition by `StatusState`.
    pub elapsed_secs: u64,
    /// Tokens streamed out since the current response started. Reset
    /// alongside the elapsed timer on each new streaming response.
    pub tokens_out: u64,
    /// Global animation frame counter (monotonic). Renderers derive their
    /// sub-state animation cursor (shimmer position, braille frame) from this.
    pub tick: u64,
}

impl LoaderInfo {
    /// Empty snapshot used by Idle / Error frames. Cheap to construct.
    pub fn idle(tick: u64) -> Self {
        Self {
            state: SpinnerState::Idle,
            tool_name: None,
            elapsed_secs: 0,
            tokens_out: 0,
            tick,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_info_has_no_tool_or_metrics() {
        let info = LoaderInfo::idle(0);
        assert_eq!(info.state, SpinnerState::Idle);
        assert!(info.tool_name.is_none());
        assert_eq!(info.elapsed_secs, 0);
        assert_eq!(info.tokens_out, 0);
    }

    #[test]
    fn spinner_state_clone_copy_eq() {
        let a = SpinnerState::Streaming;
        let b = a;
        assert_eq!(a, b);
    }
}
