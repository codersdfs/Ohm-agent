// ── TUI Layout Engine ────────────────────────────────────────────────────────────
// One public function (render_full_layout) and an ephemeral borrow-aggregator
// struct (LayoutChrome). All rendering helpers are private — the interface is
// narrow to concentrate layout decisions behind one seam.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

use super::banner;
use super::command_palette;
use super::editor::EditorState;
use super::help;
use super::provider_panel;
use super::status::StatusState;
use super::theme;
use super::transcript::Transcript;

use crate::commands;

/// Ephemeral rendering context — constructed each frame from App-owned state.
/// Copy values are taken by value; mutable borrows carry lifetime `'a`.
pub struct LayoutChrome<'a> {
    // ── Config refs (read-only) ──
    pub model_name: &'a str,
    pub config: &'a ::providers::ProviderConfig,

    // ── Mutable rendering targets ──
    pub transcript: &'a mut Transcript,
    pub status: &'a mut StatusState,

    // ── Read-only component refs ──
    pub editor: &'a EditorState,

    // ── Overlay visibility (Copy from App) ──
    pub show_help: bool,
    pub show_command_palette: bool,
    pub show_provider_panel: bool,

    // ── Overlay state (mutable borrow from App) ──
    pub command_palette: &'a mut command_palette::CommandPaletteState,
    pub provider_panel_state: &'a mut provider_panel::ProviderPanelState,

    // ── Streaming / misc flags (Copy from App) ──
    pub is_streaming: bool,
    pub is_command_mode: bool,
    pub anim_tick: u64,
    /// Names of tools currently executing, for live header chips.
    pub running_tools: Vec<String>,
}

/// Render the full TUI layout: chrome (bars, panels, editor) plus
/// overlays (help, command palette, provider panel). This is the single public
/// entry point; all other rendering helpers are private to this module.
pub fn render_full_layout(frame: &mut Frame, area: Rect, chrome: &mut LayoutChrome<'_>) {
    // ── Modal: provider panel takes full screen ──────────────────────────
    if chrome.show_provider_panel && !chrome.show_help {
        fill_area(frame, area, theme::SURFACE);
        provider_panel::render(
            area,
            frame.buffer_mut(),
            chrome.provider_panel_state,
            chrome.config,
        );
        return;
    }

    // ── Full-screen background ───────────────────────────────────────────
    fill_area(frame, area, theme::BG);
    // Flat codex-alike header: 3 rows (brand, provider+model, tokens+tools)
    let header_h = 3u16;
    // Auto-grow the chat editor as input wraps; command mode keeps a taller
    // fixed panel. Overlong input collapses to a `[pasted N lines]` marker so
    // the panel never grows unboundedly or floods the transcript above.
    let editor_h: u16 = {
        let inner_w = area.width.saturating_sub(2).max(1);
        if chrome.is_command_mode {
            7
        } else {
            let cap = (area.height / 3).clamp(4, 12);
            let rows = editor_display(&chrome.editor.buffer, inner_w, cap.saturating_sub(2));
            (rows.len() as u16 + 2).max(3)
        }
    };
    let status_h = 1u16;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Min(4),
            Constraint::Length(editor_h),
            Constraint::Length(status_h),
        ])
        .split(area);

    // ── Flat codex-alike header ──────────────────────────────────────────
    render_flat_header(
        frame,
        vert[0],
        chrome.config,
        chrome.is_streaming,
        chrome.running_tools.as_slice(),
    );

    // ── Main process panel ───────────────────────────────────────────────
    let splash_subtitle = format!("{} / {}", chrome.config.kind, chrome.model_name);
    render_process_panel(
        frame,
        vert[1],
        chrome.transcript,
        chrome.show_help,
        &splash_subtitle,
        chrome.is_streaming,
    );

    // ── Command panel ────────────────────────────────────────────────────
    render_command_panel(
        frame,
        vert[2],
        chrome.editor,
        chrome.command_palette,
        chrome.is_command_mode,
        chrome.is_streaming,
    );
    // ── Status bar ─────────────────────────────────────────────────────
    if chrome.is_streaming {
        // Explicit states (Thinking / Streaming / ToolCall) arrive via
        // stream events; layout only promotes a still-Idle loader so the
        // pre-first-token window shows activity too.
        chrome.status.ensure_active();
    } else {
        chrome
            .status
            .set_spinner_state(super::loader::SpinnerState::Idle);
    }
    chrome.status.tick_spinner();
    Widget::render(&*chrome.status, vert[3], frame.buffer_mut());

    // ── Overlays ─────────────────────────────────────────────────────────
    if chrome.show_help {
        help::render(area, frame.buffer_mut());
    }
}

