# Omega Agent

**Alpha-stage AI coding assistant** — a single-agent Rust TUI with a deterministic,
zero-token quality gate. Currently shipping: interactive chat TUI (`omega`/`omega chat`),
headless agent mode (`omega exec`), pipeline subcommands (`omega plan|build|review|plan-status|plan-approve`), and a headless MCP server (`omega serve-mcp`) with both HTTP and stdio transports.

Built on the principles of [Harness Engineering](https://github.com/anomalyco/harness-engineering).

---

## Status

**Alpha:** the single-agent chat loop is functional and tested, but this is not
a production-ready tool. The build compiles and the included test suite passes
(200+ tests across `harness`, `omega-core`, and `omega-cli`), but features like
multi-agent pipelines, real MCP stdio transport, and binary releases are still
in development. See [todo.md](todo.md) for the full plan.

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
| Pipeline subcommands (`omega plan|build|review|plan-status|plan-approve`) | Working | Plan to Build to Review to Gate workflow, wired into CLI |
| Delta-only retry | ⚠️ Experimental | Exists in pipeline code behind the env flag |
| Real embeddings | Working | `memory` crate uses fastembed (ONNX) for semantic search |
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
`omega plan`, `omega build`, `omega review`, `omega plan-status`,
`omega plan-approve`, `omega code <query>` (repo symbol search),
`omega provider`, `omega models`, `omega config`, `omega memory`,
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

## Roadmap

The forward-looking plan (vision, current state, what to do tomorrow, and the four phases from v0.1.0 to public launch) is in [`.scratch/project-roadmap/`](.scratch/project-roadmap/README.md). Read `tasks/tomorrow.md` first; it is the day's plan.

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
| `omega-table`     | `src-tauri/crates/omega-table/`  | Working   | `.otable` progressive-load format + LRU cache |
| `providers`       | `src-tauri/crates/providers/`    | Working   | `LlmProvider` trait + 14 providers (5 native + OpenAI-compatible) |
| `memory`          | `src-tauri/crates/memory/`       | Working   | Hermes memory: SQLite + FTS5 + embeddings, 3-layer (session/project/user) |
| `mcp`             | `src-tauri/crates/mcp/`          | Working   | MCP JSON-RPC client + skills registry, stdio + HTTP transports |
| `mcp-server`      | `src-tauri/crates/mcp-server/`   | Working   | Headless MCP server (HTTP + stdio), exposes built-in tools |
| `tool-harness`    | `src-tauri/crates/tool-harness/` | Working   | 14 built-in tools (read/write/edit/bash/grep/glob/etc) |
| `taste`           | `src-tauri/crates/taste/`        | Working   | Taste rules / pattern database |
| `ratata`          | `src-tauri/crates/ratata/`       | Unused    | Legacy (pre-Ratatui) TUI components |
﻿

## Release process

Cutting a release is one command:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The `release.yml` workflow matrix-builds the `omega` binary on Linux, macOS (x86_64 + aarch64), and Windows, signs each artifact with [cosign](https://docs.sigstore.dev/) keyless OIDC, generates an SPDX SBOM, computes SHA256SUMS, and publishes a GitHub Release with a conventional-commits changelog generated by [git-cliff](https://git-cliff.org/) using `cliff.toml`.

To dry-run the build without publishing, trigger the workflow manually from the Actions tab.

Verification example:

```sh
cosign verify-blob \
  --certificate omega-x86_64-unknown-linux-gnu.tar.gz.cert \
  --signature   omega-x86_64-unknown-linux-gnu.tar.gz.sig \
  --certificate-identity-regexp "https://github.com/.*" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  omega-x86_64-unknown-linux-gnu.tar.gz
```
