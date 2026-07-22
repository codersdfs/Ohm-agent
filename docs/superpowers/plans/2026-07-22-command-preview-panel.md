# Command Preview Panel (Command Palette) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Ctrl+K / empty-buffer `/` command palette that keyword-searches existing slash commands (name + description) and runs the selected one via `handle_slash_command`.

**Architecture:** New `command_palette` TUI module in `omega-core` owns registry, multi-keyword AND filter, key handling, and centered modal render. `omega-cli` App owns visibility and routes Select → existing `handle_slash_command` (single execution path). Pattern mirrors `provider_panel`.

**Tech Stack:** Rust, ratatui 0.26, crossterm 0.27, cargo test

**Spec:** `docs/superpowers/specs/2026-07-22-command-preview-panel-design.md`

## Global Constraints

- v1 lists only existing slash commands: `/help`, `/clear`, `/tools`, `/model`, `/provider`, `/cost`, `/exit` (with aliases from design)
- Selection runs immediately through `App::handle_slash_command` — do not reimplement command side effects in the palette
- Keyword search: whitespace-split AND over id + label + aliases + description + hidden keywords
- Cannot open while streaming or while provider panel is open
- Empty-buffer `/` opens palette and must not leave `/` in the editor
- No mouse, no fuzzy rank, no free-text args in palette for v1
- Prefer unit tests in `command_palette.rs`; verify with `cargo test -p omega-core` and `cargo check --workspace`

---

## File Structure

| File | Responsibility |
|------|----------------|
| Create `src-tauri/crates/omega-core/src/tui/command_palette.rs` | `CommandEntry`, static `COMMANDS`, filter, state, `handle_key`, `render`, unit tests |
| Modify `src-tauri/crates/omega-core/src/tui/mod.rs` | Export `pub mod command_palette;` |
| Modify `src-tauri/crates/omega-cli/src/main.rs` | App fields, open shortcuts (Ctrl+K, empty `/`), palette key exclusivity, render overlay, Select → `handle_slash_command` |

No new crates or Cargo dependencies.

---

### Task 1: Registry + keyword filter (TDD)

**Files:**
- Create: `src-tauri/crates/omega-core/src/tui/command_palette.rs`
- Modify: `src-tauri/crates/omega-core/src/tui/mod.rs`
- Test: unit tests inside `command_palette.rs`

**Interfaces:**
- Consumes: nothing from later tasks
- Produces:
  - `pub struct CommandEntry { id, label, aliases, description, keywords }` (all `&'static`)
  - `pub static COMMANDS: &[CommandEntry]` (7 entries)
  - `pub fn command_matches(entry: &CommandEntry, query: &str) -> bool`
  - `pub fn filter_commands(query: &str) -> Vec<usize>` (indices into `COMMANDS`)

- [ ] **Step 1: Create module stub and export it**

Add to `src-tauri/crates/omega-core/src/tui/mod.rs`:

```rust
pub mod command_palette;
```

Create `src-tauri/crates/omega-core/src/tui/command_palette.rs` with types + empty filter that will fail tests until implemented:

```rust
//! Command palette — searchable list of slash commands.

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

#[cfg(test)]
mod tests {
    use super::*;

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
                "/exit"
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
}
```

- [ ] **Step 2: Run filter tests — expect PASS**

Run:

```bash
cargo test -p omega-core command_palette -- --nocapture
```

Expected: all 6 filter tests PASS. If the module is missing from `mod.rs`, fix that first.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/crates/omega-core/src/tui/command_palette.rs src-tauri/crates/omega-core/src/tui/mod.rs
git commit -m "feat(tui): add command palette registry and keyword filter"
```

---

### Task 2: Palette state + key handling (TDD)

**Files:**
- Modify: `src-tauri/crates/omega-core/src/tui/command_palette.rs`
- Test: same file `#[cfg(test)]`

**Interfaces:**
- Consumes: `filter_commands`, `COMMANDS` from Task 1
- Produces:
  - `pub enum PaletteAction { None, Close, Select(&'static str) }`
  - `pub struct CommandPaletteState { visible, query, selected, filtered }`
  - `impl CommandPaletteState { pub fn new() -> Self; pub fn open(&mut self, seed_query: &str); pub fn close(&mut self); pub fn recompute_filter(&mut self); }`
  - `pub fn handle_key(state: &mut CommandPaletteState, key: KeyEvent) -> PaletteAction`

- [ ] **Step 1: Write failing key-handling tests**

Append to the `tests` module (keep existing filter tests):

```rust
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
```

