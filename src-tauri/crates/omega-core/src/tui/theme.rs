use ratatui::style::{Color, Modifier, Style};

/// Central style palette for the Omega TUI — cyber-noir / neon glassmorphism.
/// Anchored in abyssal obsidian (#050505) with Omega Cyan (#00daf3) and
/// Soft Violet (#d0bcff) accents. Glass panels via surface ladder.

// ── Core palette ─────────────────────────────────────────────────────────────
// DESIGN.md tokens mapped to ratatui Color::Rgb

pub const BG: Color = Color::Rgb(5, 5, 5);              // Space Black #050505
pub const FG: Color = Color::Rgb(229, 226, 225);        // on-surface #e5e2e1
pub const SURFACE: Color = Color::Rgb(26, 26, 26);      // #1a1a1a (glass panels)
pub const SURFACE_HIGH: Color = Color::Rgb(38, 38, 38); // #262626 (modals)
pub const SURFACE_LOW: Color = Color::Rgb(18, 18, 18);  // #121212
pub const RECESSED: Color = Color::Rgb(10, 10, 10);     // #0a0a0a (input fields)
pub const OUTLINE: Color = Color::Rgb(59, 73, 76);      // #3b494c
pub const OUTLINE_VARIANT: Color = Color::Rgb(132, 147, 150); // on-surface-variant
pub const PRIMARY: Color = Color::Rgb(0, 218, 243);     // Omega Cyan #00daf3
pub const PRIMARY_CONTAINER: Color = Color::Rgb(0, 229, 255); // #00e5ff (brighter)
pub const ACCENT: Color = PRIMARY; // backward compat alias
pub const SECONDARY: Color = Color::Rgb(208, 188, 255); // Soft Violet #d0bcff
pub const DIM: Color = Color::Rgb(186, 201, 204);       // on-surface-variant #bac9cc
pub const ERROR: Color = Color::Rgb(255, 180, 171);     // #ffb4ab
pub const SUCCESS: Color = Color::Rgb(0, 229, 255);     // cyan success
pub const WARN: Color = Color::Rgb(255, 190, 70);       // warm amber

// ── Layout / component colors ────────────────────────────────────────────────

pub const SIDEBAR_BG: Color = Color::Rgb(8, 8, 14);     // very dark sidebar
pub const SIDEBAR_ACTIVE: Color = Color::Rgb(20, 20, 28); // active row bg
pub const SIDEBAR_FG: Color = Color::Rgb(150, 150, 165);
pub const SIDEBAR_ACCENT: Color = PRIMARY_CONTAINER;
pub const BORDER: Color = OUTLINE;
pub const RULE: Color = Color::Rgb(42, 42, 50);
pub const GLASS_BORDER: Color = Color::Rgb(255, 255, 255); // for 0.1 alpha simulation → use OUTLINE instead
pub const GLASS_BORDER_ACTIVE: Color = PRIMARY;

// ── Diff colors ──────────────────────────────────────────────────────────────

pub const DIFF_ADD: Color = Color::Rgb(0, 200, 80);
pub const DIFF_REMOVE: Color = Color::Rgb(255, 90, 90);
pub const DIFF_HEADER: Color = PRIMARY;

// ── Role marker colors ───────────────────────────────────────────────────────

pub const USER_MARKER: Color = PRIMARY;                  // cyan
pub const AGENT_MARKER: Color = Color::Rgb(140, 140, 155);
pub const TOOL_MARKER: Color = Color::Rgb(255, 190, 70);
pub const SYSTEM_MARKER: Color = OUTLINE;

// ── Tool-specific colors ──────────────────────────────────────────────────────

pub const TOOL_BROWSER: Color = PRIMARY;
pub const TOOL_SHELL: Color = Color::Rgb(74, 222, 128);  // green
pub const TOOL_FILE_SYS: Color = Color::Rgb(250, 204, 21); // amber
pub const TOOL_SEARCH: Color = Color::Rgb(232, 121, 249); // magenta
pub const TOOL_READ: Color = PRIMARY;
pub const TOOL_WRITE: Color = TOOL_SHELL;
pub const TOOL_EDIT: Color = TOOL_FILE_SYS;
pub const TOOL_BASH: Color = TOOL_SHELL;
pub const TOOL_GLOB: Color = TOOL_SEARCH;
pub const TOOL_GREP: Color = TOOL_SEARCH;
pub const TOOL_TASK: Color = Color::Rgb(232, 121, 249);
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

/// Subtle glow effect for focused elements — a softer cyan for backgrounds.
pub const FOCUS_GLOW: Color = Color::Rgb(0, 40, 50);
/// Color for focused text field underlines.
pub const FOCUS_UNDERLINE: Color = Color::Rgb(0, 218, 243);
/// Accent color for focused container borders.
pub const FOCUS_BORDER: Color = Color::Rgb(0, 229, 255);

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
    Style::default().fg(FG)
}

pub fn style_dim() -> Style {
    Style::default().fg(DIM)
}

pub fn style_accent() -> Style {
    Style::default().fg(PRIMARY_CONTAINER).add_modifier(Modifier::BOLD)
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
    Style::default()
        .fg(FG)
        .add_modifier(Modifier::UNDERLINED)
}

/// Style for a focused button: bold, bright foreground, with glow accent.
pub fn style_focused_button() -> Style {
    Style::default()
        .fg(PRIMARY_CONTAINER)
        .add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK)
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

/// Render a glass-panel border effect: top/left brighter than bottom/right.
/// Returns styles for the four borders.
pub fn glass_border_styles(active: bool) -> (Style, Style, Style, Style) {
    let top_left = if active {
        Style::default().fg(PRIMARY_CONTAINER)
    } else {
        Style::default().fg(Color::Rgb(80, 80, 85))
    };
    let bottom_right = if active {
        Style::default().fg(PRIMARY)
    } else {
        Style::default().fg(Color::Rgb(38, 38, 45))
    };
    (top_left, top_left, bottom_right, bottom_right)
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