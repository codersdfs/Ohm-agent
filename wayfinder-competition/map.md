# Wayfinder Map: Can Omega Agent Compete with Claude Code & Codex?

## Destination

Decide the route by which `src-tauri/` (Omega Agent) can genuinely outperform or
compete with Claude Code and Codex — and chart every key decision and research
ticket needed to close the gap from the current broken prototype to a
shipping, competitive-quality coding agent. The map ends when the route is
clear (which features to build, which to kill, which tradeoffs to accept),
**not** when the features are built.

## Notes

- **Domain**: AI coding assistant / developer tooling
- **Method**: this is a *competitive route-finding* effort. The agent assessed
  the repo (see PLAN_SUMMARY.md for the full gap analysis) and surfaced fog —
  now chart the decisions that turn fog into a buildable route.
- **Skills**: use `/research` subagents for external-API / third-party fact
  gathering; `/grilling` + `/domain-modeling` for tradeoff decisions; `/prototype`
  only when a concrete artifact raises discussion fidelity.
- **Tracker**: local-markdown convention — `tracker.json` + `tickets/`. Each
  ticket is a file; blocking wired via tracker.json `blocked_by` arrays.
- **One ticket per session** (except AFK research, which may fan out).
- **Honest baseline**: the README's competitive claims ("14 providers",
  "outperforms other coding agents", "60-80% of violations", "1M+ lines") are
  treated as **unproven assertions** until validated.

## Decisions so far

- **001 — Moat = Mechanized Gate + Negative-Knowledge Loop.** RESOLVED. `GateEngine` (harness/src/engine.rs) runs deterministic, zero-token, in-process Rust checks (structural + taste + golden + linter wrappers + scoring). `NegativeKnowledgeStore` (harness/src/negative_knowledge.rs) auto-promotes recurring failures (freq ≥ 3) into permanent RulesDatabase entries — novel, no competitor has this. Multi-agent pipeline (Plan→Build→Gate→Review→Fix) is built but quarantined behind `OMEGA_EXPERIMENTAL_PIPELINE=1` and not in the CLI — a non-shipping secondary differentiator. 14-provider abstraction has router.rs but it's unwired (not a moat yet). n-gram embeddings, .otable, Entropy GC are not moats. Honest positioning: "Rust-native deterministic quality gate + auto-learning negative-knowledge loop — alpha: Gate is real and tested, pipeline is built but quarantined.", secondary differentiator. 14-provider abstraction has router.rs but it's unwired (not a moat yet). n-gram embeddings, .otable, Entropy GC are not moats. Honest positioning: "Rust-native deterministic quality gate + auto-learning negative-knowledge loop — alpha: Gate is real and tested, pipeline is built but quarantined."
- **006 — Entropy fix vs quarantine vs delete = FIX.** RESOLVED. Implemented `Language::detect()` (manifest-file scan) and `Language::label()` in harness/src/language.rs. Regenerated corrupt Cargo.lock. `cargo check -p omega-core -p omega` ✓. 63 harness tests + 135 omega-core tests pass. Entropy GC now compiles; DriftScanner feeds the Gate moat (repo-wide deterministic drift scan).

## Not yet specified

- The true competitive "moat" — is it the Rust Gate, the multi-agent pipeline,
  provider flexibility, or something else entirely? → **RESOLVED: Gate + Negative-Knowledge Loop** (see Decision 001 above).
- Whether the multi-agent Plan→Build→Review pipeline (currently gated behind
  `OMEGA_EXPERIMENTAL_PIPELINE=1` and non-compiling via the entropy breakage)
  is a competitive advantage or a liability vs Claude Code's single-agent loop → **build now compiles** (entropy fixed, ticket 006), but pipeline still env-gated and not in CLI.
- The honest cost of shipping a binary (P1-07) vs shipping source-only.
- Whether the "14 providers" abstraction has real routing/health-check value
  or is marketing window-dressing (router.rs exists but is unwired).
- The minimum viable CLI subcommand surface that constitutes "parity" with
  Codex CLI (which has: `codex`, `codex -h`, and a chat loop — that's it).

## Out of scope

- Building the taste-1 ML model (that lives in the separate
  `wayfinder/taste-system/` plan if active; do not conflate).
- Rewriting the entire TUI or replacing Ratatui with a web/Electron shell.
- Implementing any ticket resolved by this map — decisions only.
