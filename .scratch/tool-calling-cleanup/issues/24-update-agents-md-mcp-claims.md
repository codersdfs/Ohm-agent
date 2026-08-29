Type: task
Status: closed (no change needed)
Closed by: code review — AGENTS.md does not claim mcp-server is the MCP client.

## Resolution

AGENTS.md mentions only `mcp` (`mcp/`) and never `mcp-server`. The
table row is accurate: `mcp` is "MCP JSON-RPC client + skills registry".
The dependency chain correctly omits `mcp-server` from the omega-core
list (because omega-core does not depend on mcp-server).

`mcp-server` lives in `src-tauri/crates/mcp-server/` and is built as
a separate sidecar binary invoked via `omega-cli::dispatch::run_mcp_server`.
It is not part of the omega-core dependency graph and is not mentioned
in AGENTS.md. No new contributor confusion to address.

## Acceptance

- [x] `grep -in mcp AGENTS.md` shows only the `mcp` row, which is accurate.
- [x] No edits to AGENTS.md needed.

ponytail: the wider doc accuracy audit (does AGENTS.md mention every
crate that exists?) is a separate, broader task. `mcp-server` could
be added to the table for completeness (it is a real crate and a
newcomer might wonder where it is), but that is additive and not a
correction.
