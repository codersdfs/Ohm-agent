# omega-cli

A fast, keyboard-driven terminal chat app powered by AI, inspired by Claude Code's terminal experience.

## Quick Start

```sh
cd omega-cli
cargo run
```

## Keybindings

### Normal Mode (default)

| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `a` | Enter insert mode at end of line |
| `o` | Open new line below, enter insert mode |
| `O` | Open new line above, enter insert mode |
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `PgUp` / `PgDn` | Page scroll |
| `G` | Scroll to bottom |
| `/` | Open input with `/` prefix (command mode) |
| `Esc` | Quit |
| `Ctrl+C` / `Ctrl+Q` | Force quit |
| `Ctrl+L` | Clear conversation |

### Insert Mode

| Key | Action |
|-----|--------|
| `Enter` | Submit message (or execute `/command`) |
| `Shift+Enter` | New line in input |
| `Tab` | Insert 4 spaces |
| `Esc` | Return to normal mode |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |
| `←` / `→` | Move cursor |
| `↑` / `↓` | Move cursor between lines |
| `Home` / `End` | Move to start/end of line |

## Slash Commands

| Command | Action |
|---------|--------|
| `/clear` | Clear conversation history |
| `/help` | Show available commands |
| `/exit` | Quit the app |

## Architecture

```
src/
  main.rs      — Terminal init, panic hook, app launch
  app.rs       — App state, input handling, event dispatch
  ui.rs        — ratatui rendering, theme, layout
  message.rs   — Message types and history
  backend.rs   — ChatBackend trait + MockBackend
```

## Backend

The app uses a trait-based backend abstraction. The default `MockBackend` cycles through
pre-written responses to demonstrate streaming behavior. To connect a real LLM:

1. Implement the `ChatBackend` trait in `backend.rs`
2. Replace `MockBackend` in `App::new()` with your implementation
3. The `stream_chat` method receives the full message history and a channel to send chunks

## Requirements

- Rust 1.75+
- A terminal that supports raw mode and alternate screen
