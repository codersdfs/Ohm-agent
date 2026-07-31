# Ticket: Minimum CLI Parity with Codex CLI

## Question

Claude Code ships as a full VS Code extension. Codex CLI ships as a single
binary (`npm i -g codex`) with a chat loop — that's effectively its entire
surface area. **What is the absolute minimum set of CLI subcommands, tools, and
behaviors Omega Agent must ship to achieve functional parity with Codex CLI's
daily-driver workflow?**

### Current state of Omega CLI
The `CliAction` enum has only two variants: `Chat` and `ServeMcp`. The README
advertises `plan`, `code`, `build`, `review`, `gate`, `memory`, `config`,
`provider`, `models`, `repl` — **none exist** (ROADMAP P0-01 acknowledges this).

### What Codex CLI actually does
- `codex` → starts a chat loop with built-in read/write/edit/bash/grep/glob
- Respects `CODAI_API_KEY` or reads `~/.config/codex/auth.json`
- Respects `--model`, `--provider`, `--sandbox` flags
- Runs in a sandboxed Docker container by default (security boundary)
- `git diff` + apply on exit (workspace-aware edits)
- That's it. No MCP, no multi-agent, no repo map, no Gate.

### Decision needed
1. Does "parity" mean matching Codex CLI's feature set exactly, or matching
   its **workflow** (chat → propose edits → apply → git diff)?
2. Which of Omega's existing pieces (TUI chat, Gate, tools, memory, providers)
   must be wired into the binary for the minimum viable competitive product?
3. Is the Ratatui TUI a competitive advantage (rich terminal UX) or a
   liability (full-screen UI, harder to embed in VS Code terminal)?
4. Should Omega ship a binary at all (P1-07) or source-only for the parity path?

### Research needed
- Install `codex` and enumerate its actual CLI surface (`codex --help`,
  `codex --help --verbose`).
- Test Codex CLI's permission model: does it auto-apply edits or ask?
- Compare Omega's 14 built-in tools against Codex's tool set — which are
  equivalent, which does Omega lack, which does Omega have extra that matter?

## Resolution

### Answer: Minimum CLI parity = chat loop + read/write/edit/bash/grep/glob + git commit

### What Codex CLI actually ships (verified from official docs)