// ── Private rendering helpers ──────────────────────────────────────────────

/// Fill an entire rect with a solid background color.
fn fill_area(frame: &mut Frame, area: Rect, color: Color) {
    let style = Style::default().bg(color);
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = frame.buffer_mut().get_mut(x, y);
            cell.set_bg(color);
            cell.set_style(style);
        }
    }
}

// ── Flat codex-alike header ──

/// Render a flat, borderless header inspired by Codex-style CLIs.
///
/// Layout varies by terminal width:
///   ≥80 cols: 3 rows (brand, provider/model, tokens + tools)
///   40–79:   2 rows (brand, tokens + tools; model dropped)
///   <40:     1 row  (brand only)
fn render_flat_header(
    frame: &mut Frame,
    area: Rect,
    config: &::providers::ProviderConfig,
    is_streaming: bool,
    running_tools: &[String],
) {
    let version_str = format!("v{}", env!("CARGO_PKG_VERSION"));
    let kind_str = config.kind.to_string();
    let model_str = &config.model;

    let logo_style = if is_streaming {
        Style::default()
            .fg(theme::PRIMARY)
            .add_modifier(Modifier::BOLD)
    } else {
        theme::style_dim()
    };

    // Mini wordmark styles: Ω takes the stream-green accent, O M E G A the
    // bright primary — a small nod to the splash banner without inflating
    // the 3-row chrome.
    let omega_mark_style = Style::default()
        .fg(theme::ACCENT_STREAM)
        .add_modifier(Modifier::BOLD);
    let wordmark_style = Style::default()
        .fg(theme::PRIMARY_CONTAINER)
        .add_modifier(Modifier::BOLD);

    if area.width < 40 {
        let row1 = Line::from(vec![
            Span::styled("Ω", omega_mark_style),
            Span::styled(" omega ", theme::style_dim()),
            Span::styled(&version_str, theme::style_dim()),
        ]);
        Paragraph::new(row1).render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());
        return;
    }

    let row1 = Line::from(vec![
        Span::styled("Ω", omega_mark_style),
        Span::styled("  ", logo_style),
        Span::styled("O M E G A", wordmark_style),
        Span::styled("   omega ", theme::style_dim()),
        Span::styled(&version_str, theme::style_dim()),
    ]);
    Paragraph::new(row1).render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());

    let row_count = if area.width >= 80 {
        let row2 = Line::from(vec![
            Span::styled(&kind_str, theme::style_dim()),
            Span::styled("/", theme::style_dim()),
            Span::styled(model_str, Style::default().fg(theme::SECONDARY)),
        ]);
        Paragraph::new(row2).render(
            Rect::new(area.x, area.y + 1, area.width, 1),
            frame.buffer_mut(),
        );

        render_header_row3(
            frame,
            Rect::new(area.x, area.y + 2, area.width, 1),
            is_streaming,
            running_tools,
        );
        3
    } else {
        render_header_row3(
            frame,
            Rect::new(area.x, area.y + 1, area.width, 1),
            is_streaming,
            running_tools,
        );
        2
    };

    if area.height > row_count {
        for x in area.x..area.x + area.width {
            let cell = frame.buffer_mut().get_mut(x, area.y + row_count);
            cell.set_symbol("─");
            cell.set_fg(theme::OUTLINE);
        }
    }
}

