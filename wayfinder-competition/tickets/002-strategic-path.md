# Ticket: Ship-Now vs Invest-in-Moat — Which Strategic Path?

## Question

Given the gap analysis (see PLAN_SUMMARY.md), Omega Agent faces a fork:

- **Path A (Ship-now):** Strip the overclaiming, fix the compile error, ship
  just the single-agent chat TUI + Gate as an honest alpha. Compete on
  "deterministic quality gate in Rust" as the one differentiator. Skip
  multi-agent pipeline, real MCP client, embeddings, binary releases for now.
  Target: working `cargo install` + eval harness showing gate improves quality.

- **Path B (Invest-in-moat):** Execute the full Phase 1 + Phase 2 ROADMAP —
  real MCP stdio client, repo map with tree-sitter indexing, real embeddings,
  provider routing with health checks, working multi-agent pipeline, binary
  releases, eval harness. Target: 60%+ multi-file task success, gate FP < 15%.

### Tradeoffs to weigh
| | Path A | Path B |
|---|---|---|
| Time to first competitive signal | 2-3 weeks | 6-12 months |
| Engineering effort | ~3 crates fixed + docs + evals | ~12 roadmap tickets, multiple crates |
| Risk | Low (just honesty + fix) | High (many moving parts, no guarantee moat wins) |
| Market positioning | "alpha agent with a Rust quality gate" | "full-featured AI coding agent" |
| Competitive threat | Claude Code will lap it on features | Claude Code may ship Gate-equivalent first |

### Decision criteria
- Which path maximizes the **probability that Omega becomes a viable daily-
  driver for at least some developers within 12 months**?
- Is the Gate moat strong enough to defend against Claude Code's vastly larger
  engineering team shipping faster?
- Does Path A risk "giving up" the multi-agent + 14-provider differentiation
  prematurely, or does Path B risk burning runway on architecture while
  Claude Code eats the market?

### Research needed
- Estimate engineering effort for Path B using ROADMAP P1-01 through P2-07
  (count crates touched, test requirements, dependency on external tools like
  `tree-sitter` grammars, `fastembed` model downloads).
- Estimate Path A effort (entropy fix, README rewrite, eval harness skeleton
  with 5 tasks, binary build script).
- Look at Aider's trajectory: when did it become competitive? What was its
  minimal feature set at v1.0?
- Market window: how fast is Claude Code iterating on MCP/hooks/quality gates?

## Type: grilling

## Status: open

## Assigned to: (unclaimed)
