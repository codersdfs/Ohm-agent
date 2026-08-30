# Tomorrow (Day 1)

The first session after the v0.1.0 cut. Five things, in this order.

## 1. CI workflow (before anything else)

**Why first:** we have no per-PR CI today. Without it, every change is a
leap of faith. The Gate eval number can regress silently. Adding CI is
the cheapest insurance we can buy.

**Acceptance:**

- A new file `.github/workflows/ci.yml` exists.
- On every push to a PR branch, it runs `cargo test --workspace`.
- On every push to `main`, it also runs `cargo build --release` for all
  four target platforms (use the same matrix as `release.yml`).
- A failing test or a failing build blocks the PR from being merged
  (this is the default for `pull_request` events, no extra config needed).
- The workflow file is **identical in spirit to `release.yml`** but
  without the publish step. Read `release.yml` first; copy what you can.

**Estimated time:** 1-2 hours including the inevitable 2-3 fixes during
the first green run. (Ponytail: the postmortem has 10 lessons about
`release.yml`; the CI workflow will hit some of the same shapes. Re-read
the postmortem before writing.)

**Done when:** a PR opened against `main` shows green CI. The next
change to `main` will run the same gate.

## 2. Fix the BudgetExhausted gap

**Why second:** it is the only true "feature gap" that the v0.1.0 tests
surfaced. ~30 lines in `subagent.rs::run`. The mock provider and the test
from ticket #11 are wired so the new code path can be tested without
new infrastructure.

**Acceptance:**

- `subagent.rs::run` accumulates a `token_count` per turn (use the
  `Usage.output_tokens` from the LLM response, or tokenize the messages
  with the existing `ContextManager`'s tokenizer if simpler).
- When `token_count > config.token_budget`, the loop returns
  `RunOutcome::BudgetExhausted` with a one-line summary.
- The test `subagent::subagent::ticket_11::max_turns_returns_structured_outcome`
  is updated: change the assertion to `BudgetExhausted` and adjust the
  setup to trigger the budget (use a tiny `token_budget` like 100 and a
  mock LLM that returns 200 tokens per call).
- Add a sibling test `budget_exhausted_returns_structured_outcome` that
  specifically tests the budget path independently of max_turns.
- `cargo test --workspace` is still 547+ passing (the original test
  assertion changes, but the test name is the same so the ticket #11
  acceptance list is still met).

**Estimated time:** 1 hour. The hard part is finding where the existing
token-count plumbing lives, not writing the check.

**Done when:** a subagent configured with a tiny budget that emits a
long sequence of tool calls terminates with `BudgetExhausted`, and the
test for it is green.

## 3. Add the Gate eval to CI

**Why third:** now that CI exists, wire the Gate eval into it. Without
this, the 75% pass rate in `evals/baseline.md` can drift and we will not
notice.

**Acceptance:**

- A small Rust binary (or test) in `evals/` that loads the 20-task eval
  set, runs each one through the agent loop with the Gate on, and reports
  pass rate, FP rate, retries, and tokens.
- The binary is wired into `.github/workflows/ci.yml` as a step that
  runs on every PR. The current numbers are stored in a JSON or TOML
  baseline; a regression of > 5% pass rate or > 5% FP rate fails the
  build.
- The first CI run records the current numbers as the baseline. The
  second CI run confirms reproducibility (within 1 task of the baseline).

**Estimated time:** 4-6 hours. This is the biggest single piece of the
day. If you do not have 4 hours, defer it to next week. Do not half-do it.

**Done when:** a PR that drops the pass rate by 5% fails CI. (You can
prove this by temporarily breaking the gate and watching CI go red.)

## 4. Migrate tool-calling tickets to the task board

**Why fourth:** the tickets in `../tool-calling-cleanup/issues/` are
scattered across 32 files. They are not visible from one place. The
new task board is the single source of truth.

**Acceptance:**

- All 28 open tool-calling tickets are listed in `task-board.md` with:
  ticket number, one-line summary, estimated hours, and a priority bucket
  (P0/P1/P2).
- The original ticket files remain under `../tool-calling-cleanup/issues/`
  as the source of truth for the discussion. The task board is the index.
- The `task-board.md` includes a one-line note pointing to the original
  files ("see `../tool-calling-cleanup/issues/NN-...` for full discussion").

**Estimated time:** 1 hour. This is mechanical: read each file, write a
one-line summary, assign hours. Do not edit the original tickets.

**Done when:** I can answer "what is the next ticket to work on?" by
opening one file and looking at the top of the P0 list.

## 5. Cut v0.1.1

**Why last:** tomorrow's work is the v0.1.1 cut. v0.1.1 differs from
v0.1.0 in three small ways:

1. CI exists.
2. `BudgetExhausted` actually works.
3. The eval is gated on CI.

That is enough for a patch release. The user is updated via the
auto-generated changelog; nothing more is needed.

**Acceptance:**

- `v0.1.1` tag is pushed.
- The release workflow runs (it should pass cleanly; the postmortem
  fixes are already in place).
- `gh release view v0.1.1` shows a release with all four platform
  binaries, signed and SBOM-attested.
- A one-paragraph note in `evals/baseline.md` records "v0.1.1: CI
  added, BudgetExhausted fixed, eval wired into CI."

**Estimated time:** 30 minutes (most of the time is the workflow run).

**Done when:** v0.1.1 is on the GitHub Releases page.

## What is NOT on tomorrow's list (and why)

- **Negative-knowledge loop work.** That is Phase 1 work. It deserves
  a clean week, not a half-day at the end of tomorrow.
- **MCP stdio transport.** That is Phase 2 work. The codebase is not yet
  ready (see ticket #09 — wire unification is a prerequisite).
- **Markdown rendering / diff display / TUI polish.** All `todo.md`
  Phase 2-3 work. Defer until the Gate work is real.
- **Picking tickets from the new task board.** Tomorrow is consumed by
  the five items above. The board becomes the input to next-week's
  session.

## Honest time budget

The five items above total 7-10 hours of work. A typical focused
session is 2-4 hours. Realistic end-of-day state: items 1, 2, 4 done;
item 3 in progress; item 5 deferred to next session. That is fine —
item 5 is just a tag push and can happen any time after items 1-3 land.

If you only have 2 hours, do items 1 and 4. CI is the only one of the
five that meaningfully de-risks the next two weeks.
