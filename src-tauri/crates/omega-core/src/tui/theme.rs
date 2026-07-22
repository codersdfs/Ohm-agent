use ratatui::style::{Color, Modifier, Style};

/// Central style palette for the Omega TUI.
/// Dark charcoal surfaces and neutral greys keep the interface quiet and readable.

// ── Core palette ─────────────────────────────────────────────────────────────
// DESIGN.md tokens mapped to ratatui Color::Rgb

// Inherit the user's terminal background instead of painting a fixed RGB canvas.
pub const BG: Color = Color::Reset;
pub const FG: Color = Color::Rgb(234, 234, 236);
pub const SURFACE: Color = Color::Rgb(40, 40, 43);
pub const SURFACE_HIGH: Color = Color::Rgb(54, 54, 58);
pub const SURFACE_LOW: Color = Color::Rgb(32, 32, 35);
pub const RECESSED: Color = Color::Rgb(20, 20, 22);
pub const OUTLINE: Color = Color::Rgb(94, 94, 101);
pub const OUTLINE_VARIANT: Color = Color::Rgb(162, 162, 170);
pub const PRIMARY: Color = Color::Rgb(198, 198, 205);
pub const PRIMARY_CONTAINER: Color = Color::Rgb(242, 242, 244);
pub const ACCENT: Color = PRIMARY; // backward compat alias
pub const SECONDARY: Color = Color::Rgb(214, 214, 220);
pub const DIM: Color = Color::Rgb(174, 174, 181);
pub const ERROR: Color = Color::Rgb(230, 142, 142);
pub const SUCCESS: Color = Color::Rgb(145, 194, 158);
pub const WARN: Color = Color::Rgb(210, 177, 116);

// ── Layout / component colors ────────────────────────────────────────────────

pub const SIDEBAR_BG: Color = Color::Rgb(28, 28, 31);
pub const SIDEBAR_ACTIVE: Color = Color::Rgb(48, 48, 52);
pub const SIDEBAR_FG: Color = Color::Rgb(172, 172, 180);
pub const SIDEBAR_ACCENT: Color = PRIMARY_CONTAINER;
pub const BORDER: Color = OUTLINE;
pub const RULE: Color = Color::Rgb(68, 68, 73);
pub const GLASS_BORDER: Color = OUTLINE;
pub const GLASS_BORDER_ACTIVE: Color = PRIMARY;

// ── Diff colors ──────────────────────────────────────────────────────────────

pub const DIFF_ADD: Color = Color::Rgb(130, 184, 143);
pub const DIFF_REMOVE: Color = Color::Rgb(214, 132, 132);
pub const DIFF_HEADER: Color = PRIMARY;

// ── Role marker colors ───────────────────────────────────────────────────────

pub const USER_MARKER: Color = PRIMARY;
pub const AGENT_MARKER: Color = Color::Rgb(140, 140, 148);
pub const TOOL_MARKER: Color = WARN;
pub const SYSTEM_MARKER: Color = OUTLINE;

// ── Tool-specific colors ──────────────────────────────────────────────────────

pub const TOOL_BROWSER: Color = Color::Rgb(180, 180, 188);
pub const TOOL_SHELL: Color = Color::Rgb(156, 184, 163);
pub const TOOL_FILE_SYS: Color = Color::Rgb(194, 174, 137);
pub const TOOL_SEARCH: Color = Color::Rgb(174, 174, 184);
pub const TOOL_READ: Color = PRIMARY;
pub const TOOL_WRITE: Color = TOOL_SHELL;
pub const TOOL_EDIT: Color = TOOL_FILE_SYS;
pub const TOOL_BASH: Color = TOOL_SHELL;
pub const TOOL_GLOB: Color = TOOL_SEARCH;
pub const TOOL_GREP: Color = TOOL_SEARCH;
pub const TOOL_TASK: Color = Color::Rgb(174, 174, 184);
pub const TOOL_WEB: Color = PRIMARY;
pub const TOOL_LSP: Color = TOOL_SHELL;
pub const TOOL_DEFAULT: Color = TOOL_FILE_SYS;

