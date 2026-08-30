# Vision

## What we are building

Omega Agent is a **gate-first AI coding assistant**. The single most
important thing in the product is the deterministic, zero-LLM-token Quality
Gate that runs on every write. The chat loop, the tools, the providers, the
subagent architecture — all of it is plumbing. The Gate is the product.

## The niche we are trying to win

There are three kinds of "AI coding tool" users:

1. **Casual users** who want autocomplete and a chat box. They will use the
   IDE extension. We are not building one.
2. **Power users** who want full project control, multi-file edits,
   autonomous runs, and a real terminal UI. This is where Claude Code,
   Aider, Cursor, and Windsurf live. We are not going to beat them on raw
   surface area — they have 50+ engineers and 18 months of polish.
3. **Teams that ship** — engineering orgs who write code that goes into
   production, where every PR gets reviewed, where a regression costs real
   money, where "the LLM edited the wrong file" or "the LLM broke the build"
   is a real problem. These teams use CI, linters, and code review because
   they care about correctness. **This is the niche we are building for.**

For those teams, the cost of an AI tool is not the API bill — it is the
engineering hours wasted on bad edits that get caught in review. The
question is not "did the LLM write the code?" but "did the code pass the
gate?" A tool that produces 80% gate-passing code is twice as valuable as
one that produces 60%, even if the second one has more features.

## The differentiator

The Gate. Specifically:

- **Deterministic.** Same input → same output. No flakiness.
- **Zero token cost.** Runs in microseconds, on the Rust side.
- **Structural, taste, golden, repeated, external rules.** Not just lint —
   pattern scoring across 5 dimensions (see `harness/scoring.rs`).
- **Sub-threshold writes are blocked, not warned.** When in Block mode,
   a write below the pass threshold never reaches disk.
- **Negative knowledge loop.** When a recurring failure (frequency >= 3)
   is detected, it is promoted to a linter rule. The Gate learns.

No other tool has this. Claude Code has no Gate. Aider has a weak one. The
GitHub Copilot stack has nothing. This is the moat — small, real, and it
is already shipping in v0.1.0.

## What we are deliberately not building

To be honest: this is a small project (1-2 engineers, side-project budget).
We do not have the time to build a generic Claude Code competitor. The
following are explicit "no" decisions:

- **No IDE extension.** VS Code extension alone is a 3-month project.
- **No multi-agent orchestration UI.** Single-agent chat is enough.
- **No web UI.** TUI + headless is the surface.
- **No "supports all 200 languages perfectly" promise.** Gate rules are
   best-effort, not complete.
- **No retraining / fine-tuning of the LLM.** We use existing providers.
- **No context-window heroics.** 8K-200K is the working set, period.

When in doubt, ask: "is this the Gate, or is it features?" If it is
features, defer it. The Gate gets the time.

## What success looks like at v1.0

v1.0 is the cut where the Gate is the headline. Specifically:

- A new user can `cargo install omega` (or download a release), point it
  at a fresh project, run `omega gate ./src/`, and see the Gate flag a
  real violation (e.g., a 6-line `unwrap()` chain, a duplicate import,
  a magic number) with explanation and a suggested fix. **No LLM token
  was spent.**
- On the same project, the user can run `omega exec "add a CLI flag"`
  and watch the agent attempt the change, get blocked by the Gate on a
  sub-threshold draft, and re-attempt until the draft passes. The user
  sees the Gate score per draft in the TUI.
- On the 20-task internal eval (`evals/baseline.md`), the agent hits
  **>= 90% pass rate** with **<= 5% false-positive rate** on the Gate.
- `omega --version` reports a v1.0.0 tag with a signed, SPDX-attested
  release on GitHub.

That is the win. Not "more features than Claude Code." Not "more providers
than anyone." Just: the Gate works, the Gate blocks, the Gate saves the
team's review time.

## What success looks like at v2.0 (post-launch)

- 100 active users in production. (Definition of "active": ran omega exec
  or omega gate at least 3 times in the past 30 days.)
- The negative-knowledge loop has promoted at least 10 user-discovered
  rules to the Gate's rule DB. (Measured by counting rules with
  `source = "user_promoted"` in the rules table.)
- First paying customer (or first org with >= 5 seats).

## What this means for the next 3 months

The phases in this roadmap are sequenced so the Gate gets the lion's
share of the time. Phase 1 is half of the calendar. If we get to the end
of Phase 1 and the Gate is not measurably better than it is now, the
project is failing its own thesis — even if the features are shipping.
