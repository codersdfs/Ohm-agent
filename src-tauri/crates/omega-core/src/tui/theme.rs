use ratatui::style::{Color, Modifier, Style};

/// Central style palette for the Omega TUI.
/// Minimal, restrained — every color earns its place.

// ── Core palette ─────────────────────────────────────────────────────────────

pub const BG: Color = Color::Reset;
pub const FG: Color = Color::White;
pub const TEXT: Color = Color::White;
pub const DIM: Color = Color::DarkGray;
pub const ACCENT: Color = Color::Cyan;
pub const ERROR: Color = Color::Red;
pub const SUCCESS: Color = Color::Green;
pub const WARN: Color = Color::Yellow;

// ── Diff colors ──────────────────────────────────────────────────────────────

pub const DIFF_ADD: Color = Color::Green;
pub const DIFF_REMOVE: Color = Color::Red;
pub const DIFF_HEADER: Color = Color::Cyan;

// ── Role marker colors ───────────────────────────────────────────────────────

pub const USER_MARKER: Color = Color::Green;
pub const AGENT_MARKER: Color = Color::Cyan;
pub const TOOL_MARKER: Color = Color::Yellow;
pub const SYSTEM_MARKER: Color = Color::DarkGray;

// ── Tool-specific colors ──────────────────────────────────────────────────────

pub const TOOL_READ: Color = Color::Blue;
pub const TOOL_WRITE: Color = Color::Green;
pub const TOOL_EDIT: Color = Color::Yellow;
pub const TOOL_BASH: Color = Color::Magenta;
pub const TOOL_GLOB: Color = Color::Cyan;
pub const TOOL_GREP: Color = Color::Cyan;
pub const TOOL_TASK: Color = Color::Magenta;
pub const TOOL_WEB: Color = Color::Blue;
pub const TOOL_LSP: Color = Color::Green;
pub const TOOL_DEFAULT: Color = Color::Yellow;

// ── Editor border state colors ───────────────────────────────────────────────

pub const EDITOR_IDLE: Color = Color::DarkGray;
pub const EDITOR_THINKING: Color = Color::Yellow;
pub const EDITOR_STREAMING: Color = Color::Cyan;
pub const EDITOR_ERROR: Color = Color::Red;
pub const EDITOR_CONFIRM: Color = Color::Yellow;

// ── Tool box colors ────────────────────────────────────────────

pub const TOOL_BOX_BORDER: Color = Color::Yellow;

pub fn style_tool_box_border() -> Style {
    Style::default().fg(TOOL_BOX_BORDER)
}

pub fn style_tool_box_title() -> Style {
    Style::default().fg(TOOL_BOX_BORDER).add_modifier(Modifier::BOLD)
}

pub fn style_tool_box_ok() -> Style {
    Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD)
}

pub fn style_tool_box_err() -> Style {
    Style::default().fg(ERROR).add_modifier(Modifier::BOLD)
}

// ── Convenience styles ───────────────────────────────────────────────────────

pub fn style_text() -> Style {
    Style::default().fg(TEXT)
}

pub fn style_dim() -> Style {
    Style::default().fg(DIM)
}

pub fn style_accent() -> Style {
    Style::default().fg(ACCENT)
}

pub fn style_error() -> Style {
    Style::default().fg(ERROR)
}

pub fn style_success() -> Style {
    Style::default().fg(SUCCESS)
}

pub fn style_bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn style_dim_bold() -> Style {
    Style::default().fg(DIM).add_modifier(Modifier::BOLD)
}

/// Get the color associated with a tool name.
pub fn tool_color(name: &str) -> Color {
    match name {
        "read" | "view" | "cat" => TOOL_READ,
        "write" | "create" => TOOL_WRITE,
        "edit" | "patch" | "str_replace" => TOOL_EDIT,
        "bash" | "shell" | "command" | "run" => TOOL_BASH,
        "glob" | "ls" | "list" => TOOL_GLOB,
        "grep" | "search" | "rg" => TOOL_GREP,
        "task" | "agent" => TOOL_TASK,
        "web" | "fetch" | "browse" => TOOL_WEB,
        "lsp" | "goto" | "references" => TOOL_LSP,
        _ => TOOL_DEFAULT,
    }
}

/// Safe cell accessor for ratatui 0.26 (which has no `cell_mut`).
/// Returns `None` when `(x, y)` is outside the buffer area.
pub fn buf_cell_mut(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16) -> Option<&mut ratatui::buffer::Cell> {
    if x < buf.area.width && y < buf.area.height {
        Some(buf.get_mut(x, y))
    } else {
        None
    }
}
