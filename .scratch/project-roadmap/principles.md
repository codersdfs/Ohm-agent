# Principles

How to make decisions when the design is under-specified, the work is bigger
than expected, or the ticket is ambiguous. These rules are the ones I (the
agent) will follow by default; override them by saying so.

## 1. The Gate gets the time

If a task is "improve the Gate" or "improve the eval" or "close a gate-related
ticket", it is in scope. If a task is "add another tool" or "add another
provider" or "polish a UI screen", it is a defer.

Rationale: the Gate is the differentiator. Adding tools is the same game
everyone else is playing. We cannot win that game.

## 2. Tests before features

Every feature lands with at least one regression test. The test fails before
the feature is added (red), passes after (green), and the diff is the smallest
thing that flips red to green.

A "feature" without a test is a guess. A test is a contract.

## 3. Smallest diff that works

When two solutions both work, take the shorter one. The shorter one is easier
to review, easier to revert, easier to extend. The longer one is rarely
"more correct" — it is usually "more careful-looking" or "more general".

If a smaller diff feels wrong, write a `ponytail:` comment naming the
ceiling and the upgrade path. Future-you can revisit. Present-you should
ship.

## 4. Ponytail by default

Ponytail mode is active. Before writing a new helper, check whether one
already exists. Before adding a dependency, check whether stdlib does it.
Before writing 50 lines, check whether 5 lines work.

The ladder:

1. Does this need to exist? (YAGNI — speculative features are skipped.)
2. Is it already in this codebase? (Look before writing.)
3. Does the stdlib do it? (Use it.)
4. Does a native platform feature cover it? (Use it.)
5. Does an already-installed dependency solve it? (Use it.)
6. Can it be one line? (One line.)
7. Only then: the minimum code that works.

## 5. Question requirements before coding

If a ticket says "support MCP stdio transport", do not start by writing
stdio code. First ask:

- What is the minimum end-to-end test that proves stdio works?
- What is the existing HTTP test? Can the new test mirror it?
- Is the request "make stdio work for one server" or "support the full
  spec"? The former is a day; the latter is a week.

A day spent clarifying is two days saved in rewriting.

## 6. Close tickets with diffs, not prose

A ticket is "closed" when its acceptance criteria are checked off, not
when someone says it's done. A ticket with a green test and a one-line
commit is a closed ticket. A ticket with three paragraphs of explanation
and no diff is still open.

## 7. Document the *why*, not the *what*

Comments in code should explain why a non-obvious choice was made. The
code itself is the what. `ponytail:` comments name the ceiling. Doc-comments
on public types should explain the contract, not the implementation.

Long-form prose belongs in `.scratch/` or `docs/`, not in code. Code is
read by humans at 200 lines/minute; prose is read at 30 lines/minute.
Don't put prose where code is read.

## 8. The 30-line test of code review

A code change that is hard to review is probably too big. The 30-line
rule of thumb: a single commit should be reviewable in 5 minutes. If a
change is 200 lines and "obviously correct", it is usually 200 lines of
subtle assumptions. Split it.

A 30-line change is reviewable, testable, and revertable. Three 30-line
commits are better than one 90-line commit.

## 9. Measure before optimizing

The Gate has a 12% FP rate. Is 12% bad? We don't know — we have not
measured the FP rate at 20% or 5%. We don't know whether changing the
scoring rules will improve it. We don't know if a different threshold
matters.

Before tuning:
1. Capture the current numbers (which we have).
2. Make a hypothesis ("increasing the threshold will reduce FP at the cost
   of pass rate").
3. Make the change.
4. Re-measure.
5. If the change helped, keep it. If it didn't, revert and try a different
   hypothesis.

The eval set is small (20 tasks) and the FP rate is 12%. These numbers are
noisy. Do not chase noise.

## 10. Real users > real benchmarks

Internal evals are a proxy for the real question: "do users get value?"
Once we have users, the eval suite should be calibrated to predict user
satisfaction, not the other way around. Until then, the eval suite is the
best we have, and we treat it as such — useful, not authoritative.

## 11. The two-day rule

If a single piece of work takes longer than 2 days, something is wrong —
either the scope is too big, the design is unclear, or the codebase has an
unknown that needs researching. Stop and surface the issue rather than
plowing through.

Surfacing means: write a ticket, write a comment, write a message. Do not
silently extend the work to 4 days. The user is the only one who can
rebalance scope; they cannot rebalance scope they don't know about.

## 12. Postmortems on every release

Every release is a small experiment. The release pipeline itself has a
10-bug postmortem (see `../release-pipeline-postmortem.md`). New releases
will have their own surprises. Capture them in a postmortem, link the
postmortem from the release notes, and re-read the postmortem before the
next release.

The point is not to assign blame. The point is to make the next release
cheaper.