// ── Editor border state colors ───────────────────────────────────────────────

pub const EDITOR_IDLE: Color = OUTLINE;
pub const EDITOR_THINKING: Color = SECONDARY;
pub const EDITOR_STREAMING: Color = PRIMARY;
pub const EDITOR_ERROR: Color = ERROR;
pub const EDITOR_CONFIRM: Color = WARN;

// ── Tool box colors ────────────────────────────────────────────

pub const TOOL_BOX_BORDER: Color = PRIMARY;
pub const TOOL_BOX_BORDER_RUNNING: Color = WARN;

// ── Provider panel focus colors ──────────────────────────────────────────────

/// Subtle neutral fill for focused elements.
pub const FOCUS_GLOW: Color = Color::Rgb(64, 64, 69);
/// Color for focused text field underlines.
pub const FOCUS_UNDERLINE: Color = Color::Rgb(198, 198, 205);
/// Accent color for focused container borders.
pub const FOCUS_BORDER: Color = Color::Rgb(224, 224, 229);

pub fn style_tool_box_border() -> Style {
    Style::default().fg(TOOL_BOX_BORDER)
}

pub fn style_tool_box_title() -> Style {
    Style::default()
        .fg(TOOL_BOX_BORDER)
        .add_modifier(Modifier::BOLD)
}

pub fn style_tool_box_ok() -> Style {
    Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD)
}

pub fn style_tool_box_err() -> Style {
    Style::default().fg(ERROR).add_modifier(Modifier::BOLD)
}

// ── Convenience styles ───────────────────────────────────────────────────────

pub fn style_text() -> Style {
    Style::default().fg(FG)
}

pub fn style_dim() -> Style {
    Style::default().fg(DIM)
}

pub fn style_accent() -> Style {
    Style::default()
        .fg(PRIMARY_CONTAINER)
        .add_modifier(Modifier::BOLD)
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

pub fn style_primary() -> Style {
    Style::default().fg(PRIMARY)
}

pub fn style_secondary() -> Style {
    Style::default().fg(SECONDARY)
}

// ── Provider panel focus helper styles ───────────────────────────────────────

/// Style for a focused input field: brighter foreground and an underline effect.
pub fn style_focused_field() -> Style {
    Style::default().fg(FG).add_modifier(Modifier::UNDERLINED)
}

/// Style for a focused button: bold and high-contrast without animation.
pub fn style_focused_button() -> Style {
    Style::default()
        .fg(PRIMARY_CONTAINER)
        .add_modifier(Modifier::BOLD)
}

/// Get the color associated with a tool name.
pub fn tool_color(name: &str) -> Color {
    match name {
        "read" | "view" | "cat" | "browser" | "web_fetch" => TOOL_READ,
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

/// Render a subtle raised border using two neighbouring grey tones.
/// Returns styles for the four borders.
pub fn glass_border_styles(active: bool) -> (Style, Style, Style, Style) {
    let top_left = if active {
        Style::default().fg(PRIMARY_CONTAINER)
    } else {
        Style::default().fg(Color::Rgb(104, 104, 111))
    };
    let bottom_right = if active {
        Style::default().fg(PRIMARY)
    } else {
        Style::default().fg(Color::Rgb(62, 62, 67))
    };
    (top_left, top_left, bottom_right, bottom_right)
}

/// Safe cell accessor for ratatui 0.26 (which has no `cell_mut`).
/// Returns `None` when `(x, y)` is outside the buffer area.
pub fn buf_cell_mut(
    buf: &mut ratatui::buffer::Buffer,
    x: u16,
    y: u16,
) -> Option<&mut ratatui::buffer::Cell> {
    if x < buf.area.width && y < buf.area.height {
        Some(buf.get_mut(x, y))
    } else {
        None
    }
}
