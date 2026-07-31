# Wayfinder Plan: Can Omega Agent Compete with Claude Code & Codex?

## Overview

This directory contains a Wayfinder plan that maps the decisions needed to close the
gap between Omega Agent (`src-tauri/`) and its competitors (Claude Code, Codex CLI).

## How to Use

- **`map.md`** — the canonical map: Destination, Notes, Decisions-so-far, fog, out-of-scope.
- **`PLAN_SUMMARY.md`** — the full gap analysis that grounds every ticket.
- **`tickets/`** — each ticket is one decision question (one 100K-token session).
- **`tracker.json`** — ticket status, assignments, and blocking edges (local-markdown
  tracker convention: `blocked_by` arrays wire dependencies).

### Work Through the Map
1. **Claim** a frontier ticket (set `assigned_to` + `status: in_progress` in tracker.json).
2. **Resolve** it — use `/research` for facts, `/grilling` for tradeoffs.
3. **Record** the answer on the ticket (append a `## Resolution` section).
4. **Close** it (`status: closed`) and append a one-line gist + link to the map's
   "Decisions so far".
5. **Graduate** any fog that is now specifiable into new tickets; wire blocking edges.

### Frontier (takeable now — open, unblocked, unclaimed)
1. [What is Omega's true competitive moat?](tickets/001-moat.md) — `research`
2. [Ship-now vs invest-in-moat: strategic path](tickets/002-strategic-path.md) — `grilling`
3. [Minimum CLI parity with Codex CLI](tickets/003-minimum-cli-parity.md) — `research`
4. [Honest README rewrite scope](tickets/004-honest-readme.md) — `research`
5. [Is the multi-agent pipeline a moat or liability?](tickets/005-multiagent-moat.md) — `grilling`
6. [Fix entropy compile error or quarantine it?](tickets/006-entropy-fix.md) — `task`

## Destination

Decide the route by which `src-tauri/` can genuinely outperform or compete with
Claude Code and Codex — and chart every key decision and research ticket needed to
close the gap from the current broken prototype to a shipping, competitive-quality
coding agent.
