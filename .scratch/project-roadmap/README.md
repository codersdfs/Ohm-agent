# Project Roadmap

Direction and tasks for Omega Agent from the v0.1.0 cut through public launch.

## How to read this

1. **Tomorrow:** open `tasks/tomorrow.md`. That is what gets done in the first session.
2. **This week:** `tasks/this-week.md`. The end-of-week state is described at the top.
3. **The why:** `vision.md` explains the project's positioning — the niche we are
   actually trying to win, and the niche we are deliberately not chasing.
4. **The how:** `principles.md` captures the rules for making cuts when the design is
   under-specified or the work is bigger than expected.
5. **The phases:** the four `phase-*.md` files are ordered. Read them in order. Each
   phase has an entry condition, an exit condition, and a list of work that fits
   inside the phase.
6. **Task board:** `task-board.md` is the working backlog. The tool-calling tickets
   from `../tool-calling-cleanup/issues/` are migrated here. Add new tasks to this
   file, not to ad-hoc scratchpads.

## Phases at a glance

| Phase | Goal | Exit condition |
|---|---|---|
| **0 — Foundation** | Close v0.1.0 gaps, ship a known-good v0.1.1 | v0.1.1 published, BudgetExhausted fixed, all v0.1.0 postmortem lessons applied |
| **1 — Gate Differentiator** | The Gate becomes the reason people use Omega | Gate catches 90%+ of the eval set with < 5% FP, negative-knowledge loop live |
| **2 — Ship Ready** | The product is the kind of thing a paying user would install | v1.0 cut with: a real TUI, a real headless mode, real eval, real docs |
| **3 — Public Launch** | First 100 external users | README-driven install, public changelog, v1.0 announcement |

## Honest current state (snapshot)

Captured 2026-08-30, immediately after v0.1.0 was cut.

- **Tag:** v0.1.0 (https://github.com/codersdfs/Ohm-agent/releases/tag/v0.1.0)
- **Tests:** 547 passed, 0 failed (across 8 crates)
- **Tool-calling:** 28 open tickets in `../tool-calling-cleanup/issues/`
- **Gate eval:** 75% pass rate on the 20-task internal set (baseline.md)
- **Feature completeness vs Claude Code:** ~30% by surface count; ~70% in the niche where the Gate matters
- **Release pipeline:** works end-to-end after 10 fixes; postmortem recorded
- **Subagent architecture:** wired through parent's pipeline; BudgetExhausted constructor is defined but never called (the one real gap surfaced by ticket #11)

See `current-state.md` for the longer version.

## What this folder is NOT

- **Not a marketing doc.** `vision.md` is honest about the niche.
- **Not a Gantt chart.** The phases are sequenced, not scheduled. The calendar lives in `tasks/`.
- **Not a commitment.** Phases get redefined as we learn. The next session should re-read `current-state.md` before planning.
