# Command Preview Panel (Command Palette) Design

**Date:** 2026-07-22  
**Status:** Approved for implementation planning  
**Product:** Omega Agent (TUI)

## Problem

Users discover slash commands only by typing them correctly or reading a short `/help` notice. The editor already has a suggestions stub (`EditorState.suggestions`, `render_suggestions`) that is never populated. There is no searchable, keyboard-driven way to browse and run existing commands with a readable description.

## Goal

Ship a **command palette** (command preview panel) that:

- Opens with **Ctrl+K**, or by typing **`/` when the editor buffer is empty**
- Lists **existing slash commands only** (v1)
- Shows **name + one-line description** for the selected/matching commands
- **Runs the selected command immediately** on Enter
- Reuses the existing `handle_slash_command` execution path (single source of truth)

## Non-goals (v1)

- Free-text args inside the palette (e.g. typing a model name after `/model`)
- Agent tool browsing (read/write/bash as palette entries)
- Mouse support
- Fuzzy ranking, recents, or frequency sorting
- Merging the F1/`?` help overlay into the palette
- Populating the old editor `suggestions` popup as the primary UX

## Architecture

Approach: **dedicated palette module**, mirroring `provider_panel`.

| Piece | Responsibility |
|-------|----------------|
| `omega-core/src/tui/command_palette.rs` | Static command registry, filter, key handling, render |
| `omega-core/src/tui/mod.rs` | Export `command_palette` |
| `omega-cli/src/main.rs` | Own visibility flag; open/close routing; on select call `handle_slash_command` |

```text
┌─────────────────────────────────────────┐
│  App (omega-cli)                        │
│   show_command_palette: bool            │
│   command_palette: CommandPaletteState  │
│                                         │
│   open ──► palette.reset() / open()     │
│   keys ──► command_palette::handle_key  │
│   Select(id) ──► handle_slash_command   │
│   Close ──► hide palette                │
└─────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────┐
│  command_palette (omega-core tui)       │
│   COMMANDS: static registry             │
│   query, selected, filtered indices     │
│   render centered modal                 │
└─────────────────────────────────────────┘
```

`handle_slash_command` remains the only place that implements `/clear`, `/model`, etc. The palette never duplicates those side effects.

## Components

### Command registry entry

```rust
pub struct CommandEntry {
    pub id: &'static str,           // canonical slash id, e.g. "/clear"
    pub label: &'static str,        // short display name, e.g. "Clear session"
    pub aliases: &'static [&'static str],
    pub description: &'static str,  // one-line preview text
}
```

### v1 catalog

Must match what `handle_slash_command` accepts today:

| id | aliases | label | description |
|----|---------|-------|-------------|
| `/help` | `/?`, `/h` | Help | Show available commands |
| `/clear` | `/cls` | Clear session | Clear transcript and session |
| `/tools` | | List tools | List available agent tools |
| `/model` | | Choose model | Open model picker for current provider |
| `/provider` | `/providers`, `/p` | Choose provider | Open provider configuration wizard |
| `/cost` | | Session cost | Show session token usage |
| `/exit` | `/quit` | Quit | Quit Omega |

### State

```rust
pub struct CommandPaletteState {
    pub visible: bool,
    pub query: String,
    pub selected: usize,
    pub filtered: Vec<usize>, // indices into COMMANDS
}
```

### Filter

- Case-insensitive substring match over `id`, `label`, each alias, and `description`
- Empty query → all commands
- After recompute, clamp `selected` to `filtered.len().saturating_sub(1)` (or 0 if empty)

### UI

- Centered modal overlay on the main frame area
- Approx width 48 columns (clamped to terminal width − 4)
- Height: filtered rows + chrome (title/border), capped (~14 rows)
- Title: ` commands ` (same family as existing suggestions popup)
- Each row: `id` + `label`; selected row uses accent/bold; description shown for the selected row (inline second line or footer strip)
- Empty filter: show `No matching commands`
- Visual tokens: reuse `theme` colors (`PRIMARY`, `ACCENT`, `DIM`, `FG`, `SURFACE_HIGH` / `BG`) consistent with `help.rs` and `editor::render_suggestions`