- [ ] **Step 2: Run tests — expect FAIL (types missing)**

Run:

```bash
cargo test -p omega-core command_palette -- --nocapture
```

Expected: compile error — `CommandPaletteState` / `handle_key` not found.

- [ ] **Step 3: Implement state + handle_key**

Append to `command_palette.rs` (after `filter_commands`, before `#[cfg(test)]`):

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

use super::theme;

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

    fn selected_id(&self) -> Option<&'static str> {
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
```

- [ ] **Step 4: Run tests — expect PASS**

Run:

```bash
cargo test -p omega-core command_palette -- --nocapture
```

Expected: all filter + state/key tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/omega-core/src/tui/command_palette.rs
git commit -m "feat(tui): command palette state and key handling"
```

---

### Task 3: Render the palette modal

**Files:**
- Modify: `src-tauri/crates/omega-core/src/tui/command_palette.rs`
- Test: compile-only (visual; no golden TUI harness in v1)

**Interfaces:**
- Consumes: `CommandPaletteState`, `COMMANDS`, `theme`
- Produces: `pub fn render(area: Rect, buf: &mut Buffer, state: &CommandPaletteState)`

- [ ] **Step 1: Implement `render`**

Append to `command_palette.rs` (after `handle_key`):

```rust
/// Render centered command palette overlay.
pub fn render(area: Rect, buf: &mut Buffer, state: &CommandPaletteState) {
    if !state.visible || area.width < 20 || area.height < 6 {
        return;
    }

    // Dim full area (same approach as help overlay).
    for cy in area.y..area.y + area.height {
        for cx in area.x..area.x + area.width {
            if let Some(cell) = theme::buf_cell_mut(buf, cx, cy) {
                cell.set_style(Style::default().fg(theme::DIM));
            }
        }
    }

    let popup_width = area.width.min(48).max(24);
    // chrome: borders + title + search line + optional description + empty/rows
    let row_count = if state.filtered.is_empty() {
        1usize
    } else {
        state.filtered.len().min(8)
    };
    // 2 border + 1 search + rows + 1 description footer
    let popup_height = (row_count as u16)
        .saturating_add(5)
        .min(area.height.saturating_sub(2))
        .max(6);
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme::PRIMARY))
        .title(Line::from(Span::styled(
            " commands ",
            Style::default().fg(theme::DIM),
        )))
        .style(Style::default().bg(theme::SURFACE_HIGH));
    let inner = block.inner(popup_area);
    block.render(popup_area, buf);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    // Search line: "> query█"
    let search_display = format!("> {}_", state.query);
    let search_line = Line::from(Span::styled(
        truncate_to_width(&search_display, inner.width as usize),
        Style::default().fg(theme::PRIMARY_CONTAINER),
    ));
    Paragraph::new(search_line)
        .style(Style::default().bg(theme::SURFACE_HIGH))
        .render(Rect::new(inner.x, inner.y, inner.width, 1), buf);

    let list_y = inner.y + 1;
    let list_h = inner.height.saturating_sub(2); // leave 1 for description
    let list_area = Rect::new(inner.x, list_y, inner.width, list_h);

    let mut lines: Vec<Line<'static>> = Vec::new();
    if state.filtered.is_empty() {
        lines.push(Line::from(Span::styled(
            " No matching commands",
            theme::style_dim(),
        )));
    } else {
        // Scroll window so selected stays visible.
        let max_rows = list_h as usize;
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
            let text = format!("{}{}  {}", marker, entry.id, entry.label);
            lines.push(Line::from(Span::styled(
                truncate_to_width(&text, inner.width as usize),
                style,
            )));
        }
    }

    Paragraph::new(Text::from(lines))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(theme::SURFACE_HIGH))
        .render(list_area, buf);

    // Description footer for selected row.
    let desc = state
        .selected_id()
        .and_then(|id| COMMANDS.iter().find(|e| e.id == id))
        .map(|e| e.description)
        .unwrap_or("");
    let desc_y = inner.y + inner.height.saturating_sub(1);
    Paragraph::new(Line::from(Span::styled(
        truncate_to_width(&format!(" {desc}"), inner.width as usize),
        theme::style_dim(),
    )))
    .style(Style::default().bg(theme::SURFACE_HIGH))
    .render(Rect::new(inner.x, desc_y, inner.width, 1), buf);
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
```

Confirm `theme::buf_cell_mut` exists (used by `help.rs`). If missing, use the same pattern `help.rs` uses — do not invent a different dimming approach.

- [ ] **Step 2: Compile check**

Run:

