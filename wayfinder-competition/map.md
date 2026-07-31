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

- **001 — Moat = Mechanized Gate + Negative-Knowledge Loop.** RESOLVED. `GateEngine` (harness/src/engine.rs) runs deterministic, zero-token, in-process Rust checks (structural + taste + golden + linter wrappers + scoring). `NegativeKnowledgeStore` (harness/src/negative_knowledge.rs) auto-promotes recurring failures (freq ≥ 3) into permanent RulesDatabase entries — novel, no competitor has this. Multi-agent pipeline is built but quarantined behind `OMEGA_EXPERIMENTAL_PIPELINE=1` and not in the CLI. Honest positioning: "Rust-native deterministic quality gate + auto-learning negative-knowledge loop — alpha: Gate is real and tested, pipeline is built but quarantined."

- **002 — Ship-now vs invest-in-moat strategic path.** RESOLVED. Hybrid approach: ship Path A (single-agent chat + Gate) alpha first with honest README and eval harness (~2-3 weeks), keep multi-agent and advanced features gated behind experimental flags pending validation. Key insight: Path A validates market early while de-risking Path B investment through sequential gates.

- **003 — Minimum CLI parity with Codex CLI.** RESOLVED. Codex CLI surface is minimal (`codex`, `codex -h`, chat loop). Omega should implement equivalent basic chat interface first, then expand. No need to match feature-for-feature; focus on unique differentiators (Gate) instead.

- **004 — Honest README rewrite scope.** PENDING BLOCKED BY 006. Once entropy compile is fixed, rewrite README to reflect only shipping capabilities. Do not overclaim unimplemented features.

- **005 — Is multi-agent pipeline a moat or liability?** UNDER REVIEW. Pipeline compiles after entropy fix (006) but remains environment-gated and not wired into CLI. Question remains: does Plan→Build→Review actually improve success rate enough to justify added latency and complexity? See ongoing research.

- **006 — Fix entropy compile error or quarantine it?** RESOLVED (TASK). Implemented `Language::detect()` in `harness/src/language.rs`; added `Language::label()`; fixed corrupted `Cargo.lock`. Now `cargo check -p omega-core -p omega` passes. 203 tests pass across entropy (5), harness (63), omega-core (135). Unblocking ticket 004.

## Not yet specified

- **Path B technical feasibility** — What are the specific engineering estimates, dependencies, and showstoppers for executing Phase 1 + Phase 2 ROADMAP? → **007-path-b-feasibility** (open, researching)
- **Path B timing viability** — Will the 6-12 month execution window let us validate components before competitive window closes? → **008-path-b-timing** (open, blocked by 007)
- **Path B risk mitigation strategies** — How do we plan for the high-probability/high-impact risks identified? → **009-path-b-risk-mitigation** (open, blocked by 007)
- **Path B go/no-go decision criteria** — At what measurable thresholds do we commit to full Path B execution versus pivot? → **010-path-b-decision-criteria** (open, blocked by 008, 009)
- **Multi-agent pipeline cost/benefit analysis** — Does the quality delta justify 3× token cost? → continues investigation in 005
- **Provider routing health-check value** — Does the 14-provider abstraction have real value beyond marketing? → needs more research

## Out of scope

- Building the taste-1 ML model (that lives in the separate `wayfinder/taste-system/` plan if active; do not conflate).
- Rewriting the entire TUI or replacing Ratatui with a web/Electron shell.
- Implementing any ticket resolved by this map — decisions only.
- Full MVP product implementation — this map charts the route, it doesn't walk it.
