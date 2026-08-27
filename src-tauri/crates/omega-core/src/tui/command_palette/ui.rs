//! Command palette key handling + rendering (P5 split from command_palette.rs).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use super::state::CommandPaletteState;
use super::COMMANDS;
use crate::tui::theme;

/// Actions returned to the App key loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteAction {
    None,
    Close,
    /// Canonical command id, e.g. `"/clear"`.
    Select(&'static str),
}

/// Handle a key while the palette is open.
pub fn handle_key(state: &mut CommandPaletteState, key: KeyEvent) -> PaletteAction {
    if key.kind != KeyEventKind::Press {
        return PaletteAction::None;
    }

    // Ctrl+C closes (App global quit only when palette is closed).
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return PaletteAction::Close;
    }

    match key.code {
        KeyCode::Esc => PaletteAction::Close,
        KeyCode::Enter => match state.selected_id() {
            Some(id) => PaletteAction::Select(id),
            None => PaletteAction::None,
        },
        KeyCode::Up => {
            state.move_sel(-1);
            PaletteAction::None
        }
        KeyCode::Down => {
            state.move_sel(1);
            PaletteAction::None
        }
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                state.move_sel(-1);
            } else {
                state.move_sel(1);
            }
            PaletteAction::None
        }
        // Crossterm reports Shift+Tab as BackTab on most terminals.
        KeyCode::BackTab => {
            state.move_sel(-1);
            PaletteAction::None
        }
        KeyCode::Backspace => {
            state.query.pop();
            state.recompute_filter();
            PaletteAction::None
        }
        KeyCode::Char(c) => {
            // Ignore other control chords for typing.
            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT)
            {
                return PaletteAction::None;
            }
            state.query.push(c);
            state.recompute_filter();
            PaletteAction::None
        }
        _ => PaletteAction::None,
    }
}

/// Render command palette docked inline in the given area with a glass-style
/// thin-rule edge that matches the editor panel.
pub fn render(area: Rect, buf: &mut Buffer, state: &CommandPaletteState) {
    if !state.visible || area.width < 20 || area.height < 3 {
        return;
    }

    let line_style = Style::default().fg(theme::OUTLINE);
    let top_y = area.y;
    let bottom_y = area.y + area.height - 1;

    // Top rule with " commands " label:  ── commands ──
    let title = " commands ";
    let title_w = title.chars().count() as u16;
    let left_dash = (area.width.saturating_sub(title_w)) / 2;
    for x in area.x..area.x + area.width {
        buf.get_mut(x, top_y).set_char('─').set_style(line_style);
    }
    for (i, ch) in title.chars().enumerate() {
        let cx = area.x + left_dash + i as u16;
        if cx < area.x + area.width {
            buf.get_mut(cx, top_y).set_char(ch).set_fg(theme::DIM);
        }
    }

    // Bottom rule
    for x in area.x..area.x + area.width {
        buf.get_mut(x, bottom_y).set_char('─').set_style(line_style);
    }

    // Search line: "> query█"
    let search_y = area.y + 1;
    let search_display = format!("> {}_", state.query);
    let search_text = Line::from(Span::styled(
        search_display,
        Style::default().fg(theme::PRIMARY_CONTAINER),
    ));
    Paragraph::new(search_text).render(
        Rect::new(area.x + 1, search_y, area.width.saturating_sub(2), 1),
        buf,
    );

    // Compact: list fills remaining rows; selected id + description shown when possible.
    let body_y = area.y + 2;
    let body_h = area.height.saturating_sub(3); // top rule + search + list + bottom rule
    if body_h < 1 {
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    if state.filtered.is_empty() {
        lines.push(Line::from(Span::styled(
            " No matching commands",
            theme::style_dim(),
        )));
    } else {
        let max_rows = body_h as usize;
        let sel = state.selected;
        let start = if sel >= max_rows {
            sel + 1 - max_rows
        } else {
            0
        };
        for (row_i, &cmd_i) in state.filtered.iter().enumerate().skip(start).take(max_rows) {
            let entry = &COMMANDS[cmd_i];
            let is_sel = row_i == sel;
            let style = if is_sel {
                Style::default()
                    .fg(theme::PRIMARY_CONTAINER)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::FG)
            };
            let marker = if is_sel { "▸ " } else { "  " };
            let text = format!(
                "{}{}  {} — {}",
                marker, entry.id, entry.label, entry.description
            );
            lines.push(Line::from(Span::styled(
                truncate_to_width(&text, inner_width(area)),
                style,
            )));
        }
    }

    Paragraph::new(Text::from(lines))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .render(
            Rect::new(area.x + 1, body_y, area.width.saturating_sub(2), body_h),
            buf,
        );
}

