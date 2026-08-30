# This week (Day 1 through Day 5)

The first five sessions after the v0.1.0 cut. The end-of-week state is the
transition from Phase 0 (Foundation) to Phase 1 (Gate Differentiator).

## End-of-week goal

By Friday:

- CI runs on every PR and fails on test regression or Gate pass-rate
  drop.
- `BudgetExhausted` is enforced and tested.
- The negative-knowledge loop has a minimal end-to-end implementation
  (the "recurring failure detected" path, even if the auto-promotion is
  still manual).
- v0.1.1 is published.
- The task board is up to date and the next 2-3 tickets are queued.

## Day 1 — Foundation starts

See `tomorrow.md` for the full plan. The five items, in order:

1. Add `.github/workflows/ci.yml` (~2h)
2. Fix `BudgetExhausted` gap (~1h)
3. Add Gate eval to CI (~4-6h)
4. Migrate tool-calling tickets to `task-board.md` (~1h)
5. Cut `v0.1.1` (~30m)

Realistic completion: items 1, 2, 4 done; item 3 in progress or done;
item 5 deferred to Day 2 if needed.

## Day 2 — Foundation finishes, Phase 1 starts

- **Morning:** finish the Gate eval CI work from Day 1 (if not done).
- **Afternoon:** open Phase 1 with the first negative-knowledge ticket.
  Read `phase-1-gate-differentiator.md` for the design.
- **End of day:** cut `v0.1.1` (item 5 from Day 1, if not already done).

## Day 3 — Negative-knowledge loop (1/2)

The recurring-failure detector. A tool error is "recurring" when the same
error message has appeared >= 3 times in the rules database.

- **Morning:** design the schema. The current `harness::rules::RulesDatabase`
  needs a `frequency` column (or a separate `failures` table). Decide.
- **Afternoon:** implement the detector. It runs after every agent
  turn, in the same place that negative-knowledge is currently called
  (search for `neg_knowledge` in the codebase to find the hook point).
- **End of day:** the detector is in place but only logs; auto-promotion
  is a follow-up.

## Day 4 — Negative-knowledge loop (2/2)

- **Morning:** auto-promotion. When a rule is detected as recurring, the
  user gets a prompt ("this has failed 3 times; promote to a rule?").
  If yes, the message + the suggested fix becomes a linter rule in the
  Gate's rules DB. If no, it stays at frequency = 3 and increments only
  on further recurrence.
- **Afternoon:** the user-facing flow. Today it is a TUI prompt; in
  Phase 2 it becomes a CLI flag (`omega exec --promote-rule=true`).
- **End of day:** an end-to-end test (use the existing eval framework)
  where a single repeated failure triggers promotion. The Gate now
  blocks the same failure on subsequent runs.

## Day 5 — Wrap up, queue next week

- **Morning:** measure the impact. Re-run the 20-task eval. Compare to
  the Day 1 baseline. Update `evals/baseline.md` with the new numbers.
- **Afternoon:** write a one-paragraph "what changed this week" note in
  `evals/baseline.md` and link it from the project README. Update the
  task board with what got done and what is next.
- **End of day:** plan next week. The two big candidates for next week
  are MCP stdio transport (Phase 2) and the TUI polish work (Phase 2).
  Pick based on what the eval tells you — if the Gate is now > 85% pass
  rate, lean into Phase 2. If it is still at 75%, the next week should
  be more Phase 1 work.

## Risk register for the week

| Risk | Mitigation |
|---|---|
| The Gate eval CI turns out to be flaky (small eval set, high variance) | Run the eval 3 times in the first green CI; record the variance. If > 1 task, expand the eval set. |
| `BudgetExhausted` change breaks an existing test that depends on the old behavior | Re-read the test before changing. If it does break, the test was wrong. |
| Negative-knowledge auto-promotion is annoying to users (too many prompts) | Make the prompt rare: only prompt for rules with frequency >= 5, not 3. Tune later. |
| CI workflow has the same class of bugs as `release.yml` (Windows PATH, etc.) | Re-read the postmortem before writing CI. |
| The week is interrupted (real work, real life) | Each day is independent. Day 1 can stand alone. If only Days 1-2 land, the project is still in better shape than today. |

## What success looks like at the end of the week

- `cargo test --workspace` is green on CI for `main` and every PR.
- The Gate eval is part of CI. A 5% regression is a red build.
- `v0.1.1` is on GitHub Releases. A real user could download and run it.
- The negative-knowledge loop has a working end-to-end path on at least
  one example.
- `evals/baseline.md` has fresh numbers and a one-paragraph changelog.
- The task board is the single source of truth for "what is next".

## What failure looks like

- Day 1 is not done by Day 3. (Either scope is wrong, or distractions
  won. The fix is to drop items 3-5 from Day 1 and only do items 1, 2, 4.)
- The Gate eval CI is built but not run on any PR. (The "CI" file exists
  but is not actually triggered. The fix is to make the workflow file
  trigger on `pull_request` events, not just `push` to main.)
- The negative-knowledge work is half-done and abandoned. (The fix is to
  cut scope to "detect only" and defer "promote" to next week.)

## What is NOT in this week

- **No new tools.** Tool-calling tickets #07, #08, #09, #10 are not
  touched. They are design debt; the week is for the Gate thesis.
- **No TUI polish.** `todo.md` Phase 2-3 work is deferred.
- **No MCP stdio.** Phase 2 work. Wait until the Gate is real.
- **No provider work.** 14 providers is enough.
