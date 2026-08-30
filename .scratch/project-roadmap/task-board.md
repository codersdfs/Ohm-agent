# Task board

The single source of truth for "what is next". Migrated from
`../tool-calling-cleanup/issues/` on 2026-08-30. Add new tasks here, not
to ad-hoc scratchpads. Original ticket files are referenced for full
discussion; do not duplicate the long-form text.

## Priority buckets

- **P0** — blocks Phase exit. Do first.
- **P1** — needed for the next phase's entry condition. Do this phase.
- **P2** — design debt. Pick up between P0/P1 work, or defer.
- **P3** — polish, nice-to-have. Don't actively schedule.

## P0 — must-do this phase

| # | Title | Source | Est. |
|---|---|---|---|
| — | Add `.github/workflows/ci.yml` | new (Day 1) | 2h |
| — | Fix `RunOutcome::BudgetExhausted` constructor call site | ticket #11 finding | 1h |
| — | Add Gate eval to CI with regression guard | new (Day 1) | 4-6h |
| — | Cut `v0.1.1` | new (Day 1) | 30m |
| — | Negative-knowledge loop: frequency detector | new (Day 3) | 4h |
| — | Negative-knowledge loop: auto-promotion to rules DB | new (Day 4) | 6h |

## P1 — design debt, this phase

| # | Title | Source | Est. |
|---|---|---|---|
| 08 | Make `HookContext` required on `ExecutionPipeline::new` (default-trap fix) | `tool-calling-cleanup/issues/08` | 1-2h |
| 10/27 | `is_read_only` vs `check_permissions` — pick one | `tool-calling-cleanup/issues/10` and `27` | 1h (design) + 2h (impl) |
| 14 | Split `chat.rs` by concern (commands/loop/state) | `tool-calling-cleanup/issues/14` | 2-3h |
| 22 | Concurrency-safe required metadata for tool registry | `tool-calling-cleanup/issues/22` | 2h |

## P2 — design debt, defer if needed

| # | Title | Source | Est. |
|---|---|---|---|
| 01 | Tool concurrency safety: pick one source of truth | `tool-calling-cleanup/issues/01` | 2h |
| 02 | Surface tool-result truncation to the LLM (already done in some paths) | `tool-calling-cleanup/issues/02` | 1h |
| 03 | Delete dead mcp-bridge code (verify before deleting — see #32) | `tool-calling-cleanup/issues/03` and `32` | 1h |
| 05 | `hook_inject_messages` actually honored | `tool-calling-cleanup/issues/05` | 2h |
| 07 | Add `jsonschema` crate for input validation | `tool-calling-cleanup/issues/07` | 2h |
| 09 | Unify mcp wire types | `tool-calling-cleanup/issues/09` | 2-3h |
| 12/28 | `edit` vs `apply_patch` — pick one | `tool-calling-cleanup/issues/12` and `28` | 1h (design) + 3h (impl) |
| 13 | Collapse stdio `Content-Length` impls | `tool-calling-cleanup/issues/13` | 1h |
| 15 | Drop OpenAI-local `toolcall` structs | `tool-calling-cleanup/issues/15` | 1h |
| 17 | Remove dead `McpServerConfig::default` | `tool-calling-cleanup/issues/17` | 30m |
| 19 | Source tool populated centrally (not per-request) | `tool-calling-cleanup/issues/19` | 2h |
| 20/31 | `Hook` trait `self` vs `mut self` decision | `tool-calling-cleanup/issues/20` and `31` | 1h (design) |
| 21 | Verify "brackets" naming is honest | `tool-calling-cleanup/issues/21` | 30m |
| 23 | Flip `tool-harness` mcp dep arrow | `tool-calling-cleanup/issues/23` | 1h |
| 24 | Update AGENTS.md mcp claims | `tool-calling-cleanup/issues/24` | 30m |

## P3 — defer to v1.0+

| # | Title | Source | Est. |
|---|---|---|---|
| 16 | MCP HTTP transport decision (axum vs hand-rolled) | `tool-calling-cleanup/issues/16` and `29` | 4h |
| 18/30 | Tilde expansion in path-rewriter hook (already done in some paths; verify) | `tool-calling-cleanup/issues/18` and `30` | 1h |

## Recently closed (last 7 days)

| Date | Title | Commit |
|---|---|---|
| 2026-08-30 | Ticket #11 acceptance — gate, budget, whitelist tests | `2618f5e` |
| 2026-08-30 | v0.1.0 release cut (with 10 pipeline fixes) | `aaab474` |
| 2026-08-30 | Release pipeline postmortem | `177a25c` |
| 2026-08-29 | Tickets #04 and #25 — single subagent dispatch path | (pre-summary) |

## How to add a new task

1. Pick a P-bucket. If unsure, default to P2.
2. One-line title. The longer explanation goes in a separate file under
   `tasks/` or `reference/`, linked from the table.
3. Estimate. Be honest; 1-2h for small, half-day for medium, full day for
   large. Multi-day tasks should be split before they go on the board.
4. If the task is from a discussion or a user message, link the source.
5. When the task is done, move it to "Recently closed" with the commit hash.
6. Old "Recently closed" entries (> 30 days) move to `reference/history.md`.