```bash
cargo test -p omega-core command_palette -- --nocapture
cargo check -p omega-core
```

Expected: PASS / finished successfully.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/crates/omega-core/src/tui/command_palette.rs
git commit -m "feat(tui): render command palette modal with search line"
```

---

### Task 4: Wire palette into App (open / keys / select)

**Files:**
- Modify: `src-tauri/crates/omega-cli/src/main.rs` (App struct, `new`, `handle_key`)

**Interfaces:**
- Consumes: `CommandPaletteState`, `handle_key`, `PaletteAction`, `render` from Tasks 1–3
- Produces: App opens palette on Ctrl+K and empty-buffer `/`; Select runs `handle_slash_command`

- [ ] **Step 1: Add App fields**

In `struct App` (near `show_provider_panel`):

```rust
    // Command palette
    show_command_palette: bool,
    command_palette: omega_core::tui::command_palette::CommandPaletteState,
```

In `App::new` initializer (with other `show_*` fields):

```rust
            show_command_palette: false,
            command_palette: omega_core::tui::command_palette::CommandPaletteState::new(),
```

- [ ] **Step 2: Add open helper**

Inside `impl App`, near `handle_slash_command`:

```rust
    fn open_command_palette(&mut self, seed_query: &str) {
        if self.is_streaming || self.show_provider_panel {
            return;
        }
        self.show_help = false;
        self.command_palette.open(seed_query);
        self.show_command_palette = true;
    }
```

- [ ] **Step 3: Route keys — palette exclusivity + open shortcuts**

In `handle_key`, **after** the streaming early-return and **before** the provider-panel block is fine; place palette handling **immediately after** the provider-panel block (provider still wins if both were somehow true). Recommended order:

1. Global Ctrl+C / Ctrl+Q (existing) — **adjust Ctrl+C**: if palette open and not streaming, close palette instead of quit  
2. Streaming early return (existing)  
3. Provider panel exclusivity (existing)  
4. **NEW: command palette exclusivity**  
5. Help `?` (existing)  
6. **NEW: Ctrl+K open**  
7. Ctrl+B / Ctrl+E (existing)  
8. Editor keys — **intercept bare `/` when buffer empty**

**Ctrl+C adjustment** (replace the existing Ctrl+C arm):

```rust
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.is_streaming {
                    self.cancel_streaming();
                } else if self.show_command_palette {
                    self.command_palette.close();
                    self.show_command_palette = false;
                } else {
                    self.should_quit = true;
                }
                return;
            }
```

**Palette exclusivity block** (insert after provider panel `return`):

```rust
        // Command palette takes over key handling
        if self.show_command_palette {
            let action = omega_core::tui::command_palette::handle_key(
                &mut self.command_palette,
                key,
            );
            match action {
                omega_core::tui::command_palette::PaletteAction::Select(id) => {
                    self.command_palette.close();
                    self.show_command_palette = false;
                    self.handle_slash_command(id);
                }
                omega_core::tui::command_palette::PaletteAction::Close => {
                    self.command_palette.close();
                    self.show_command_palette = false;
                }
                omega_core::tui::command_palette::PaletteAction::None => {}
            }
            return;
        }
```

**Ctrl+K open** (after help toggle, before Ctrl+B):

```rust
        // Ctrl+K: open command palette
        if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_command_palette("");
            return;
        }
```

**Empty-buffer `/` open** — change editor delegation so `/` does not insert when buffer is empty:

Find the block:

```rust
        // Delegate to editor component (handles letters, Enter, navigation, Tab)
        let action = self.editor.handle_key(key);
        match action {
            Action::SendMessage => self.submit_message(),
            _ => {}
        }
```

Replace with:

```rust
        // Empty-buffer `/` opens the command palette instead of inserting.
        if key.code == KeyCode::Char('/')
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && self.editor.buffer.is_empty()
        {
            self.open_command_palette("/");
            return;
        }

        // Delegate to editor component (handles letters, Enter, navigation, Tab)
        let action = self.editor.handle_key(key);
        match action {
            Action::SendMessage => self.submit_message(),
            _ => {}
        }
```

- [ ] **Step 4: Compile check CLI crate**

Run:

```bash
cargo check -p omega-cli
```

Expected: finished successfully (no unresolved imports).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/omega-cli/src/main.rs
git commit -m "feat(cli): wire command palette open, keys, and select"
```

---

### Task 5: Render overlay + status hint

**Files:**
- Modify: `src-tauri/crates/omega-cli/src/main.rs` (`render_widgets`)

