Type: task
Status: open (acceptance criteria rewritten)
Closed by: deletion of tools/spawn_subagent.rs (ticket 25 decision)
Blocked by: 04

## Resolution (decision: delete it)

Under ticket 25's decision, there is no `Tool` impl to wire. The inline
`commands/chat.rs::handle_spawn_subagent` dispatches to
`subagent::spawn_subagent`, which builds its tool executor by
**delegating to the parent's tool pipeline** (`subagent/subagent.rs:158`:
`Build the tool executor — delegates to the parent's tool pipeline`).
So hooks / gate / permissions / budget already apply to subagent writes.

The original ticket's "rewrite the test gap" intent is still real —
subagent's interaction with the pipeline has thin coverage. The original
acceptance criteria referenced a `Tool` impl that no longer exists;
they have been rewritten below.

## New acceptance (under delete-it decision)

- [ ] A test asserts: a `GateHook` configured to block also blocks a
      subagent's write calls (drive the inline `handle_spawn_subagent`
      path with a write-class subagent task and a blocking GateHook).
- [ ] A test asserts: a subagent whose token budget is exceeded returns
      a structured `RunOutcome` with the `BudgetExceeded` reason
      (not a panic, not a silent return).
- [ ] A test asserts: `handle_spawn_subagent` does not run a write
      tool that is not in `tool_whitelist` (currently the whitelist
      is documented but enforcement is via the parent's
      `is_concurrency_safe` partition — verify it).
- [ ] `cargo test --workspace` green; the new tests live under
      `omega-core/src/subagent/subagent.rs` (or a new test module).

## Original question

Currently `handle_spawn_subagent` in `commands/chat.rs` is an inline intercept: it bypasses `execute_tool_inner`, so subagent invocations get none of the pipeline's hook/permission/gate/budget enforcement. It's also a 10-parameter function with no test.

After ticket 04 (one `spawn_subagent` path), the remaining path is a `Tool` impl, dispatched through `execute_tool_inner` like every other tool. This ticket is the *follow-on* — make sure the `Tool` impl wires hooks/gate/permissions/budget correctly, and that the test gap is closed.

**Acceptance:**
- `SpawnSubagentTool::call` (or whatever the post-04 name is) is testable in isolation.
- A test asserts: a `GateHook` configured to block also blocks a subagent's `write` calls.
- A test asserts: a subagent whose `spawn_subagent` call exceeds budget gets a `BudgetCheck` back.
- `commands/chat.rs::handle_spawn_subagent` is gone.
