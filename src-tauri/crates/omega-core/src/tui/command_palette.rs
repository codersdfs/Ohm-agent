//! Command palette — searchable list of slash commands.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use super::theme;

/// One palette row / slash command.
#[derive(Debug, Clone, Copy)]
pub struct CommandEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    /// Extra search terms not shown in the UI.
    pub keywords: &'static [&'static str],
}

/// Canonical v1 catalog. Ids must match `App::handle_slash_command`.
pub static COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        id: "/help",
        label: "Help",
        aliases: &["/?", "/h"],
        description: "Show available commands",
        keywords: &["commands", "usage", "docs"],
    },
    CommandEntry {
        id: "/clear",
        label: "Clear session",
        aliases: &["/cls"],
        description: "Clear transcript and session",
        keywords: &["reset", "new", "session"],
    },
    CommandEntry {
        id: "/tools",
        label: "List tools",
        aliases: &[],
        description: "List available agent tools",
        keywords: &["agent", "capabilities"],
    },
    CommandEntry {
        id: "/model",
        label: "Choose model",
        aliases: &[],
        description: "Open model picker for current provider",
        keywords: &["llm", "gpt", "claude", "switch"],
    },
    CommandEntry {
        id: "/provider",
        label: "Choose provider",
        aliases: &["/providers", "/p"],
        description: "Open provider configuration wizard",
        keywords: &["api", "openai", "anthropic", "google", "endpoint"],
    },
    CommandEntry {
        id: "/cost",
        label: "Session cost",
        aliases: &[],
        description: "Show session token usage",
        keywords: &["tokens", "usage", "billing"],
    },
    CommandEntry {
        id: "/exit",
        label: "Quit",
        aliases: &["/quit"],
        description: "Quit Omega",
        keywords: &["quit", "close", "leave"],
    },
    CommandEntry {
        id: "/fetch",
        label: "Fetch URL",
        aliases: &["/web", "/url"],
        description: "Fetch and display content from a URL",
        keywords: &["http", "web", "internet", "download", "curl", "get"],
    },
    CommandEntry {
        id: "/status",
        label: "System status",
        aliases: &["/ping", "/health", "/net"],
        description: "Check network connectivity and provider status",
        keywords: &["network", "connectivity", "reachable", "health", "ping"],
    },
    CommandEntry {
        id: "/search",
        label: "Web search",
        aliases: &["/google", "/websearch"],
        description: "Search the web",
        keywords: &["google", "duckduckgo", "web", "browse", "find"],
    },
];

/// Build lowercase haystack for keyword search.
fn haystack(entry: &CommandEntry) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(4 + entry.aliases.len() + entry.keywords.len());
    parts.push(entry.id);
    parts.push(entry.label);
    parts.extend(entry.aliases.iter().copied());
    parts.push(entry.description);
    parts.extend(entry.keywords.iter().copied());
    parts.join(" ").to_lowercase()
}

/// True if every whitespace-separated keyword is a substring of the entry haystack.
pub fn command_matches(entry: &CommandEntry, query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    let hay = haystack(entry);
    q.split_whitespace()
        .all(|kw| hay.contains(&kw.to_lowercase()))
}

/// Indices into `COMMANDS` matching `query` (stable registry order).
pub fn filter_commands(query: &str) -> Vec<usize> {
    COMMANDS
        .iter()
        .enumerate()
        .filter(|(_, e)| command_matches(e, query))
        .map(|(i, _)| i)
        .collect()
}

