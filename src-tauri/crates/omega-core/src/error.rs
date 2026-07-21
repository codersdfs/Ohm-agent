//! Typed errors for the Omega agent.
//!
//! The goal is to replace ad-hoc `String` error propagation with a structured
//! enum the UI can render by variant (distinct colors / chips) and the runtime
//! can branch on for recovery (retry vs abort).
//!
//! This module is deliberately dependency-free (only `ratatui::style::Color`
//! for the chip helpers) so it can be referenced from both `omega-core` and
//! `omega-cli` without pulling extra crates.

use ratatui::style::{Color, Modifier, Style};

// ─── Tool errors ─────────────────────────────────────────────────────────────

/// Classification of a tool execution failure. Drives the chip label/color
/// and the recoverability decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolErrorKind {
    /// File/path not found, or glob returned nothing.
    NotFound,
    /// Permission denied (OS or sandbox).
    PermissionDenied,
    /// Command exceeded the deadline.
    Timeout,
    /// Output could not be parsed into the expected shape.
    ParseFailed,
    /// The tool ran but reported a logical failure (non-zero exit, rejected edit).
    ExecutionFailed,
    /// User or orchestrator aborted the tool mid-run.
    Aborted,
}

impl ToolErrorKind {
    pub fn is_recoverable(&self) -> bool {
        matches!(self,
            ToolErrorKind::Timeout | ToolErrorKind::ExecutionFailed | ToolErrorKind::ParseFailed
        )
    }

    /// Short uppercase chip label for the redesigned tool bar.
    pub fn chip_label(&self) -> &'static str {
        match self {
            ToolErrorKind::NotFound          => "NOT_FOUND",
            ToolErrorKind::PermissionDenied  => "PERM",
            ToolErrorKind::Timeout           => "TIMEOUT",
            ToolErrorKind::ParseFailed       => "PARSE",
            ToolErrorKind::ExecutionFailed   => "FAIL",
            ToolErrorKind::Aborted           => "ABORT",
        }
    }

    /// Chip color on the cyber-noir palette.
    pub fn chip_color(&self) -> Color {
        match self {
            ToolErrorKind::NotFound          => Color::Rgb(186, 201, 204), // dim
            ToolErrorKind::PermissionDenied  => Color::Rgb(255, 180, 171), // error red
            ToolErrorKind::Timeout           => Color::Rgb(255, 190, 70),  // warn amber
            ToolErrorKind::ParseFailed       => Color::Rgb(0, 218, 243),   // cyan
            ToolErrorKind::ExecutionFailed   => Color::Rgb(255, 180, 171), // error red
            ToolErrorKind::Aborted           => Color::Rgb(186, 201, 204), // dim
        }
    }

    /// 1-glyph icon shown before the tool name in the bar.
    pub fn icon(&self) -> &'static str {
        match self {
            ToolErrorKind::NotFound          => "?",
            ToolErrorKind::PermissionDenied  => "⊘",
            ToolErrorKind::Timeout           => "⏱",
            ToolErrorKind::ParseFailed       => "ﯦ",
            ToolErrorKind::ExecutionFailed   => "✗",
            ToolErrorKind::Aborted           => "⏹",
        }
    }
}

/// A structured tool call error. Carries enough context for the bar to render
/// a focused one-liner and for retry logic to decide what to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallError {
    pub tool: String,
    pub kind: ToolErrorKind,
    /// Human-readable message (first line shown in the bar, full in expanded).
    pub message: String,
    /// True if retrying the same call is reasonable.
    pub recoverable: bool,
}

impl ToolCallError {
    pub fn new(tool: impl Into<String>, kind: ToolErrorKind, message: impl Into<String>) -> Self {
        let recoverable = kind.is_recoverable();
        Self { tool: tool.into(), kind, message: message.into(), recoverable }
    }

