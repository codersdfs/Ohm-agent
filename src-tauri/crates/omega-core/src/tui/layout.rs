// ── TUI Layout Engine ────────────────────────────────────────────────────────────
// One public function (render_full_layout) and an ephemeral borrow-aggregator
// struct (LayoutChrome). All rendering helpers are private — the interface is
// narrow to concentrate layout decisions behind one seam.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Frame;

use super::command_palette;
use super::editor::EditorState;
use super::help;
use super::provider_panel;
use super::status::StatusState;
use super::theme;
use super::component::Component;
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
    pub session_messages: u64,
    pub anim_tick: u64,
}

/// Render the full TUI layout: chrome (bars, panels, editor, footer) plus
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

    // ── Layout: vertical stack ───────────────────────────────────────────
    let top_bar_h = 1u16;
    let metrics_h = 3u16;
    let footer_h = 1u16;
    let editor_h = 3u16;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_bar_h),
            Constraint::Length(metrics_h),
            Constraint::Min(4),
            Constraint::Length(editor_h),
            Constraint::Length(footer_h),
        ])
        .split(area);

    // ── Top system bar ───────────────────────────────────────────────────
    render_top_bar(frame, vert[0], chrome.model_name);

    // ── Metrics panel ────────────────────────────────────────────────────
    render_metrics_panel(frame, vert[1], chrome.config, chrome.is_streaming);

    // ── Main process panel ───────────────────────────────────────────────
    render_process_panel(frame, vert[2], chrome.transcript, chrome.show_help);

    // ── Command input ────────────────────────────────────────────────────
    render_command_input(
        frame,
        vert[3],
        chrome.editor,
        chrome.is_streaming,
        chrome.status,
    );

    // ── Footer bar ───────────────────────────────────────────────────────
    chrome.status.hint_text = Some("[CR] COMMIT | [^C] ABORT | ^K cmds | ? help".into());
    let (tokens_in, tokens_out) = commands::chat::session_token_counts();
    chrome.status.tokens_in = tokens_in;
    chrome.status.tokens_out = tokens_out;
    chrome.status.messages_count = chrome.session_messages;
    chrome.status.streaming_estimate = 0;
    frame.render_widget(&*chrome.status, vert[4]);

    // ── Overlays ─────────────────────────────────────────────────────────
    if chrome.show_help {
        help::render(area, frame.buffer_mut());
    }
    if chrome.show_command_palette {
        command_palette::render(area, frame.buffer_mut(), chrome.command_palette);
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

/// Render a glass-style bordered block: fills inner bg, draws box-drawing
/// border, returns the inner area for content placement.
fn render_glass_block(
    frame: &mut Frame,
    area: Rect,
    inner_bg: Color,
    border_color: Color,
    title: &str,
    title_color: Color,
) -> Rect {
    let inner_pad = 1u16;
    let inner_area = if area.width > 2 && area.height > 2 {
        Rect::new(
            area.x + inner_pad,
            area.y + inner_pad,
            area.width.saturating_sub(inner_pad * 2),
            area.height.saturating_sub(inner_pad * 2),
        )
    } else {
        area
    };

    // Fill inner
    fill_area(frame, inner_area, inner_bg);

    // Draw border using box-drawing characters ┌─…─┐ │  │ └─…─┘
    let bw = area.width;
    let bh = area.height;

    // Top border ┌─ title ────────┐
    let title_chars: Vec<char> = title.chars().collect();
    let dash_count = bw
        .saturating_sub(title_chars.len() as u16)
        .saturating_sub(4);
    {
        let y = area.y;
        // ┌
        frame
            .buffer_mut()
            .get_mut(area.x, y)
            .set_symbol("┌")
            .set_fg(border_color);
        // title
        for (i, ch) in title_chars.iter().enumerate() {
            let cx = area.x + 2 + i as u16;
            if cx < area.x + bw {
                frame
                    .buffer_mut()
                    .get_mut(cx, y)
                    .set_char(*ch)
                    .set_fg(title_color);
            }
        }
        // ─ fill
        for i in 0..dash_count {
            let cx = area.x + 2 + title_chars.len() as u16 + i;
            if cx < area.x + bw - 1 {
                frame
                    .buffer_mut()
                    .get_mut(cx, y)
                    .set_symbol("─")
                    .set_fg(border_color);
            }
        }
        // ┐
        frame
            .buffer_mut()
            .get_mut(area.x + bw - 1, y)
            .set_symbol("┐")
            .set_fg(border_color);
    }

    // Sides │ … │
    for dy in 1..bh.saturating_sub(1) {
        let y = area.y + dy;
        frame
            .buffer_mut()
            .get_mut(area.x, y)
            .set_symbol("│")
            .set_fg(border_color);
        frame
            .buffer_mut()
            .get_mut(area.x + bw - 1, y)
            .set_symbol("│")
            .set_fg(border_color);
    }

    // Bottom border └──────────────┘
    {
        let y = area.y + bh - 1;
        frame
            .buffer_mut()
            .get_mut(area.x, y)
            .set_symbol("└")
            .set_fg(border_color);
        for i in 0..dash_count {
            let cx = area.x + 2 + title_chars.len() as u16 + i;
            if cx < area.x + bw - 1 {
                frame
                    .buffer_mut()
                    .get_mut(cx, y)
                    .set_symbol("─")
                    .set_fg(border_color);
            }
        }
        frame
            .buffer_mut()
            .get_mut(area.x + bw - 1, y)
            .set_symbol("┘")
            .set_fg(border_color);
    }

    inner_area
}

// ── Top system bar ──────────────────────────────────────────────────────

fn render_top_bar(frame: &mut Frame, area: Rect, model: &str) {
    if area.height < 1 || area.width < 30 {
        return;
    }

    let left_spans = vec![
        Span::styled(
            " OMEGA_AGENT ",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("v", theme::style_dim()),
        Span::styled(env!("CARGO_PKG_VERSION"), theme::style_dim()),
        Span::styled(" · ", theme::style_dim()),
        Span::styled("SYS_STATUS: ", theme::style_dim()),
        Span::styled("ONLINE", Style::default().fg(theme::PRIMARY)),
        Span::styled(" · ", theme::style_dim()),
        Span::styled("UPTIME: ", theme::style_dim()),
        Span::styled(model, Style::default().fg(theme::SECONDARY)),
    ];

    let right_hint = format!(" [F1] HELP [F2] LOGS [F3] NET [F10] EXIT ");
    let right_w = right_hint.len() as u16;
    let left_w: u16 = left_spans.iter().map(|s| s.width() as u16).sum();
    let fill = area.width.saturating_sub(left_w).saturating_sub(right_w);

    let mut out = vec![];
    out.extend(left_spans);
    if fill > 0 {
        out.push(Span::raw(" ".repeat(fill as usize)));
    }
    out.push(Span::styled(right_hint, theme::style_dim()));

    let para = Paragraph::new(Line::from(out));
    para.render(Rect::new(area.x, area.y, area.width, 1), frame.buffer_mut());

    // separator rule under top bar
    if area.height > 1 {
        for x in area.x..area.x + area.width {
            let cell = frame.buffer_mut().get_mut(x, area.y + area.height - 1);
            cell.set_symbol("─");
            cell.set_fg(theme::OUTLINE);
        }
    }
}

// ── Metrics panel ────────────────────────────────────────────────────────

fn render_metrics_panel(
    frame: &mut Frame,
    area: Rect,
    config: &::providers::ProviderConfig,
    _is_streaming: bool,
) {
    if area.height < 3 || area.width < 40 {
        return;
    }

    let inner = render_glass_block(
        frame,
        area,
        theme::SURFACE_LOW,
        theme::OUTLINE,
        " SYSTEM METRICS & ACTIVE TOOLS ",
        theme::PRIMARY,
    );

    // Row 0: real session input/output usage
    let gauge_y = inner.y;
    let (tokens_in, tokens_out) = commands::chat::session_token_counts();
    let usage = StatusState::format_token_usage(tokens_in, tokens_out);
    let gauge_spans = vec![
        Span::styled("TOKENS ", theme::style_dim()),
        Span::styled(usage, Style::default().fg(theme::PRIMARY)),
    ];
    Paragraph::new(Line::from(gauge_spans)).render(
        Rect::new(inner.x + 1, gauge_y, inner.width.saturating_sub(2), 1),
        frame.buffer_mut(),
    );

    // Row 1: model only
    let row1_y = gauge_y + 1;
    let model_str = config.model.clone();
    let model_spans = vec![
        Span::styled("MODEL ", theme::style_dim()),
        Span::styled(model_str, Style::default().fg(theme::SECONDARY)),
    ];
    Paragraph::new(Line::from(model_spans)).render(
        Rect::new(inner.x + 1, row1_y, inner.width.saturating_sub(2), 1),
        frame.buffer_mut(),
    );

    // Row 2: active tool chips
    if inner.height > 2 {
        let chips_y = row1_y + 1;
        let chips = vec![
            ("[ BROWSER ]", theme::TOOL_BROWSER),
            ("[ SHELL ]", theme::TOOL_SHELL),
            ("[ FILE_SYS ]", theme::TOOL_FILE_SYS),
            ("[ SEARCH ]", theme::TOOL_SEARCH),
        ];
        let mut chip_spans: Vec<Span> = Vec::new();
        for (i, (label, col)) in chips.iter().enumerate() {
            if i > 0 {
                chip_spans.push(Span::raw(" "));
            }
            chip_spans.push(Span::styled(*label, Style::default().fg(*col)));
        }
        Paragraph::new(Line::from(chip_spans))
            .alignment(Alignment::Right)
            .render(
                Rect::new(inner.x + 1, chips_y, inner.width.saturating_sub(2), 1),
                frame.buffer_mut(),
            );
    }
}

// ── Main process panel ────────────────────────────────────────────────────

fn render_process_panel(
    frame: &mut Frame,
    area: Rect,
    transcript: &mut Transcript,
    _show_help: bool,
) {
    if area.height < 3 || area.width < 20 {
        return;
    }

    // No frame around the transcript — just the content on the terminal canvas.
    fill_area(frame, area, theme::BG);
    transcript.render(frame, area);
}

// ── Command input ─────────────────────────────────────────────────────────

fn render_command_input(
    frame: &mut Frame,
    area: Rect,
    editor: &EditorState,
    is_streaming: bool,
    status: &StatusState,
) {
    let spinner = &status.spinner;
    if area.height < 3 || area.width < 4 {
        return;
    }

    // Inherit the terminal background — no dark recessed fill.
    fill_area(frame, area, theme::BG);

    let line_style = Style::default().fg(theme::OUTLINE);
    let top_y = area.y;
    let content_y = area.y + 1;
    let bottom_y = area.y + 2;

    // Top and bottom rules only — no side borders, no labels.
    for x in area.x..area.x + area.width {
        frame
            .buffer_mut()
            .get_mut(x, top_y)
            .set_symbol("─")
            .set_style(line_style);
        frame
            .buffer_mut()
            .get_mut(x, bottom_y)
            .set_symbol("─")
            .set_style(line_style);
    }

    let content_x = area.x + 1;
    let content_w = area.width.saturating_sub(2);
    if content_w == 0 {
        return;
    }

    if is_streaming && editor.buffer.is_empty() {
        let activity = format!("{} {}", spinner.current_glyph(), spinner.current_phrase());
        Paragraph::new(Line::from(Span::styled(activity, spinner.glyph_style()))).render(
            Rect::new(content_x, content_y, content_w, 1),
            frame.buffer_mut(),
        );
    } else if !editor.buffer.is_empty() {
        // Single-line display: keep the tail of multi-line text visible.
        let display = editor.buffer.lines().last().unwrap_or("");
        let shown = if display.chars().count() > content_w as usize {
            let skip = display.chars().count().saturating_sub(content_w as usize);
            display.chars().skip(skip).collect::<String>()
        } else {
            display.to_string()
        };
        Paragraph::new(Line::from(Span::styled(
            shown,
            Style::default().fg(theme::FG),
        )))
        .render(
            Rect::new(content_x, content_y, content_w, 1),
            frame.buffer_mut(),
        );
    }
    // Empty idle: just the two horizontal lines.
}