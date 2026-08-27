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
    /// Permission request received - show dialog.
    PermissionRequest {
        prompt: String,
        options: Vec<String>,
        default_idx: usize,
    },
    /// User response to a permission request (true = allow, false = deny).
    PermissionResponse(bool),
    /// Permission dialog was cancelled (e.g., ESC).
    PermissionCancel,
}

/// Actions that cross component boundaries.
/// Returned by `handle_key` methods and used for cross-component communication.
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
    /// Permission request received - show dialog.
    PermissionRequest {
        prompt: String,
        options: Vec<String>,
        default_idx: usize,
    },
    /// User selected permission option (index of choice).
    PermissionResponse(usize),
    /// Permission dialog was cancelled (e.g., ESC).
    PermissionCancel,
}
