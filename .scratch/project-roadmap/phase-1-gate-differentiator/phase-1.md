# Phase 1 — Gate Differentiator

## Goal

The Gate becomes the reason people use Omega. By the end of this phase,
the agent on a 20-task internal eval should hit **>= 90% pass rate** with
**<= 5% false-positive rate** on the Gate. The negative-knowledge loop is
fully automatic — recurring failures promote to linter rules without user
intervention, the new rules appear in subsequent runs, and the pass rate
climbs because the agent no longer repeats the same mistake.

## Entry condition

Phase 0 is complete (CI exists, eval is guarded, v0.1.1 is out).

## Exit condition

- The 20-task eval reports >= 90% pass rate (up from 75% at the start of
  Phase 0).
- The Gate's false-positive rate is <= 5% (down from 12% at the start of
  Phase 0).
- A "recurring failure" auto-promoted to a linter rule actually blocks
  a subsequent run. Verified by an end-to-end test in the eval suite.
- The number of "negative knowledge" rules in the rules DB grows over
  time as the agent runs, and the rules are visible to the user in a
  human-readable form (a file in the project, or a TUI panel, or both).

## Why this phase is half the calendar

The Gate is the thesis. The thesis is unproven. Until the Gate measurably
beats the no-gate baseline by enough to justify its existence, the project
is just another AI coding tool. We are not in the feature-count business;
we are in the Gate-quality business.

## Work in this phase

This phase is split into 4 sub-phases (rough order):

### 1A. Eval maturation

- Expand the eval set from 20 to 50 tasks. Add 15 "real-world" tasks
  drawn from a typical Rails/Node/Go project (cross-language coverage is
  a stretch).
- Categorize tasks by failure mode (gate-blocked, agent-gave-up,
  user-rejected, completed). This is the diagnostic data the next
  sub-phases need.
- Add the eval to the per-PR CI as a regression guard. Today, a 5% drop
  should turn the build red; tomorrow, a 5% drop in any single
  category should turn the build red.

**Estimated time:** 1 week.

### 1B. Gate tuning

- Look at every false positive in the eval set. Categorize: scoring
  rule too strict? Threshold too high? Wrong content type?
- For each category, pick the lowest-leverage fix first (a single
  threshold tweak), try it, measure, revert if it hurts. Move up the
  ladder only as needed (a content-type fix, a new rule, a new scorer).
- Target: 12% FP → 5% FP. If a single round of tuning only drops to 9%,
  do another round. If two rounds don't break 8%, the eval set may be
  noisy; expand it.

**Estimated time:** 1-2 weeks.

### 1C. Negative-knowledge auto-promotion

- A "recurring failure" (same error message, frequency >= 3) is
  detected at the end of every agent turn.
- When detected, the message + a fix (suggested by the LLM, validated by
  the user on the first instance) is promoted to a linter rule in the
  Gate's rules DB.
- The new rule is loaded by subsequent agent runs. The same failure
  message now triggers the Gate on the first try, blocking the agent
  from repeating the mistake.
- End-to-end: run the eval set, pick the most-frequent failure,
  promote it, re-run the eval set, the promoted rule should fire on the
  same task on the first attempt and block the bad output.

**Estimated time:** 1 week (some of this overlaps with the eval work).

### 1D. Competitive benchmark

- Pick a public eval set that Claude Code and Aider have run on
  (SWE-bench, or a curated subset).
- Run Omega on the same set, with and without the Gate. Publish the
  numbers (even if they're unflattering).
- This is the moment the thesis becomes testable. The hypothesis is
  "Gate On > Gate Off on the same model". If the data doesn't show
  that, the thesis is wrong and Phase 3 should pivot to "no-Gate" mode
  (or we are in the wrong niche).

**Estimated time:** 1 week (mostly waiting for eval runs).

## What is NOT in Phase 1

- No new tools. No new providers. No TUI polish. The whole phase is
  about the Gate.
- No architectural rewrites. The code is good enough. Don't refactor
  what works.
- No "support all 200 languages" expansion. Gate rules are best-effort.

## Risks

| Risk | Mitigation |
|---|---|
| The eval set is too small to be statistically meaningful (20 → 50 is still small) | Use paired comparisons (the same task with/without the Gate), which have higher statistical power than absolute rates. |
| The Gate's pass rate plateaus at 85% no matter what we tune | This is a real signal: the model is the bottleneck, not the Gate. Pivot to a different model tier (or to "use Omega as a quality gate on top of an existing tool"). |
| Auto-promotion is too aggressive and adds noise to the rules DB | Default to "log only" for the first 2 weeks. Switch to "prompt" after seeing the failure log. Switch to "auto" only after the prompt has been useful 10+ times. |
| The competitive benchmark shows Gate Off is actually better on some tasks | That's data. The thesis is "the Gate helps on most tasks, costs little on the rest." If the data shows "the Gate hurts on more tasks than it helps", we have to redesign. |

## Definition of done for the phase

Two things, both required:

1. The eval suite reports >= 90% / <= 5% (pass rate / FP rate) on the
   50-task set.
2. The negative-knowledge loop has at least 5 auto-promoted rules in
   the rules DB, each verified to fire on the eval set.

If either fails, Phase 1 is not done. Extend the phase, don't lower
the bar.

## What success looks like at v1.0 (end of Phase 2)

When Phase 1 is done, the v1.0 cut (Phase 2) is a packaging exercise:
the Gate works, the eval proves it, the negative-knowledge loop is live,
and the work in Phase 2 is to make the product presentable. Phase 2 is
small.

If Phase 1 is not done by the planned date, the project should not cut
v1.0. The thesis is not yet proven, and a public launch of an
unproven-thesis product is wasted effort.
