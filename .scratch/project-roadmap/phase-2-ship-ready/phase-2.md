# Phase 2 — Ship Ready

## Goal

The product is the kind of thing a paying user would install, run, and
recommend. The v1.0 cut is a packaging exercise; the work in Phase 2 is
to make the product presentable, documented, and tested.

## Entry condition

Phase 1 is complete (Gate at 90%/5%, negative-knowledge live,
competitive benchmark published).

## Exit condition

- `v1.0.0` is published.
- README-driven install: a new user can copy-paste four commands and
  have a working installation.
- The full eval suite runs on the published binary (not just on the dev
  build).
- The changelog from v0.1.0 to v1.0.0 is auto-generated, human-readable,
  and accurate.
- The competitive benchmark from Phase 1 is linked from the README.

## Work in this phase

### 2A. TUI polish

- Markdown rendering in chat output (pull-cmark + ANSI).
- Syntax highlighting in code blocks (syntect).
- Diff display after write/edit (green/red lines).
- Permission prompt (currently dead code, must be wired).
- Status bar at the bottom (provider, model, step, token count).
- Slash command polish (`/init`, `/cost`, `/retry`, `/help` grouped by
  category).

These are `todo.md` Phase 2-7 work. Estimated 1-2 weeks.

### 2B. Headless mode polish

- `omega exec --json` for downstream tooling.
- `omega exec --promote-rule` (the CLI form of the negative-knowledge
  auto-promotion).
- `omega exec --gate-mode=block|warn|advice` (currently env var, should
  also be a flag).
- Exit codes that downstream CI can use (`0 = clean, 1 = gate blocked,
  `2 = retried-and-failed, 3 = user-rejected).

Estimated 1 week.

### 2C. Documentation

- README rewrite for the new niche and the new numbers.
- A `docs/quickstart.md` with copy-pasteable examples.
- A `docs/gate.md` explaining what the Gate catches, with examples.
- A `docs/eval.md` explaining the eval set, how to run it, and how to
  read the numbers.
- A `docs/architecture.md` for the curious (the existing `AGENTS.md` is
  the closest thing; promote it to a real doc).

Estimated 1 week. (Ponytail: this is a big task, but most of it is
writing, not coding. The existing README is a starting point.)

### 2D. v1.0.0 cut

- Same shape as the v0.1.0 cut, but:
- The release notes lead with the new Gate numbers (90%/5%).
- The competitive benchmark is linked from the notes.
- The postmortem pattern from v0.1.0 is repeated (capture every surprise
  in `.scratch/release-pipeline-postmortem-v1.0.md`).
- A blog-style announcement (could be a GitHub Discussion, a
  `docs/announcements/v1.0.md`, or a tweet — decide when we get there).

Estimated 30 minutes of work, plus 30 minutes of waiting for the workflow.

## What is NOT in Phase 2

- No new Gate features. Phase 1 is closed by definition. If a new Gate
  idea comes up, file it in the task board under P2 — it goes in v1.1,
  not v1.0.
- No new tools. (Exception: if a tool-calling ticket is now a blocker,
  it goes in the v1.0 work. But that should be rare.)
- No new providers.
- No architectural rewrites.

## Risks

| Risk | Mitigation |
|---|---|
| The TUI polish takes longer than 2 weeks (it usually does) | Cut scope. Render markdown, skip syntax highlighting. Show diffs, skip the bottom status bar. Polish, then ship — not the reverse. |
| The docs take longer than 1 week | Cut scope. README rewrite is the only must-have. `docs/quickstart.md` is the second. Everything else can wait for v1.0.1. |
| A Gate bug is found during the polish work | That's a Phase 1 finding. Reopen Phase 1 for the day, fix the bug, retest. Then back to Phase 2. Don't ship a known-bad Gate. |

## Definition of done for the phase

`v1.0.0` is on GitHub Releases with a release note that links to the
Phase 1 benchmark. The new-niche README is the project README. A new user
who follows the README can run `omega gate ./src/` on a fresh project and
see a real violation flagged in microseconds, with no LLM token spent.
