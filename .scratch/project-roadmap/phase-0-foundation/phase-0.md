# Phase 0 — Foundation

## Goal

Close the v0.1.0 gaps and ship a known-good v0.1.1. This phase is about
*operational hygiene* — making the next phases cheaper — not about new
features.

## Entry condition

v0.1.0 has been cut and the postmortem is on `main`. (Done:
`177a25c` + `aaab474`.)

## Exit condition

All of the following are true:

- `v0.1.1` is published with CI, the BudgetExhausted fix, and the eval
  guard wired in.
- Per-PR CI runs `cargo test --workspace` and the eval suite on every
  pull request.
- The eval suite has a stored baseline; a 5% regression fails the build.
- The task board is the single source of truth for next-work. Ad-hoc
  scratchpads are not used.

## Work in this phase

See `tasks/tomorrow.md` and `tasks/this-week.md` for the day-by-day plan.
The phase-level work is:

1. CI workflow (`.github/workflows/ci.yml`).
2. `BudgetExhausted` enforcement in `subagent.rs::run`.
3. Gate eval in CI with regression guard.
4. Task board migration.
5. v0.1.1 cut.

## What is NOT in Phase 0

No new features. No tool changes. No provider changes. No TUI polish.
The only "feature" change is `BudgetExhausted` because it is a bug fix
(the field is declared but unused; closing that gap is hygiene, not
feature work).

## Risks

| Risk | Mitigation |
|---|---|
| CI workflow has the same class of bugs as `release.yml` (Windows PATH, syft flag syntax, etc.) | Read `../release-pipeline-postmortem.md` before writing CI. Don't write the workflow from memory. |
| Eval set is too small to give stable numbers (20 tasks, 12% FP rate = ~2.4 tasks of noise) | Run the eval 3 times in the first green CI; record the variance. If the variance is too high, expand the set. |
| `BudgetExhausted` change regresses a real user path | The change is purely additive (it adds a new outcome to an existing loop). It cannot regress `Completed` or `MaxTurns` outcomes. |
| The phase runs out of time before v0.1.1 is cut | v0.1.1 is a tag, not a build. The build already exists. The cut is a 5-minute operation; defer it. |

## Definition of done for the phase

When the user asks "is Phase 0 done?", the answer should be unambiguous:

- Yes, v0.1.1 is published and CI is green for at least one PR cycle.
- No, if any of the four work items is incomplete.

No "kinda". No "in progress". Yes or no.
