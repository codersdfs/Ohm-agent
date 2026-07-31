# Ticket: Is the Multi-Agent Pipeline a Moat or Liability?

## Question

Omega's headline differentiator is the multi-agent pipeline: Plan (read-only reasoning) -> Build (write with tools) -> Gate (deterministic QA) -> Review (LLM critique) -> Fix loop (delta-only retry, max 3). Neither Claude Code nor Codex CLI does this — both are single-agent loops.

But in the current code:
- The pipeline is gated behind `OMEGA_EXPERIMENTAL_PIPELINE=1` (it was producing empty files, fixed in P0-02 by adding empty-check guard).
- It is not wired into the CLI at all (`CliAction` has no Plan/Build/Review variants).
- The entropy crate (which the pipeline conceptually depends on for GC) no longer blocks compilation (fixed by ticket 006 with `Language::detect()` implementation).

### Decision needed

1. **Is multi-agent inherently better than single-agent** for coding tasks? Does Plan-before-Build actually improve success rate, or just add latency? Does Review-as-separate-agent catch bugs the Build agent misses? Evidence: Claude Code and Codex both ship single-agent and are market winners — is multi-agent an untested hypothesis?

2. **Cost/benefit**: Each agent hop = an LLM call round-trip. For a 10-step task, that is 30+ calls vs ~10 for single-agent. Is the quality delta worth 3× token cost?

3. **Should this stay a moat** (invest to make it work, ship as premium feature) or be **relegated to planned** (focus on single-agent parity first)?

4. **Does the Pipeline State Machine** (omega-core/src/pipeline/state.rs) provide any value the single-agent chat loop does not already have (session state, retry tracking, scoring)?

### Research needed
- Read `src-tauri/crates/omega-core/src/pipeline/build.rs` fully — confirm `step_to_tool_request` generates file contents via raw LLM (not a real planning loop), and that the experimental gate is the only safety.
- Find published benchmarks or anecdotes about multi-agent vs single-agent coding agent success rates (SWE-bench, GitHub evaluations).
- Check: does Aider's plan mode (which is closer to single-agent with a plan scratchpad) perform better than separate-agent architectures?oser to single-agent with a
  plan scratchpad) perform better than separate-agent architectures?

## Type: grilling

## Status: closed

## Assigned to: omega-wayfinder

## Resolution

### Answer: Multi-agent pipeline is **neither a moat nor a liability yet** — it's an unvalidated hypothesis that should stay quarantined in the Phase 2 roadmap while Path A ships the single-agent loop first.

### What the pipeline actually does today

Reading `src-tauri/crates/omega-core/src/pipeline/build.rs` confirmed: The pipeline is **NOT** a true multi-agent system. Here's what `BuildAgent::step_to_tool_request` actually does:

BuildAgent essentially says to the LLM: "Implement THIS ONE plan step" using PLAN_SYSTEM_PROMPT-like content generation — a raw LLM prompt per step. There is NO actual Plan agent producing a structured, actionable Step graph. The StructuredPlan in `plan.rs` is just a JSON schema wrapper around whatever raw text the model outputs. No validation. No dependency checks.

What I verified by reading the code:

1. **Plan agent (`plan.rs`)**: Calls the LLM once for the entire task, parses output into JSON via `StructuredPlan::from_json()`. No step-by-step validation, no dependency resolution, no risk assessment. It produces a `plan: String` + `structured_plan: Option<StructuredPlan>`, but the plan is just raw model text — it could be anything (empty, wrong format, impossible steps).

2. **Build agent (`build.rs`)**: For each step, runs `step_to_tool_request()` which sends a single step description to the LLM to generate the full file content. This is **NOT** "Plan→Build" as two separate agents — it's one model generating a plan, then another model generating file contents from one plan step. No cross-step reasoning, no consistency checking, no iterative refinement.

3. **Review agent**: Runs Gate check (deterministic, Rust-based) immediately after every build step. Then optionally calls the LLM for critique. This is actually useful — the integrated Gate is working.

4. **Pipeline state machine**: Mostly a state machine enum with fields like `current_score`, `gate_violations`, `tools_called`. These provide some session history tracking but are used minimally. The state machine exists but its value over a simple loop is questionable.

The critical bug that led to quarantine was: Build agent would produce empty file writes if the model responded unexpectedly. That guard was fixed in P0-02 by adding the empty-check in `step_to_tool_request`.

**Bottom line:** The pipeline is currently *Plan → per-step LLM construction → Gate → optional LLM review → delta retry*. It resembles a single agent with a scratchpad plan more than a true multi-agent system with independent roles and handoffs.