/// Render header row 3: compact token counts + stream marker + tool chips.
fn render_header_row3(frame: &mut Frame, area: Rect, is_streaming: bool, running_tools: &[String]) {
    let (tokens_in, tokens_out) = commands::cost_tracker::session_token_counts();
    let usage = StatusState::format_usage_compact(tokens_in, tokens_out);

    let mut left_spans = Vec::new();
    if is_streaming {
        left_spans.push(Span::styled(
            "… ",
            Style::default().fg(theme::ACCENT_STREAM),
        ));
    }
    left_spans.push(Span::styled(&usage, Style::default().fg(theme::FG)));

    let mut tool_spans = Vec::new();
    for tool in running_tools.iter().take(5) {
        tool_spans.push(Span::styled(
            format!("[{}] ", tool.to_uppercase()),
            Style::default().fg(theme::TOOL_DEFAULT),
        ));
    }
    let tool_text: String = tool_spans.iter().map(|s| s.content.clone()).collect();
    let tool_w = tool_text.chars().count() as u16;

    let left_w: u16 = left_spans.iter().map(|s| s.width() as u16).sum();
    let fill = area.width.saturating_sub(left_w).saturating_sub(tool_w);
    if fill > 0 {
        left_spans.push(Span::raw(" ".repeat(fill as usize)));
    }
    for span in tool_spans {
        left_spans.push(span);
    }

    Paragraph::new(Line::from(left_spans))
        .render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());
}
fn render_process_panel(
    frame: &mut Frame,
    area: Rect,
    transcript: &mut Transcript,
    _show_help: bool,
    splash_subtitle: &str,
    is_streaming: bool,
) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    // No frame around the transcript — just the content on the terminal canvas.
    fill_area(frame, area, theme::BG);

    // ── Startup splash: OMEGA AGENT banner until real conversation starts ──
    // Startup notices don't block the splash; the latest error notice (e.g.
    // missing API key) is folded into the splash block so warnings stay
    // visible. Clears itself on the first message/stream.
    let error_notice = transcript.entries.iter().rev().find_map(|e| match e {
        crate::tui::transcript::TranscriptEntry::Notice {
            text,
            is_error: true,
        } => Some(text.as_str()),
        _ => None,
    });
    if !transcript.has_conversation() && !is_streaming {
        banner::render_splash(
            area,
            frame.buffer_mut(),
            splash_subtitle,
            "type a message to begin · ^K commands",
            error_notice,
        );
        return;
    }

    transcript.render(frame, area);
}

// ── Command panel (replaces the simple editor input) ──────────────────────