fn inner_width(area: Rect) -> usize {
    area.width.saturating_sub(2) as usize
}

fn truncate_to_width(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for ch in s.chars() {
        let cw = if ch == '\t' { 1 } else { 1 };
        if w + cw > width {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out
}

/// Render command palette rows into a bounded area (used by layout.rs).
/// Only renders the list rows, not the search line or borders.
pub fn render_panel(area: Rect, buf: &mut Buffer, state: &CommandPaletteState, max_rows: u16) {
    if !state.visible
        || state.filtered.is_empty()
        || max_rows == 0
        || area.height < 1
        || area.width < 10
    {
        return;
    }

    let rows = (area.height).min(max_rows) as usize;
    let sel = state.selected;
    let count = state.filtered.len();
    let start = sel.saturating_sub(rows.saturating_sub(1));
    let end = (start + rows).min(count);

    for i in start..end {
        let row_idx = i - start;
        let cmd_idx = state.filtered[i];
        let entry = &COMMANDS[cmd_idx];
        let is_sel = i == sel;
        let style = if is_sel {
            Style::default()
                .fg(theme::PRIMARY_CONTAINER)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::FG)
        };
        let marker = if is_sel { "▸" } else { " " };
        let text = format!("{} {}  {}", marker, entry.id, entry.label);
        let display = truncate_to_width(&text, area.width as usize);

        Paragraph::new(Line::from(Span::styled(display, style))).render(
            Rect::new(area.x, area.y + row_idx as u16, area.width, 1),
            buf,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn enter_selects_current_command() {
        let mut s = CommandPaletteState::new();
        s.open("");
        // move to /clear (index 1 in full list)
        s.selected = 1;
        let action = handle_key(&mut s, press(KeyCode::Enter));
        assert_eq!(action, PaletteAction::Select("/clear"));
    }

    #[test]
    fn enter_noop_when_empty_filter() {
        let mut s = CommandPaletteState::new();
        s.open("zzz");
        assert!(s.filtered.is_empty());
        let action = handle_key(&mut s, press(KeyCode::Enter));
        assert_eq!(action, PaletteAction::None);
    }

    #[test]
    fn esc_closes() {
        let mut s = CommandPaletteState::new();
        s.open("");
        let action = handle_key(&mut s, press(KeyCode::Esc));
        assert_eq!(action, PaletteAction::Close);
    }

    #[test]
    fn typing_updates_query() {
        let mut s = CommandPaletteState::new();
        s.open("");
        handle_key(&mut s, press(KeyCode::Char('c')));
        handle_key(&mut s, press(KeyCode::Char('l')));
        assert_eq!(s.query, "cl");
        assert!(s.filtered.iter().any(|&i| COMMANDS[i].id == "/clear"));
    }

    #[test]
    fn backtab_moves_selection_up() {
        let mut s = CommandPaletteState::new();
        s.open("");
        s.selected = 1;
        let action = handle_key(&mut s, press(KeyCode::BackTab));
        assert_eq!(action, PaletteAction::None);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn truncate_respects_width() {
        assert_eq!(truncate_to_width("abcdef", 3), "abc");
        assert_eq!(truncate_to_width("abcdef", 0), "");
        assert_eq!(truncate_to_width("ab", 5), "ab");
    }
}
