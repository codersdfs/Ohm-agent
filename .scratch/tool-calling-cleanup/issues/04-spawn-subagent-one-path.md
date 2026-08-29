Type: task
Status: closed
Closed by: deletion of tools/spawn_subagent.rs (ticket 25 decision)
Blocked by: 25

## Resolution

Ticket 25 chose "delete it." `tools/spawn_subagent.rs` (the deprecated
`SpawnSubagentTool` impl) was removed. `mod spawn_subagent;` was
dropped from `tools/mod.rs`. The inline `commands/chat.rs::handle_spawn_subagent`
is now the only path.

Acceptance from this ticket:
- [x] Exactly one `spawn_subagent` code path exists.
- [x] `tools/spawn_subagent.rs` is gone.
- [x] `grep -rn spawn_subagent src-tauri/crates/` returns one definition
      (the function in `subagent/subagent.rs`) + one call site
      (`handle_spawn_subagent` in `commands/chat.rs`).
- [x] `cargo test --workspace` green.

The remaining inline path does NOT go through the standard pipeline
(`execute_tool_inner`), but the subagent's **own** tool calls do route
through the parent's tool pipeline — see ticket 11 for the follow-up
to that wiring (acceptance criteria there were rewritten under the
delete-it decision).

## Question

Two implementations of `spawn_subagent` exist:
- `tools/spawn_subagent.rs` (a `Tool` impl) — exported from `tools/mod.rs` but **not** in `default_tool_registry`.
- `commands/chat.rs::handle_spawn_subagent` — the actual handler, called inline before `execute_tool_inner`.

So the LLM sees the `spawn_subagent` tool definition (from `tool_definitions()` in chat.rs) but the dispatch never goes through the pipeline. If someone re-adds it to `default_tool_registry`, the LLM sees a duplicate definition and dispatch is undefined.

**Decision (already settled by ticket 25 grilling):** register it in the registry and remove the inline intercept, OR delete `tools/spawn_subagent.rs` entirely. After ticket 25 resolves, this ticket just does the chosen lane.

**Acceptance:**
- Exactly one `spawn_subagent` code path exists.
- `commands/chat.rs::handle_spawn_subagent` is gone (or `tools/spawn_subagent.rs` is gone).
- The remaining path goes through the standard pipeline (hooks, gate, budget, permissions all apply).
- `grep -rn spawn_subagent src-tauri/crates/` returns one definition + one call site.
