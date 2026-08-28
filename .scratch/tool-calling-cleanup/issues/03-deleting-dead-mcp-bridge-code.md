Type: task
Status: closed (scope narrowed by code analysis)
Closed by: code review — the bridge router is *active* (tools/list + tools/call fallback), not dead.

## What this ticket got right

- `RemoteMcpClient` (`mcp-server/src/bridge/remote_client.rs`, ~387 LOC): **dead**. Zero callers register a remote. The MCP transport in production is `.mcp.json` + `mcp::JsonRpcTransport`, not the bridge.
- `ToolBackend::Remote` path in `router::call_tool`: **dead**. Native tools error out with "use server's native pipeline" — the router never successfully routes a call; the pipeline always handles the actual dispatch.
- `merge_mcp_tools` / `mcp_tools_mut` in `tool-harness/src/registry.rs`: **dead** (per ticket 32 confirmation). Can be deleted.

## What this ticket got wrong

- `ToolRouter` is **not** dead. It is the source of truth for `tools/list` (the `register_async_handler("tools/list", ...)` handler reads `router.list_tools()` and `router.discover_remote_tools()` as a fallback). Deleting the router breaks `tools/list`.
- The "0 callers" claim about `call_tool` was a misread — `call_tool` is wired in `server.rs:288` (router_for_call) and is hit on every `tools/call` request; it just always returns the "use pipeline" error, so the pipeline-fallback branch is what actually dispatches.

## Decision

**Narrower scope (today):** delete the dead `RemoteMcpClient`, the dead `ToolBackend::Remote` arm, and the dead `ToolRegistry::merge_mcp_tools` / `mcp_tools_mut` methods. Keep the `ToolRouter` (it serves `tools/list`).

ponytail: rewriting `tools/list` to read `tool_definitions` directly (skipping the router) and rewriting `tools/call` to skip the dead "try router first" branch is a separate cleanup. Marked as `mcp-server-cleanup-followup` in the test debt ledger.

## Acceptance

- [x] One chosen option (`A but narrower`), the wider `Option B (wire it)` ruled out as a separate wayfinder if/when remote MCP is needed.
- [ ] `git grep RemoteMcpClient src-tauri/crates/` returns nothing.
- [ ] `git grep ToolBackend::Remote src-tauri/crates/` returns nothing.
- [ ] `git grep merge_mcp_tools mcp_tools_mut src-tauri/crates/` returns nothing.
- [ ] `mcp-server` still compiles; the sidecar binary still works; existing `tools/list` and `tools/call` tests pass.

## Question (original)

Verified dead code (ticket 32 confirms):
- `mcp-server/src/bridge/router.rs` (~275 LOC) — `ToolRouter::register_remote`, `discover_remote_tools`, `call_tool` have 0 callers.
- `mcp-server/src/bridge/remote_client.rs` (~387 LOC) — `RemoteMcpClient::connect`, `discover_tools`, `call_tool` have 0 callers.
- `tool-harness/src/registry.rs::merge_mcp_tools`, `mcp_tools_mut` — 0 callers.
- `ToolRegistry::mcp_tools` HashMap — only the dead methods touch it.
