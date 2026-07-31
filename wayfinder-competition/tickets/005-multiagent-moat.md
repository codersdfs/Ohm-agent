# Ticket: Is the Multi-Agent Pipeline a Moat or Liability?

## Question

Omega-s headline differentiator is the multi-agent pipeline: Plan (read-only
reasoning) -> Build (write with tools) -> Gate (deterministic QA) -> Review
(LLM critique) -> Fix loop (delta-only retry, max 3). Neither Claude Code nor
Codex CLI does this — both are single-agent loops.

But in the current code:
- The pipeline is gated behind OMEGA_EXPERIMENTAL_PIPELINE=1 (P0-02
  quarantine — it was producing empty files).
- It is not wired into the CLI at all (CliAction has no Plan/Build/Review
  variants).
- The entropy crate (which the pipeline conceptually depends on for GC)
  does not compile, blocking omega-core.

### Decision needed
1. **Is multi-agent inherently better than single-agent** for coding tasks?
   - Does Plan-before-Build actually improve success rate, or just add latency?
   - Does Review-as-separate-agent catch bugs the Build agent misses?
   - Evidence: Claude Code and Codex both ship single-agent and are market
     winners — is multi-agent an untested hypothesis?
2. **Cost/benefit**: each agent hop = an LLM call round-trip. For a 10-step task,
   that is 30+ calls vs ~10 for single-agent. Is the quality delta worth 3x
   token cost?
3. **Should this stay a moat** (invest to make it work, ship as premium feature)
   or be **relegated to planned** (focus on single-agent parity first)?
4. **Does the Pipeline State Machine** (omega-core/src/pipeline/state.rs)
   provide any value the single-agent chat loop does not already have
   (session state, retry tracking, scoring)?

### Research needed
- Read src-tauri/crates/omega-core/src/pipeline/build.rs fully — confirm
  step_to_tool_request generates file contents via raw LLM (not a real
  planning loop), and that the experimental gate is the only safety.
- Find published benchmarks or anecdotes about multi-agent vs single-agent
  coding agent success rates (SWE-bench, GitHub evaluations).
- Check: does Aiders plan mode (which is closer to single-agent with a
  plan scratchpad) perform better than separate-agent architectures?

## Type: grilling

## Status: open

## Assigned to: (unclaimed)