### Key handling (palette open)

| Key | Behavior |
|-----|----------|
| Printable chars | Append to `query`, recompute filter |
| Backspace | Delete last char of `query` |
| ↑ / ↓ | Move selection |
| Tab / Shift+Tab | Move selection (wrap) |
| Enter | `PaletteAction::Select(id)` if a row is selected; no-op if empty filter |
| Esc | `PaletteAction::Close` |
| Ctrl+C | `PaletteAction::Close` (App may still treat global quit separately only when palette is closed; while open, prefer Close) |

No mouse in v1.

### Palette actions

```rust
pub enum PaletteAction {
    None,
    Close,
    Select(&'static str), // command id, e.g. "/clear"
}
```

## Open / close rules

1. **Ctrl+K** opens the palette when:
   - not streaming
   - provider panel is not open
2. **`/` with empty editor buffer** opens the palette under the same constraints, seeds `query` with `"/"`, and does **not** insert `/` into the editor.
3. Opening the palette dismisses the help overlay if it was open (`show_help = false`).
4. **Esc / Close** hides the palette; editor buffer is left unchanged.
5. While the palette is open, it owns key handling (same exclusivity pattern as `show_provider_panel`).
6. Palette cannot open mid-stream.

## Data flow

```text
User presses Ctrl+K (or empty-buffer /)
  → App.show_command_palette = true
  → palette.open() / reset(+ optional seed "/")

User types / navigates
  → command_palette::handle_key updates query/selected

User presses Enter on a row
  → PaletteAction::Select("/clear")
  → App.show_command_palette = false
  → App.handle_slash_command("/clear")

User presses Esc
  → PaletteAction::Close
  → App.show_command_palette = false
```

`/model` and `/provider` continue to open the existing provider panel via `handle_slash_command` — the palette only triggers that path.

## Error and edge cases

| Case | Behavior |
|------|----------|
| Streaming | Open shortcuts ignored |
| Provider panel open | Ctrl+K ignored (panel exclusivity) |
| Empty filter results | Message in modal; Enter no-op |
| Unknown id | Should not occur; `handle_slash_command` already emits an error notice if it does |
| `/model` / `/provider` while streaming | Palette not openable while streaming |
| Help open + palette open | Opening palette closes help |

## Testing

Unit tests in `command_palette.rs` (or adjacent `#[cfg(test)]` module):

1. `filter("")` returns all 7 commands  
2. `filter("cle")` matches `/clear`  
3. `filter("cls")` matches `/clear` via alias  
4. `filter("zzz")` is empty  
5. Selection clamps when filter shrinks  
6. `handle_key(Enter)` with a selection returns `Select` with the correct id  
7. `handle_key(Esc)` returns `Close`

Verification commands:

```bash
cargo test -p omega-core
cargo check --workspace
```

No full TUI integration harness required for v1.

## File impact summary

| File | Change |
|------|--------|
| Create `src-tauri/crates/omega-core/src/tui/command_palette.rs` | Registry, state, filter, keys, render, tests |
| Modify `src-tauri/crates/omega-core/src/tui/mod.rs` | `pub mod command_palette;` |
| Modify `src-tauri/crates/omega-cli/src/main.rs` | App fields, open shortcuts, key exclusivity, render overlay, select → `handle_slash_command` |

Optional later (not v1): wire editor `suggestions` as a lightweight fallback, or merge help overlay into palette.

## Success criteria

- [ ] Ctrl+K opens palette when idle
- [ ] `/` on empty editor opens palette (does not leave a lone `/` in the buffer)
- [ ] Filtering narrows the list; selection keyboard-navigable
- [ ] Enter runs the same behavior as typing the slash command
- [ ] Esc closes without side effects
- [ ] Unit tests above pass; workspace still checks
