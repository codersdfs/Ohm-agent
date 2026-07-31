# Ticket: Honest README Rewrite — Competitive Positioning Scope

## Question

The current README claims Omega Agent "outperforms other coding agents," has
"14 providers," catches "60-80% of violations," ships "Plan/Build/Review"
subcommands, uses "fastembed" for real embeddings, and more — most of which
is unimplemented or broken (see PLAN_SUMMARY.md gap analysis).

ROADMAP P0-01 is the existing ticket for "Honest README rewrite." But this
competition wayfinder needs a scoped answer: what is the honest positioning
Omega should claim today, and what should it defer to Phase 1/Phase 2?

### Decision needed
1. **What can Omega honestly claim today?** Examples: "single-agent Rust TUI
   with a deterministic quality gate — alpha quality, broken build currently."
2. **What competitive claims should the README make** even if the moat is real
   but unproven? E.g., "the only coding agent with a Rust-native mechanical
   gate" — only if true and defensible.
3. **What must the README explicitly say is not done?** (entropy GC, multi-
   agent pipeline, real MCP client, real embeddings, binary releases,
   14-subcommand CLI)
4. **How does the positioning change** depending on Path A vs Path B from
   ticket #002? (This ticket blocks on #002 for the final positioning, but
   the honest-claims list is independent.)

### Research needed
- Audit every README claim against actual compiled code (done partially in
  PLAN_SUMMARY.md — complete it).
- Survey how competitor READMEs position themselves honestly (Aider, OpenHands,
  Cursor CLI) — do they overclaim? What works?
- Check: is there any independent measurement of Codex-s or Claude Code-s
  quality gate or eval numbers we can reference?

## Type: research

## Status: closed

## Assigned to: omega-wayfinder
