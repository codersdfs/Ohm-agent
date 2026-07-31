# Omega Agent

**Alpha-stage AI coding assistant** — a single-agent Rust TUI with a deterministic,
zero-token quality gate. Currently shipping: interactive chat TUI (`omega`/`omega chat`),
headless agent mode (`omega exec`), and a headless MCP server (`omega serve-mcp`).

Built on the principles of [Harness Engineering](https://github.com/anomalyco/harness-engineering).

---

## Status

**Alpha:** the single-agent chat loop is functional and tested, but this is not
a production-ready tool. The build compiles and the included test suite passes
(200+ tests across `harness`, `omega-core`, and `omega-cli`), but features like
multi-agent pipelines, real MCP stdio transport, and binary releases are still
in development. See [ROADMAP.md](ROADMAP.md) for the full plan.

### What works today

| Capability | Status | Notes |
|---|---|---|
| Interactive TUI chat (`omega chat`) | ✅ Working | Ratatui full-screen UI with streaming, tools, session restore |
| Headless agent (`omega exec`) | ✅ Working | Non-TUI mode for CI and scripting — added in this release |
| 14 built-in tools | ✅ Working | read, write, edit, apply_patch, bash, grep, glob, git_status, git_diff, git_log, git_commit, web_fetch, todo, ask_user |
| Provider abstraction | ✅ Working | OpenAI, Anthropic, Google, Groq, and 9 more — unified `LlmProvider` trait, streaming + non-streaming |
| Provider panel (in-TUI) | ✅ Working | Switch provider/model at runtime via Ctrl+P |
| Deterministic Quality Gate | ✅ Working | Rust-native lint + rule engine catches violations in microseconds, zero LLM tokens |
| Negative Knowledge Loop | ⚠️ Partial | Recurring failures (freq >= 3) track in DB; full system-prompt injection coming |
| Session persistence | ✅ Working | SQLite JSONL, resume with `--session ID` or `--new-session` |
| Token cost tracking | ✅ Working | Session token counts shown in TUI and `omega exec --show-tokens` |
| MCP server (`omega serve-mcp`) | ✅ Working | HTTP JSON-RPC, exposes built-in tools |
| Tilde (~) path expansion | ✅ Working | All tool paths expand `~` correctly |
| `AGENTS.md` loading | ✅ Working | Reads project instructions from `AGENTS.md`, `.omega/instructions.md`, `CLAUDE.md` |

### Not yet shipping (planned)

| Capability | Status | Notes |
|---|---|---|
| Multi-agent pipeline (Plan -> Build -> Review -> Gate) | ⚠️ Experimental | Behind `OMEGA_EXPERIMENTAL_PIPELINE=1` env var; not wired into CLI subcommands |
| Delta-only retry | ⚠️ Experimental | Exists in pipeline code behind the env flag |
| Real embeddings (ONNX/fastembed) | ⚠️ Stub | `memory` crate uses n-gram hashing (256-dim) for semantic search. ONNX engine exists behind `onnx-embed` feature flag, not built by default. |
| MCP stdio client | ❌ Not started | `mcp` crate is HTTP-only JSON-RPC; stdio transport planned for Phase 1 |
| Provider routing with health checks | ⚠️ Partial | `providers/src/router.rs` exists but is not wired into the CLI's `load_provider_config()` |
| Repo map / symbol index | ❌ Not started | Planned via tree-sitter indexing |
| Binary releases | ❌ Not started | Build via `cargo build -p omega`; release binaries in Phase 1 |
| Sandboxing | ❌ Not started | Permission dialog exists; true sandboxing planned |
| Entropy GC (multi-language) | ⚠️ Rust-only | Drift scanner runs Rust `rustfmt`/`clippy`; multi-language GC in Phase 2 |
| Delta context cache | ❌ Not started | Planned; not yet implemented |

### Honest positioning

Omega Agent is **not** Claude Code or Codex CLI yet. It is an alpha-quality Rust
coding agent whose differentiating strength is a **deterministic, zero-token
quality gate** — structural, taste, golden, and repeated-violation rules checked
in-process via Rust, with a negative-knowledge feedback loop that tracks recurring
errors. This gate runs on every file edit/write during the agent loop, before
output is accepted, giving it a quality-first workflow that neither Claude Code
nor Codex provides by default.

The project is early-stage. Use it to explore the Gate's quality benefits; do
not rely on it for high-stakes work without careful review.

---

## Quick Start

```bash
# Build and run the interactive TUI
cargo run -p omega -- chat

# Headless mode for CI / scripting (no TUI, streams to stdout)
cargo run -p omega -- exec "explain the architecture of src/main.rs"
cargo run -p omega -- exec --permission-mode off --show-tokens "write tests for the Gate engine"

# Run the MCP server (exposes built-in tools over HTTP)
cargo run -p omega -- serve-mcp --port 3100
```

### Configuration

- **Default**: reads `OMEGA_API_KEY` env var; if unset, prompts interactively
- **Config**: `~/.config/omega-agent/config.json` (via `directories` crate)
- **Local provider**: Ollama at `http://127.0.0.1:11434` by default
- **Permission modes**: `off` (auto-approve), `on` (prompt per tool, default
  for `omega chat`), `strict` (deny all mutations)

```bash
# Using OpenAI
export OMEGA_API_KEY=sk-…
omega exec "write a hello world program in Rust"

# Using local Ollama
omega chat -p local -m llama3.1:8b

# Resume a session
omega chat --session <id> --new-session   # or skip --new-session to auto-resume last
```

Available CLI commands: `omega chat` (default), `omega exec`, `omega serve-mcp`,
`omega --help`, `omega <command> --help`.

---

## Why Omega Agent?

| Problem | Omega's Approach |
|---------|------------------|
| LLMs miss quality issues | **Deterministic Gate in Rust** — catches structural/taste/golden/repeated violations in microseconds, no LLM tokens spent |
| Same mistakes repeat | **Negative knowledge loop** — errors at frequency >= 3 are tracked and auto-promoted to linter rules |
| No learning mechanism | **Entropy GC** — daily drift scans with automatic remediation (experimental, Rust-only) |
| Quality not enforced | **Gate-first workflow** — every write/edit passes through the Gate before reaching your repo |

---

## Architecture

```
┌───────────────────────────────────────────────────────────────────┐
│                      Omega CLI ("omega")                          │
│                                                                   │
│  ┌──────────────┐     ┌──────────────────────────────────┐       │
│  │  TUI (Ratatui) │◄──►│  Chat Loop (omega-core)          │       │
│  │  or headless   │     │                                  │       │
│  │  (omega exec)  │     │  ┌─────────────┐   ┌──────────┐  │       │
│  └──────────────┘     │  │  Tools        │   │ Providers│  │       │
│                         │  │ (14 built-in) │   │ (14 LLM) │  │       │
│                         │  └──────┬──────┘   └────┬─────┘  │       │
│                         │         │                 │         │       │
│                         │  ┌──────┴──────────────┐  │       │       │
│                         │  │ Gate Engine         │  │       │       │
│                         │  │ (harness, Rust)     │  │       │       │
│                         │  └─────────────────────┘  │       │       │
│                         └───────────────────────────┘           │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │  Experimental (OMEGA_EXPERIMENTAL_PIPELINE=1)            │    │
│  │  Plan -> Build -> Review -> Gate -> Retry                │    │
│  └──────────────────────────────────────────────────────────┘    │
└───────────────────────────────────────────────────────────────────┘
```

### Workspace Crates

| Crate | Path | Status | Purpose |
|-------|------|--------|---------|
| `omega` | `src-tauri/crates/omega-cli/` | ✅ Shipping | CLI binary — `chat`, `exec`, `serve-mcp` subcommands |
| `omega-core` | `src-tauri/crates/omega-core/` | ✅ Shipping | Core library — `AppState`, chat loop, commands, TUI, pipeline |
| `harness` | `src-tauri/crates/harness/` | ✅ Shipping | **Mechanized Gate** — rules engine, pattern matching, scoring, linter integration |
| `entropy` | `src-tauri/crates/entropy/` | ⚠️ Alpha | Drift scanner + GC (Rust-only) |
| `omega-table` | `src-tauri/crates/omega-table/` | ✅ Compiles | `.otable` progressive-load format (not yet 