    /// Stable prefix used when the error is flattened into a String for the
    /// legacy `UiStreamEvent::Error(String)` path. Renderers parse this prefix
    /// back into a typed view without losing information.
    pub fn prefix(&self) -> &'static str {
        match self.kind {
            ToolErrorKind::NotFound          => "TOOL_NOT_FOUND",
            ToolErrorKind::PermissionDenied  => "TOOL_PERM",
            ToolErrorKind::Timeout           => "TOOL_TIMEOUT",
            ToolErrorKind::ParseFailed       => "TOOL_PARSE",
            ToolErrorKind::ExecutionFailed   => "TOOL_FAIL",
            ToolErrorKind::Aborted           => "TOOL_ABORT",
        }
    }

    /// Render to a single-line string suitable for logging or legacy channels.
    pub fn to_flat_string(&self) -> String {
        format!("{}:{}:{}", self.prefix(), self.tool, self.message)
    }

    /// Try to recover a typed error from a flat `[VARIANT:tool:msg]`/`ERROR:…`
    /// string. Returns `None` if the prefix is unrecognized.
    pub fn from_flat_string(s: &str) -> Option<ToolCallError> {
        let (prefix, rest) = s.split_once(':')?;
        let (tool, message) = match rest.split_once(':') {
            Some((t, m)) => (t.to_string(), m.to_string()),
            None => (String::new(), rest.to_string()),
        };
        let kind = match prefix {
            "TOOL_NOT_FOUND" => ToolErrorKind::NotFound,
            "TOOL_PERM"      => ToolErrorKind::PermissionDenied,
            "TOOL_TIMEOUT"   => ToolErrorKind::Timeout,
            "TOOL_PARSE"     => ToolErrorKind::ParseFailed,
            "TOOL_FAIL"      => ToolErrorKind::ExecutionFailed,
            "TOOL_ABORT"     => ToolErrorKind::Aborted,
            _ => return None,
        };
        Some(ToolCallError::new(tool, kind, message))
    }

    pub fn style(&self) -> Style {
        Style::default().fg(self.kind.chip_color()).add_modifier(Modifier::BOLD)
    }
}

// ─── Agent-wide errors ───────────────────────────────────────────────────────

/// Classification of provider/network/API level failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderErrorKind {
    /// 401/403 — bad or missing API key.
    Auth,
    /// 429 — rate limited.
    RateLimited,
    /// 4xx other than auth/rate.
    BadRequest,
    /// 5xx — server fault.
    ServerError,
    /// No response / connection refused / DNS.
    Network,
    /// Response body could not be decoded as a stream event.
    Decode,
}

impl ProviderErrorKind {
    pub fn is_recoverable(&self) -> bool {
        matches!(self, ProviderErrorKind::RateLimited | ProviderErrorKind::ServerError | ProviderErrorKind::Network)
    }

    pub fn chip_label(&self) -> &'static str {
        match self {
            ProviderErrorKind::Auth         => "AUTH",
            ProviderErrorKind::RateLimited  => "RATE",
            ProviderErrorKind::BadRequest   => "4xx",
            ProviderErrorKind::ServerError  => "5xx",
            ProviderErrorKind::Network      => "NET",
            ProviderErrorKind::Decode       => "DECODE",
        }
    }

    pub fn chip_color(&self) -> Color {
        match self {
            ProviderErrorKind::Auth         => Color::Rgb(255, 180, 171), // error red
            ProviderErrorKind::RateLimited  => Color::Rgb(255, 190, 70),   // warn amber
            ProviderErrorKind::BadRequest   => Color::Rgb(255, 180, 171), // error red
            ProviderErrorKind::ServerError  => Color::Rgb(255, 190, 70),   // warn amber
            ProviderErrorKind::Network      => Color::Rgb(0, 218, 243),    // cyan
            ProviderErrorKind::Decode       => Color::Rgb(0, 218, 243),    // cyan
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ProviderErrorKind::Auth         => "⊘",
            ProviderErrorKind::RateLimited  => "⚠",
            ProviderErrorKind::BadRequest   => "✗",
            ProviderErrorKind::ServerError  => "⚠",
            ProviderErrorKind::Network      => "↻",
            ProviderErrorKind::Decode       => "ﯦ",
        }
    }
}

/// Top-level typed error for the agent. One variant per failure origin so the
/// UI can render distinct chips and the runtime can decide retry/abort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    Provider { kind: ProviderErrorKind, message: String },
    Network(String),
    Tool(ToolCallError),
    Stream(String),
    Config(String),
    Cancelled,
    Io(String),
    Unknown(String),
}