**Interfaces:**
- Consumes: `command_palette::render`, `show_command_palette`
- Produces: palette visible on screen; footer mentions Ctrl+K

- [ ] **Step 1: Render palette overlay**

In `render_widgets`, after the help overlay block (end of function, before closing brace):

```rust
        if self.show_help {
            omega_core::tui::help::render(area, frame.buffer_mut());
        }
        if self.show_command_palette {
            omega_core::tui::command_palette::render(
                area,
                frame.buffer_mut(),
                &self.command_palette,
            );
        }
```

Also update the early modal return so palette can own the screen when help is not showing and you prefer a filled backdrop. **Do not** short-circuit the whole chrome for the palette unless dimming on top of chrome looks wrong — the design is a centered overlay on the main UI (like help). Prefer drawing over chrome, same as help.

If provider panel early-return condition is `show_provider_panel && !show_help`, leave it unchanged. Palette is not full-screen exclusive like provider panel.

- [ ] **Step 2: Footer hint**

Where hint is set:

```rust
        self.status.hint_text = Some("[CR] COMMIT | [^C] ABORT | ^K cmds | ? help".into());
```

(Replace the existing hint string that currently ends with `? help`.)

- [ ] **Step 3: Compile + unit tests**

Run:

```bash
cargo test -p omega-core command_palette -- --nocapture
cargo check --workspace
```

Expected: all command_palette tests PASS; workspace check succeeds.

- [ ] **Step 4: Manual smoke (if TUI available)**

Run the CLI (`cargo run -p omega-cli` or project’s usual entry), then:

1. Press `Ctrl+K` → palette opens with all commands  
2. Type `cle` → only Clear remains  
3. Enter → session clears (same as `/clear`)  
4. `Ctrl+K`, type `token cost` → Session cost selected; Enter shows token notice  
5. Empty editor, type `/` → palette opens with query `/`; Esc closes; editor still empty  
6. Start a stream (send a message) → `Ctrl+K` ignored until idle  
7. `/provider` via palette opens provider wizard  

If you cannot run interactive TUI in this environment, note that in the commit message body and rely on unit tests + `cargo check`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/omega-cli/src/main.rs
git commit -m "feat(cli): render command palette overlay and hint"
```

---

### Task 6: Final verification

**Files:**
- None (verification only)

- [ ] **Step 1: Full test + check**

Run:

```bash
cargo test -p omega-core -- --nocapture
cargo check --workspace
```

Expected: all tests PASS; check clean.

- [ ] **Step 2: Spec success criteria checklist**

Confirm against the design:

- [ ] Ctrl+K opens palette when idle  
- [ ] `/` on empty editor opens palette (no lone `/` left in buffer)  
- [ ] Keyword search narrows list; keyboard navigable  
- [ ] Search line shows live query  
- [ ] Enter runs same path as typed slash command  
- [ ] Esc closes without side effects  
- [ ] Unit tests pass; workspace checks  

- [ ] **Step 3: Final commit only if uncommitted fixes remain**

If verification required small fixes, commit them:

```bash
git add -A
git status
git commit -m "fix: command palette verification cleanups"
```

If tree is clean, skip.

---

## Self-Review

**1. Spec coverage**

| Spec requirement | Task |
|------------------|------|
| Dedicated `command_palette.rs` module | Task 1–3 |
| Registry of 7 slash commands + aliases + keywords | Task 1 |
| Multi-keyword AND filter | Task 1 |
| Name + description preview | Task 3 |
| Search line with live query | Task 3 |
| Ctrl+K open | Task 4 |
| Empty-buffer `/` open, seed `/`, no insert | Task 4 |
| Select → `handle_slash_command` | Task 4 |
| Esc / Ctrl+C close | Task 2 + 4 |
| Block open while streaming / provider panel | Task 4 `open_command_palette` |
| Dismiss help on open | Task 4 |
| Unit tests listed in design | Tasks 1–2 |
| Footer/hint optional polish | Task 5 |

**2. Placeholder scan:** No TBD/TODO; full code blocks for each implementation step.

**3. Type consistency:** `PaletteAction::Select(&'static str)` uses registry `id` strings; App calls `handle_slash_command(id)` which already matches on those ids. `CommandPaletteState::{open, close, recompute_filter}` names are consistent across tasks.

**Out of scope (do not implement):** free-text args, agent-tool browsing, mouse, fuzzy rank, help-overlay merge, populating `EditorState.suggestions`.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-22-command-preview-panel.md`. Two execution options:

**1. Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration  
**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints  

Which approach?
