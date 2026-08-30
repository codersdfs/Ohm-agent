# Current state

Snapshot from 2026-08-30, after the v0.1.0 cut. Re-read this before planning the next session.

## What works today (verified by running it)

- **Release pipeline:** `cargo build --workspace` produces binaries for linux x64,
  macos x64, macos arm64, windows x64. Each is signed (cosign keyless), SHA-256
  digested, and SPDX-attested. Consolidated SHA256SUMS is signed. GitHub Release
  is created with auto-generated changelog via git-cliff. See
  `../release-pipeline-postmortem.md` for the ten fixes that made it work.
- **Single-agent chat loop:** 14 built-in tools, 14 LLM providers, real streaming,
  session persistence, session resume, AGENTS.md/.omega/CLAUDE.md loading, tilde
  expansion, tool-result truncation surfacing, token cost tracking, gate-fail
  retries. Verified by 547 passing tests across 8 crates.
- **Headless mode:** `omega exec` runs the same agent loop non-interactively
  for CI/scripting use. JSON output mode for downstream tools.
- **MCP server:** `omega serve-mcp` exposes the built-in tool set over HTTP
  JSON-RPC. Stdio transport is in the codebase but not yet battle-tested.
- **Quality Gate:** structural + taste + golden + repeated + external rules
  via `harness/`. 75% pass rate on the 20-task internal eval. FP rate 12%.
- **Subagent delegation:** `spawn_subagent` inline path, parent's pipeline
  routing, ticket #11 acceptance met (gate, max-turns, whitelist).

## What does not work / has gaps

### High priority (will block v0.1.1 or v0.2.0)

- **`RunOutcome::BudgetExhausted` is defined but never constructed.** The
  subagent's `token_budget` field is documented but not enforced. The loop
  only checks `max_turns`. Fix is ~30 lines in `subagent.rs::run`; the test
  from ticket #11 is wired to flip the assertion once the code path exists.
- **MCP stdio transport** is in the codebase but not exercised. The mcp crate
  is HTTP-only. Production deployment needs stdio (the spec's primary
  transport).
- **Negative-knowledge loop** is partial. Frequency tracking exists; auto-
  promotion to linter rules is not yet implemented. This is the second half
  of the Gate thesis — it has to ship for the v1.0 win.
- **28 open tool-calling tickets** in `../tool-calling-cleanup/issues/`. None
  block shipping, but several (e.g., #08 HookContext default trap, #09 mcp
  wire unification, #10/#27 is_read_only vs check_permissions) are design
  debt that compounds over time.

### Medium priority (will block v1.0)

- **Markdown rendering in TUI.** Currently raw text. `todo.md` Phase 2 work.
- **Diff display after write/edit.** Currently invisible to user. `todo.md`
  Phase 3 work.
- **Permission prompt dead code** (`tui/permission_prompt.rs` exists but
  is not wired). Needed for v1.0 — sensitive ops must prompt, not just
  log.
- **Public eval suite.** `evals/baseline.md` is the only measurement we
  have. No regression guard. A PR that drops the gate pass rate from 75%
  to 70% would not fail CI today.
- **Public benchmark vs Claude Code / Aider.** We have no apples-to-apples
  comparison. The thesis ("we win on the Gate") is unmeasured.

### Low priority (won't block v1.0)

- **Multi-language Entropy GC.** Rust-only today.
- **Provider routing with health checks.** Partial.
- **Delta context cache.** Planned but not implemented.
- **TUI status bar / provider panel polish.** Working, not pretty.

## Numbers (from `evals/baseline.md` and recent runs)

| Metric | Value |
|---|---|
| Tests passing | 547 / 547 |
| Gate pass rate (internal eval) | 75% (15/20) |
| Gate false-positive rate | 12% |
| Avg retries per task (Gate On) | 1.1 |
| Avg tokens per task (Gate On) | 8,200 |
| Repeat error recurrence (Gate On) | 8% |
| v0.1.0 release size | 4 platform binaries + 1 SBOM per binary |

## Known unknowns (questions, not answers)

- **Will the Gate's 12% FP rate hurt adoption?** A team that gets false
  positives will turn the Gate off. We don't know if 12% is tolerable.
- **Does the negative-knowledge loop actually improve the pass rate?** It
  is the second half of the thesis. If frequency-3 promotion only adds
  noise, we ship a worse product, not better.
- **Will 547 tests be enough regression coverage for the next 30 days?**
  Probably not. We will add tests as we go.
- **What is the "session duration" that a user actually wants?** Today
  sessions are linear. Real users may want branching, replay, search.

## Velocity (estimated)

From the last session: v0.1.0 was cut, the postmortem was written, ticket
#11 was closed with three integration tests, and the budget gap was
documented. That is roughly 1.5-2 days of focused work in one sitting. The
realistic per-week cadence is:

- 3-4 small tickets closed (a "small" ticket is < 2 hours)
- 1 medium ticket closed (2-6 hours)
- 1 large ticket started, possibly finished
- 1 day of unplanned work (bug, customer feedback, infra)

If a week goes by without closing a medium ticket, something is off —
either scope was too big, or distractions were too many.

## What is missing from this snapshot

- **No customer feedback yet.** v0.1.0 is the first public release. We
  will learn from real users only after Phase 3.
- **No competitive measurement.** The "we win on the Gate" claim is
  unverified against Claude Code / Aider on the same tasks.
- **No CI.** There is no GitHub Actions workflow that runs on PR. The
  release workflow exists but the per-PR CI does not. **This is a real
  gap and should be the first thing added in Phase 0.**
