Type: task
Status: open (deferred — out of scope for "today" deadline)
Blocked by: 03 [DONE — bridge deleted in commit 19aae6e]

## Status

The blocker (03) is now closed (commit 19aae6e), so the unification
path is clear. The work itself is:

1. Move `mcp-server::types::{JsonRpcRequest, JsonRpcResponse,
   RequestId, JsonRpcError}` into a shared place (either re-export
   from `mcp` or extract into a new `mcp-types` microcrate).
2. Delete `mcp::McpRequest` and `mcp::McpResponse`.
3. Rewrite `mcp::transport::JsonRpcTransport::send` to serialize the
   unified type instead of building the body inline with
   `serde_json::json!({...})`.
4. Update every call site of `McpRequest` in `mcp::`.
5. Add a wire-format snapshot test to lock the JSON shape.

## Why deferred

The work is 2-3 hours of careful refactor across two crates and a
dozen call sites. The user's "finish by today" deadline prioritized
the tool-calling bugs (concurrency, truncation, bridge deletion) and
the dep/doc cleanups. The MCP wire unification is a follow-up that
can land in the next session without any user-visible regression
(both type sets are internal to the agent).

## Acceptance (still open)

- [ ] `grep -rn "JsonRpcRequest\|McpRequest" src-tauri/crates/mcp src-tauri/crates/mcp-server` returns one definition of each.
- [ ] `mcp::JsonRpcTransport::send` takes the unified type.
- [ ] Wire-format snapshot test.
- [ ] No behavior change.