impl AgentError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            AgentError::Provider { kind, .. } => kind.is_recoverable(),
            AgentError::Network(_)          => true,
            AgentError::Tool(t)              => t.recoverable,
            AgentError::Stream(_)            => false,
            AgentError::Config(_)            => false,
            AgentError::Cancelled            => false, // not an error to recover from
            AgentError::Io(_)                => true,
            AgentError::Unknown(_)           => false,
        }
    }

    /// Should this be displayed to the user as an error (red), or as a quiet
    /// notice (dim)? Cancelled is user intent — render dim, not red.
    pub fn is_quiet(&self) -> bool {
        matches!(self, AgentError::Cancelled)
    }

    /// Stable prefix for the flat-string channel.
    pub fn prefix(&self) -> &'static str {
        match self {
            AgentError::Provider { .. } => "PROVIDER",
            AgentError::Network(_)      => "NETWORK",
            AgentError::Tool(_)         => "TOOL",
            AgentError::Stream(_)       => "STREAM",
            AgentError::Config(_)       => "CONFIG",
            AgentError::Cancelled       => "CANCELLED",
            AgentError::Io(_)           => "IO",
            AgentError::Unknown(_)      => "UNKNOWN",
        }
    }

    pub fn to_flat_string(&self) -> String {
        match self {
            AgentError::Provider { kind, message } =>
                format!("PROVIDER:{}:{}", kind.chip_label(), message),
            AgentError::Tool(t)      => format!("TOOL:{}", t.to_flat_string()),
            AgentError::Network(m)   => format!("NETWORK:{}", m),
            AgentError::Stream(m)    => format!("STREAM:{}", m),
            AgentError::Config(m)    => format!("CONFIG:{}", m),
            AgentError::Cancelled    => "CANCELLED".to_string(),
            AgentError::Io(m)        => format!("IO:{}", m),
            AgentError::Unknown(m)   => format!("UNKNOWN:{}", m),
        }
    }

    /// Parse a flat string (emitted on the legacy `Error(String)` channel)
    /// back into a typed `AgentError`. Falls back to `Unknown` for anything
    /// unrecognized so the pipeline never drops an error on the floor.
    pub fn from_flat_string(s: &str) -> AgentError {
        let Some((prefix, rest)) = s.split_once(':') else {
            return AgentError::Unknown(s.to_string());
        };
        match prefix {
            "PROVIDER" => {
                let (kind_label, msg) = rest.split_once(':')
                    .map(|(k, m)| (k, m.to_string()))
                    .unwrap_or(("", rest.to_string()));
                let kind = match kind_label {
                    "AUTH"  => ProviderErrorKind::Auth,
                    "RATE"  => ProviderErrorKind::RateLimited,
                    "4xx"   => ProviderErrorKind::BadRequest,
                    "5xx"   => ProviderErrorKind::ServerError,
                    "NET"   => ProviderErrorKind::Network,
                    "DECODE"=> ProviderErrorKind::Decode,
                    _       => ProviderErrorKind::BadRequest,
                };
                AgentError::Provider { kind, message: msg }
            }
            "NETWORK"  => AgentError::Network(rest.to_string()),
            "TOOL"     => ToolCallError::from_flat_string(rest)
                .map(AgentError::Tool)
                .unwrap_or_else(|| AgentError::Unknown(s.to_string())),
            "STREAM"   => AgentError::Stream(rest.to_string()),
            "CONFIG"   => AgentError::Config(rest.to_string()),
            "CANCELLED" => AgentError::Cancelled,
            "IO"       => AgentError::Io(rest.to_string()),
            "UNKNOWN"  => AgentError::Unknown(rest.to_string()),
            _          => AgentError::Unknown(s.to_string()),
        }
    }

    /// Favor one chip label/color for the redesigned bar / notice.
    pub fn chip_label(&self) -> &'static str {
        match self {
            AgentError::Provider { kind, .. } => kind.chip_label(),
            AgentError::Network(_)      => "NET",
            AgentError::Tool(t)         => t.kind.chip_label(),
            AgentError::Stream(_)       => "STREAM",
            AgentError::Config(_)       => "CONFIG",
            AgentError::Cancelled       => "STOP",
            AgentError::Io(_)           => "IO",
            AgentError::Unknown(_)       => "ERR",
        }
    }

    pub fn chip_color(&self) -> Color {
        match self {
            AgentError::Provider { kind, .. } => kind.chip_color(),
            AgentError::Network(_)      => Color::Rgb(0, 218, 243),
            AgentError::Tool(t)         => t.kind.chip_color(),
            AgentError::Stream(_)       => Color::Rgb(0, 218, 243),
            AgentError::Config(_)       => Color::Rgb(255, 180, 171),
            AgentError::Cancelled       => Color::Rgb(186, 201, 204),
            AgentError::Io(_)           => Color::Rgb(255, 190, 70),
            AgentError::Unknown(_)      => Color::Rgb(255, 180, 171),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            AgentError::Provider { kind, .. } => kind.icon(),
            AgentError::Network(_)      => "↻",
            AgentError::Tool(t)         => t.kind.icon(),
            AgentError::Stream(_)       => "ﯦ",
            AgentError::Config(_)      => "⊘",
            AgentError::Cancelled       => "⏹",
            AgentError::Io(_)           => "⏱",
            AgentError::Unknown(_)      => "✗",
        }
    }

    pub fn message(&self) -> String {
        match self {
            AgentError::Provider { message, .. } => message.clone(),
            AgentError::Network(m)   => m.clone(),
            AgentError::Tool(t)      => t.message.clone(),
            AgentError::Stream(m)    => m.clone(),
            AgentError::Config(m)    => m.clone(),
            AgentError::Cancelled    => "Cancelled by user".to_string(),
            AgentError::Io(m)        => m.clone(),
            AgentError::Unknown(m)    => m.clone(),
        }
    }

    pub fn style(&self) -> Style {
        if self.is_quiet() {
            Style::default().fg(self.chip_color())
        } else {
            Style::default().fg(self.chip_color()).add_modifier(Modifier::BOLD)
        }
    }

    /// If this error wraps a `Tool`, return it by value. Used by the transcript
    /// renderer to attach typed errors to a `ToolCallState` when the error
    /// arrives on the legacy flat-string channel.
    pub fn typed_tool_error(&self) -> Option<ToolCallError> {
        if let AgentError::Tool(t) = self { Some(t.clone()) } else { None }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_roundtrip_tool_error() {
        let e = ToolCallError::new("read", ToolErrorKind::NotFound, "src/main.rs");
        let flat = e.to_flat_string();
        let back = ToolCallError::from_flat_string(&flat).unwrap();
        assert_eq!(back.tool, "read");
        assert_eq!(back.kind, ToolErrorKind::NotFound);
        assert_eq!(back.message, "src/main.rs");
    }

    #[test]
    fn flat_roundtrip_agent_error_tool() {
        let e = AgentError::Tool(ToolCallError::new("bash", ToolErrorKind::Timeout, "timed out"));
        let flat = e.to_flat_string();
        let back = AgentError::from_flat_string(&flat);
        match back {
            AgentError::Tool(t) => {
                assert_eq!(t.tool, "bash");
                assert_eq!(t.kind, ToolErrorKind::Timeout);
            }
            other => panic!("expected Tool, got {:?}", other),
        }
    }

    #[test]
    fn flat_roundtrip_agent_error_provider() {
        let e = AgentError::Provider { kind: ProviderErrorKind::RateLimited, message: "slow down".into() };
        let flat = e.to_flat_string();
        let back = AgentError::from_flat_string(&flat);
        match back {
            AgentError::Provider { kind, message } => {
                assert_eq!(kind, ProviderErrorKind::RateLimited);
                assert_eq!(message, "slow down");
            }
            other => panic!("expected Provider, got {:?}", other),
        }
    }

    #[test]
    fn cancelled_is_quiet_and_not_recoverable() {
        assert!(AgentError::Cancelled.is_quiet());
        assert!(!AgentError::Cancelled.is_recoverable());
    }

    #[test]
    fn unknown_prefix_falls_back() {
        let back = AgentError::from_flat_string("BOGUS:whatever");
        assert!(matches!(back, AgentError::Unknown(_)));
    }

    #[test]
    fn recoverability_matrix() {
        assert!(ToolErrorKind::Timeout.is_recoverable());
        assert!(!ToolErrorKind::NotFound.is_recoverable());
        assert!(ProviderErrorKind::RateLimited.is_recoverable());
        assert!(!ProviderErrorKind::Auth.is_recoverable());
    }
}