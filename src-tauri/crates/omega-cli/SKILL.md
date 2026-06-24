---
name: omega-cli
description: A fast, keyboard-driven terminal chat app powered by AI, built with Ratatui + Crossterm. Supports real LLM providers (Anthropic, OpenAI, Ollama, Google, Mistral) via the providers crate, code quality evaluation via the harness (Gate) crate, and tool execution via the MCP crate.
---

# omega-cli

Terminal-native AI chat interface with vim-style keybindings, streaming responses, and pluggable backends. Located at `src-tauri/crates/omega-cli/` inside the omega-agent workspace.

## Architecture

```
src-tauri/crates/omega-cli/src/
├── main.rs                 # Entry: terminal init, panic hook, launch App
├── app.rs                  # App struct — owns state, drives tick/render loop
├── ui.rs                   # ratatui rendering — dark theme, chat/input/status
├── event.rs                # Background crossterm poller → UnboundedReceiver
├── message.rs              # Message, MessageSender, MessageHistory with scroll
├── backend/
│   ├── mod.rs              # ChatBackend trait + MockBackend
│   └── providers_backend.rs # ProviderBackend — wraps providers::LlmProvider
└── commands/
    ├── mod.rs              # Slash command dispatcher
    ├── gate.rs             # /gate, /gate-score — harness code quality evaluation
    └── mcp.rs              # /connect, /skills — MCP client integration
```

## Keybindings

### Normal Mode
| Key | Action |
|-----|--------|
| `i` / `a` | Enter insert mode |
| `o` / `O` | Open new line (insert mode) |
| `j`/`k` or `↑`/`↓` | Scroll message history |
| `PgUp`/`PgDn` | Page scroll |
| `G` | Scroll to bottom |
| `/` | Enter command mode |
| `Esc` | Quit |
| `Ctrl+C` / `Ctrl+Q` | Force quit |
| `Ctrl+L` | Clear conversation |

### Insert Mode
| Key | Action |
|-----|--------|
| `Enter` | Submit message or execute `/command` |
| `Shift+Enter` | New line in input |
| `Tab` | Insert 4 spaces |
| `Esc` | Return to normal mode |

## Slash Commands

| Command | Description |
|---------|-------------|
| `/help` | Show all commands |
| `/provider [name]` | Show or set provider (mock, anthropic, openai, local, ...) |
| `/model [name]` | Show or set model name |
| `/api-key <key>` | Set API key (masked echo) |
| `/base-url [url]` | Show or set base URL |
| `/config` | Print current configuration |
| `/connect <url>` | Connect to MCP server |
| `/skills` | List registered MCP skills |
| `/gate <path>` | Run Gate evaluation on file |
| `/gate-score <path>` | Show Gate score only |
| `/clear` | Clear conversation |
| `/exit` | Quit |

## Backend Architecture

The `ChatBackend` trait defines a single method:

```rust
async fn stream_chat(&self, history: &[Message], tx: UnboundedSender<String>, cycle_idx: usize) -> Result<(), String>;
```

Two implementations:
- `MockBackend` — cycles through pre-written responses with simulated streaming
- `ProviderBackend` — wraps `providers::LlmProvider`, streams real LLM responses via `StreamChunk`

Provider is switched at runtime via `/provider` and `/api-key` commands. The App stores `Option<ProviderConfig>` and rebuilds the backend on switch.

## Adding a New Backend

1. Implement `ChatBackend` in a new file under `backend/`
2. Add a re-export in `backend/mod.rs`
3. Add a `/` command in `commands/mod.rs` to switch to it
4. Call `app.switch_backend(...)` in the command handler

## Adding a New Command

1. Create a handler function in `commands/<module>.rs`
2. Register it in `commands/mod.rs` match statement
3. The handler receives `&mut App` and can push messages, switch providers, etc.

## Build & Run

```sh
cd src-tauri
cargo run -p omega-cli
```

## Dependencies

- `providers` — LLM provider trait + 14 provider implementations
- `harness` — Gate code quality evaluation (structural/taste/golden/repeated)
- `mcp` — MCP JSON-RPC client + skills registry
- `ratatui` 0.28 + `crossterm` 0.28 — terminal UI
- `tokio` — async runtime
- `clap` — CLI arg parsing (currently unused, all config via `/commands`)

## Conventions

- `App` owns all mutable state; UI renders from `&App`
- Backend tasks are spawned via `tokio::spawn` and communicate via `UnboundedSender<String>`
- Messages are appended to `MessageHistory` and displayed on next draw
- Slash commands are dispatched in `commands/mod.rs`
- MCP skill files use `.mcp.json` extension
- Skills resolution: `$OMEGA_CLI_SKILLS_DIR` → `./skills/` → `../skills/` (relative to binary)
