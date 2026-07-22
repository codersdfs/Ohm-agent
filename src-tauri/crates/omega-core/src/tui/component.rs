use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Events sent from the streaming task to the UI event loop.
#[derive(Debug, Clone)]
pub enum UiStreamEvent {
    Token(String),
    Thinking(String),
    ThinkingDone,
    ToolCall {
        name: String,
        args: String,
    },
    ToolResult {
        name: String,
        success: bool,
        output: String,
    },
    Done {
        full: String,
        tokens_in: u32,
        tokens_out: u32,
        messages: Vec<providers::ChatMessage>,
    },
    Error(String),
}

/// Actions that cross component boundaries.
/// Returned by `Component::handle_key` and `Component::update`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Nothing to do.
    Noop,
    /// Exit the application.
    Quit,
    /// Submit the editor buffer as a new user message.
    SendMessage,
    /// Cancel the current streaming response.
    CancelStream,
    /// Toggle the help overlay.
    ToggleHelp,
    /// Toggle the provider configuration panel.
    ToggleProviderPanel,
    /// Apply the provider config from the panel.
    ProviderApply,
    /// Close the provider panel without applying.
    ProviderClose,
    /// Clear the conversation transcript.
    ClearConversation,
    /// Scroll the transcript up by N lines.
    ScrollUp(u16),
    /// Scroll the transcript down by N lines.
    ScrollDown(u16),
    /// Scroll to top.
    ScrollTop,
    /// Scroll to bottom.
    ScrollBottom,
    /// Streaming completed successfully.
    StreamDone { tokens_in: u32, tokens_out: u32 },
    /// Streaming finished with an error.
    StreamError,
}

/// A self-contained piece of UI that owns its state, handles events,
/// and renders itself into a given area of the frame.
pub trait Component {
    /// React to a key event. Default returns `Action::Noop`.
    fn handle_key(&mut self, key: KeyEvent) -> Action {
        let _ = key;
        Action::Noop
    }

    /// React to an action from a sibling or parent. Default returns `Action::Noop`.
    fn update(&mut self, action: &Action) -> Action {
        let _ = action;
        Action::Noop
    }

    /// Draw the component into the given rect.
    fn render(&mut self, f: &mut Frame, area: Rect);
}
