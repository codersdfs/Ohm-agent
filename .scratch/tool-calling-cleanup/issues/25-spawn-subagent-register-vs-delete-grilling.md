Type: grilling
Status: closed
Closed by: deletion of tools/spawn_subagent.rs

## Decision (resolved)

**Delete it.** Keep subagent as a chat-loop construct, not a tool. The
lean in this ticket is the chosen lane.

Rationale: subagent is a fundamentally different execution shape (own
conversation, own loop, own state). Modeling it as a generic `Tool::call`
returning a string is a leaky abstraction. The inline branch in
`commands/chat.rs::handle_spawn_subagent` already knows the right
context (provider, emitter, conversation history) and can pass them.
A `Tool::call` cannot.

Implementation:
- Deleted: `src-tauri/crates/tool-harness/src/tools/spawn_subagent.rs`
- Removed: `mod spawn_subagent;` in `tools/mod.rs`

The inline `commands/chat.rs::handle_spawn_subagent` is the only path.
Subagent's own tool calls already route through the parent's tool
pipeline (`subagent/subagent.rs:158`), so hooks / gate / permissions /
budget already apply to subagent writes. Ticket 11 is resolved by
reversal under this decision (no `Tool` impl to wire), and its
acceptance criteria were rewritten to cover the inline path.

Verification: `cargo test --workspace` green, ~493 passed, 0 failed.
`grep -rn "SpawnSubagentTool" src-tauri/` returns no matches.
`grep -rn spawn_subagent src-tauri/crates/` returns one definition
(`pub async fn spawn_subagent` in `subagent/subagent.rs`) + one call
site (`handle_spawn_subagent` in `commands/chat.rs`).

## Question

`SpawnSubagentTool` exists as a `Tool` impl but is not in `default_tool_registry`. The actual subagent dispatch is an inline branch in `commands/chat.rs::handle_spawn_subagent`. Two options for ticket 04 to execute.

**Decision question (one question per grilling pass):** which lane?
- **Register it:** add to `default_tool_registry`, delete the inline branch. Subagent goes through pipeline (hooks, gate, budget, permissions). Requires fixing the SpawnSubagentTool impl to accept pipeline-shaped input (it currently expects a different signature than `handle_spawn_subagent` accepts).
- **Delete it:** remove `tools/spawn_subagent.rs` and the export from `tools/mod.rs`. Keep the inline branch as the only path. Lose pipeline integration but no new bugs.

**Constraint to grill on:** is subagent-as-a-tool worth the fix? Subagent is a fundamentally different execution shape (own conversation, own loop, own state). Modeling it as a "tool" that returns a string is a leaky abstraction. The inline branch in chat.rs knows the right context (provider, emitter, conversation history) and can pass them; a generic `Tool::call` cannot.

**My lean:** delete it. Keep subagent as a chat-loop construct, not a tool. The "tool" framing is what causes the duplication in the first place.

**Acceptance:** decision recorded. Ticket 04 implements.