/// Actions returned to the App key loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteAction {
    None,
    Close,
    /// Canonical command id, e.g. `"/clear"`.
    Select(&'static str),
}

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    pub visible: bool,
    pub query: String,
    pub selected: usize,
    pub filtered: Vec<usize>,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        let mut s = Self {
            visible: false,
            query: String::new(),
            selected: 0,
            filtered: Vec::new(),
        };
        s.recompute_filter();
        s
    }

    /// Open palette, optionally seeding the search query (e.g. `"/"`).
    pub fn open(&mut self, seed_query: &str) {
        self.visible = true;
        self.query = seed_query.to_string();
        self.selected = 0;
        self.recompute_filter();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.selected = 0;
        self.recompute_filter();
    }

    pub fn recompute_filter(&mut self) {
        self.filtered = filter_commands(&self.query);
        if self.filtered.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.filtered.len() - 1);
        }
    }

    fn move_sel(&mut self, delta: isize) {
        let n = self.filtered.len();
        if n == 0 {
            self.selected = 0;
            return;
        }
        let cur = self.selected as isize;
        let next = (cur + delta).rem_euclid(n as isize) as usize;
        self.selected = next;
    }

    pub fn selected_id(&self) -> Option<&'static str> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| COMMANDS.get(i))
            .map(|e| e.id)
    }
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
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
        buf.get_mut(x, top_y)
            .set_char('─')
            .set_style(line_style);
    }
    for (i, ch) in title.chars().enumerate() {
        let cx = area.x + left_dash + i as u16;
        if cx < area.x + area.width {
            buf.get_mut(cx, top_y)
                .set_char(ch)
                .set_fg(theme::DIM);
        }
    }

    // Bottom rule
    for x in area.x..area.x + area.width {
        buf.get_mut(x, bottom_y)
            .set_char('─')
            .set_style(line_style);
    }

    // Search line: "> query█"
    let search_y = area.y + 1;
    let search_display = format!("> {}_", state.query);
    let search_text = Line::from(Span::styled(
        search_display,
        Style::default().fg(theme::PRIMARY_CONTAINER),
    ));
    Paragraph::new(search_text)
        .render(Rect::new(area.x + 1, search_y, area.width.saturating_sub(2), 1), buf);

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
        for (row_i, &cmd_i) in state
            .filtered
            .iter()
            .enumerate()
            .skip(start)
            .take(max_rows)
        {
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
        .render(Rect::new(area.x + 1, body_y, area.width.saturating_sub(2), body_h), buf);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn filter_empty_returns_all() {
        let ids: Vec<_> = filter_commands("")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "/help",
                "/clear",
                "/tools",
                "/model",
                "/provider",
                "/cost",
                "/exit",
                "/fetch",
                "/status",
                "/search",
            ]
        );
    }

    #[test]
    fn filter_substring_matches_clear() {
        let ids: Vec<_> = filter_commands("cle")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert_eq!(ids, vec!["/clear"]);
    }

    #[test]
    fn filter_alias_cls() {
        let ids: Vec<_> = filter_commands("cls")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert_eq!(ids, vec!["/clear"]);
    }

    #[test]
    fn filter_multi_keyword_token_cost() {
        let ids: Vec<_> = filter_commands("token cost")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert_eq!(ids, vec!["/cost"]);
    }

    #[test]
    fn filter_keyword_quit_matches_exit() {
        let ids: Vec<_> = filter_commands("quit")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/exit"));
    }

    #[test]
    fn filter_no_match() {
        assert!(filter_commands("zzz").is_empty());
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn open_seeds_query_and_filters() {
        let mut s = CommandPaletteState::new();
        s.open("/");
        assert!(s.visible);
        assert_eq!(s.query, "/");
        assert!(!s.filtered.is_empty());
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn selection_clamps_when_filter_shrinks() {
        let mut s = CommandPaletteState::new();
        s.open("");
        s.selected = 6; // last of 7
        s.query = "cle".into();
        s.recompute_filter();
        assert_eq!(s.filtered.len(), 1);
        assert_eq!(s.selected, 0);
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
    fn filter_fetch_matches_new_command() {
        let ids: Vec<_> = filter_commands("fetch")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/fetch"), "fetch command should appear when searching 'fetch'");
    }

    #[test]
    fn filter_status_matches_new_command() {
        let ids: Vec<_> = filter_commands("status")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/status"), "status command should appear when searching 'status'");
    }

    #[test]
    fn filter_search_matches_new_command() {
        let ids: Vec<_> = filter_commands("search web")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/search"), "search command should appear when searching 'search web'");
    }
}