/// Wrap a multi-line buffer into display rows of at most `inner_w` columns,
/// breaking purely by column width (a word longer than the column takes its
/// own overflow row rather than being truncated).
fn editor_wrap(text: &str, inner_w: u16) -> Vec<String> {
    let inner = inner_w.max(1) as usize;
    let mut rows = Vec::new();
    for logical in text.split('\n') {
        let mut cur = String::new();
        let mut len = 0usize; // chars already on the current row
        for c in logical.chars() {
            if len >= inner {
                rows.push(std::mem::take(&mut cur));
                len = 0;
            }
            cur.push(c);
            len += 1;
        }
        rows.push(cur);
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

/// Choose the display representation for the editor: the real wrapped text, or
/// a single `[pasted N lines]` marker when it cannot fit within `max_rows`.
fn editor_display(text: &str, inner_w: u16, max_rows: u16) -> Vec<String> {
    let rows = editor_wrap(text, inner_w);
    if rows.len() as u16 <= max_rows.max(1) {
        rows
    } else {
        vec![format!("[pasted {} lines]", text.lines().count())]
    }
}

/// Render the bottom command panel — a glass-chrome block that replaces the old
/// simple editor input. In chat mode it shows the text buffer. In command mode
/// it also shows the filtered slash-command list inside the block.
fn render_command_panel(
    frame: &mut Frame,
    area: Rect,
    editor: &EditorState,
    palette: &command_palette::CommandPaletteState,
    is_command_mode: bool,
    is_streaming: bool,
) {
    if area.height < 3 || area.width < 10 {
        return;
    }

    let showing_palette = is_command_mode && palette.visible;

    if !showing_palette {
        let out_style = Style::default().fg(theme::OUTLINE);
        let input_style = if is_streaming {
            Style::default().fg(theme::DIM)
        } else {
            Style::default().fg(theme::FG)
        };
        let rule = "─".repeat(area.width as usize);

        // Content is inset one column on each side; wrap to the inner width so
        // overlong lines gain rows instead of being truncated.
        let left_pad = 1u16;
        let inner_w = area.width.saturating_sub(2).max(1);
        // Rows available inside the allocated panel (top rule + content + bottom rule).
        let avail_rows = area.height.saturating_sub(2).max(1);
        // Overlong input collapses to a `[pasted N lines]` marker that fits.
        let rows = editor_display(&editor.buffer, inner_w, avail_rows);
        let shown_rows = rows.len() as u16;

        // Top rule
        Paragraph::new(Line::from(Span::styled(&rule, out_style)))
            .render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());

        // Wrapped content rows
        let content_y = area.y + 1;
        for r in 0..shown_rows {
            let row = &rows[r as usize];
            let y = content_y + r;
            for (col, ch) in row.chars().take(inner_w as usize).enumerate() {
                let cell = frame
                    .buffer_mut()
                    .get_mut(area.x + left_pad + col as u16, y);
                cell.set_char(ch);
                cell.set_style(input_style);
            }
        }

        // Cursor block at the end of the last shown row.
        let last_idx = shown_rows.saturating_sub(1);
        let last_row_cols = rows
            .get(last_idx as usize)
            .map(|r| r.chars().count())
            .unwrap_or(0);
        let cursor_x = area.x + left_pad + (last_row_cols as u16).min(inner_w);
        let cursor_y = content_y + last_idx;
        if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
            let cell = frame.buffer_mut().get_mut(cursor_x, cursor_y);
            cell.set_char('█');
            cell.set_style(input_style);
        }

        // Bottom rule
        Paragraph::new(Line::from(Span::styled(&rule, out_style))).render(
            Rect::new(area.x, area.y + shown_rows + 1, area.width, 1),
            frame.buffer_mut(),
        );
    }

    // ── Command palette (top rule with centered label, no side borders) ──
    if showing_palette {
        let top_y = area.y;
        let line_style = Style::default().fg(theme::OUTLINE);
        let title = " commands ";
        let title_w = title.chars().count() as u16;
        let left_dash = area.width.saturating_sub(title_w) / 2;

        for x in area.x..area.x + area.width {
            frame
                .buffer_mut()
                .get_mut(x, top_y)
                .set_symbol("─")
                .set_style(line_style);
        }
        for (i, ch) in title.chars().enumerate() {
            let cx = area.x + left_dash + i as u16;
            if cx < area.x + area.width {
                frame
                    .buffer_mut()
                    .get_mut(cx, top_y)
                    .set_char(ch)
                    .set_fg(theme::PRIMARY);
            }
        }

        // Search line
        Paragraph::new(Line::from(Span::styled(
            format!("> {}_", palette.query),
            Style::default().fg(theme::PRIMARY_CONTAINER),
        )))
        .render(
            Rect::new(area.x + 2, top_y + 1, area.width.saturating_sub(4), 1),
            frame.buffer_mut(),
        );

        // List rows
        let list_y = top_y + 2;
        let list_h = area.height.saturating_sub(3);
        if list_h > 0 {
            command_palette::render_panel(
                Rect::new(area.x + 2, list_y, area.width.saturating_sub(4), list_h),
                frame.buffer_mut(),
                palette,
                list_h,
            );
        }

        // Bottom rule
        let bottom_y = area.y + area.height - 1;
        for x in area.x..area.x + area.width {
            frame
                .buffer_mut()
                .get_mut(x, bottom_y)
                .set_symbol("─")
                .set_style(line_style);
        }
    }
}
