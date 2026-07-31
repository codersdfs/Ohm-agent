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

## Type: research

## Status: open

## Assigned to: (unclaimed)