**CLI command surface (from https://developers.openai.com/codex/cli/reference):**

| Command | Type | Purpose |
|---------|------|---------|
| `codex [PROMPT]` | Stable | Launch the interactive TUI with optional initial prompt |
| `codex exec [PROMPT]` | Stable | Non-interactive / scripted run (stdout or JSONL output) |
| `codex apply [TASK_ID]` | Stable | Apply cloud-generated diff to local working tree |
| `codex review [--uncommitted\|--base\|--commit]` | Stable | Non-interactive code review of changes |
| `codex resume [SESSION]` | Stable | Resume a previous interactive session |
| `codex fork [SESSION]` | Stable | Fork a session into a new chat |
| `codex archive / unarchive / delete` | Stable | Session lifecycle management |
| `codex mcp (add\|remove\|list\|login\|logout)` | Stable | MCP server management (stdio + HTTP) |
| `codex mcp-server` | Stable | Run Codex as an MCP server over stdio |
| `codex sandbox` | Stable | Run commands in Codex-provided sandbox |
| `codex login / logout` | Stable | Authentication |
| `codex doctor` | Stable | Diagnostics |
| `codex features (list\|enable\|disable)` | Stable | Feature flag management |
| `codex completion` | Stable | Shell completion scripts |
| `codex update` | Stable | Self-update |
| `codex plugin` | Stable | Plugin marketplace |
| `codex cloud` | Experimental | Cloud task submission |
| `codex app-server` | Experimental | Local app server |
| `codex remote-control` | Experimental | Remote daemon management |
| `codex debug *` | Experimental | Debug subcommands |
| `codex execpolicy` | Experimental | Policy rule evaluation |

**Key insight: the ENTIRE daily-driver workflow is `codex` (interactive) or `codex exec` (headless).** Everything else (apply, review, resume, mcp, sandbox, login, doctor) is lifecycle/UX infrastructure. The core work — "agent uses tools to edit code" — is just the chat loop.

**Built-in tools (from the agent loop article):**
- `shell` — runs commands (note: in sandbox by default)
- `write` — write file
- `edit` — edit file (via `apply_patch` style diffs)
- `read` — read file
- `glob` — file pattern matching
- `grep` — search file contents
- MCP tools (dynamic, from registered servers)

That's **6 core built-in tools** (shell, write, edit, read, glob, grep). Everything else is MCP-provided or external.

**Permission model (from docs):**
- `--ask-for-approval -a`: `untrusted | on-request | never` — controls when Codex pauses for human approval before running commands
- `--sandbox -s`: `read-only | workspace-write | danger-full-access` — sandbox policy for model-generated shell commands
- `--dangerously-bypass-approvals-and-sandbox` / `--yolo` — skip all safety
- Default for `codex exec` (headless): sandboxed + auto-approval
- Default for `codex` (interactive): `workspace-write` sandbox + `on-request` approval

So Codex asks for permission before dangerous operations (shell commands that modify files outside workspace, git push, etc.) but auto-applies file edits within the sandbox.

**Authentication:** `codex login` (ChatGPT OAuth, API key, or access token). `CODX_API_KEY` or `~/.config/codex/auth.json`.

### What Omega needs for minimum viable parity

Omega currently has `CliAction::{Chat, ServeMcp}` — only two subcommands. The README claims 14 more (`plan`, `code`, `build`, `review`, `gate`, `memory`, `config`, `provider`, `models`, `repl`, `plan-status`, `plan-approve`) that don't exist.

**The minimum viable CLI for parity is:**

1. **`omega` (or `omega chat`)** → the existing Ratatui TUI chat loop ✅ (already works)
2. **`omega -m <model> -p <provider>`** flags → already supported via `load_provider_config()` ✅
3. **`omega exec` or `omega run`** → non-interactive mode (headless, stream to stdout, no TUI). The `stream_message_with_history` + `TerminalPrinter` emitter already exist in `chat.rs` — just needs a non-TUI entry point. ⚠️ (not yet a subcommand)
4. Built-in tools: Omega already has **14 tools** — more than Codex's 6:

| Omega Tool | Codex Equivalent | Category |
|---|---|---|
| `read` | `read` | FileOperations ✅ |
| `write` | `write` | FileOperations ✅ |
| `edit` | `edit` | FileOperations ✅ |
| `apply_patch` | (via edit) | DiffPatch ✅ (extra, useful) |
| `bash` | `shell` | CodeExecution ✅ |
| `grep` | `grep` | SearchQuery ✅ |
| `glob` | `glob` | SearchQuery ✅ |
| `git_status` | (via shell) | System ✅ (extra) |
| `git_diff` | (via shell) | System ✅ (extra) |
| `git_log` | (via shell) | System ✅ (extra) |
| `git_commit` | (via shell) | System ✅ (extra) |
| `web_fetch` | (via shell/curl) | WebNetwork ✅ (extra) |
| `todo` | (none) | AgentManagement ✅ (extra, useful for multi-step) |
| `ask_user` | (builtin prompt) | Communication ✅ (extra) |

Omega has **full parity + 8 extra tools** that matter (git operations, web_fetch, apply_patch, todo, ask_user).

**Critical gaps for Path A parity:**

| Gap | Status | Fix |
|-----|--------|-----|
| No `omega exec` headless mode | Missing | Wire `TerminalPrinter` emitter to a new `Exec` subcommand — 1 day |
| TUI auto-approves writes in permission_mode "on" | Bug | Fix `check_permission` to actually prompt in TUI — P1-05 |
| No sandbox model | Missing | Document `bash` tool permission mode; defer to P1-05 |
| MCP is HTTP-only stub | Missing | Defer to P1-02 (Path B) |
| No `AGENTS.md` loading | Already done | `project_instructions_snippet()` already reads AGENTS.md ✅ |
| No `git diff + apply on exit` | Partially | `git_diff` tool exists; `git_commit` tool exists ✅ |

### Decision answers

1. **"Parity means matching Codex CLI's feature set exactly, or matching its workflow?"** → Workflow. Codex CLI's daily-driver workflow is: `codex` → chat loop with tools → propose edits → git diff on exit. Omega already matches this in the TUI. The only critical missing piece is `omega exec` (headless/non-interactive), which Codex ships as `codex exec`.

2. **"Which of Omega's existing pieces must be wired into the binary?"** → Already wired: chat TUI, 14 tools, Gate (on write/edit via `execute_tool_inner`), session persistence, cancel, context compaction. Not wired: `exec` subcommand (needs TerminalPrinter path), permission prompts in TUI (auto-approves), provider routing (flat `load_provider_config`).

3. **"Is the Ratatui TUI an advantage or liability?"** → **Advantage for developers, liability for CI/embedding.** Codex ships both `codex` (TUI) and `codex exec` (headless). Omega should follow the same dual-surface pattern: TUI for interactive use (rich, where Omega's native Rust tools shine), headless `exec` for CI/automation (where Codex currently wins via npm installability). The dual-surface model is proven by Codex's success.

4. **"Should Omega ship a binary at all?"** → Yes. `cargo install` + GitHub release binaries (P1-07). Codex ships as npm package (`npm i -g codex`) + binary. Omega's Rust binary is simpler to distribute (single file, no Node runtime dependency).

### Bottom line

**Minimum viable CLI parity = 3 subcommands:**
1. `omega` / `omega chat` — TUI with Gate ✅ (exists)
2. `omega exec` — headless loop with stdout streaming ⚠️ (add 1 subcommand, wire TerminalPrinter)
3. `omega --help` / global flags (`-m`, `-p`, `-b`, `--session`, `--new-session`) ✅ (exists)

Plus fix the TUI permission prompt (P1-05) so `omega chat` doesn't auto-approve risky bash commands. The 14 built-in tools already exceed Codex's 6. No new tools needed for parity — they all exist and work.

## Type: research

## Status: open

## Assigned to: (unclaimed)
