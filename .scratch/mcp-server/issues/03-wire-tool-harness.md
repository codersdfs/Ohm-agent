# 03 — Wire Tool-Harness into MCP Server

**What to build:** Connect the MCP server's `tools/list` and `tools/call` handlers to Ohm-agent's actual tool-harness. When a client calls `tools/list`, the server returns the same tool definitions the LLM sees (read, write, edit, bash, grep, glob, etc.). When a client calls `tools/call`, it routes through the `tool-harness::ExecutionPipeline` just like the LLM would — including gate checks, budget tracking, and result formatting.

**Blocked by:** 02 (needs protocol handlers)

**Status:** ready-for-agent

- [ ] Add `tool-harness` as a dependency to `mcp-server`
- [ ] `handlers/tools.rs` — `tools/list` returns all tool-harness tool definitions (including MCP skills from `omega-core`)
- [ ] `handlers/tools.rs` — `tools/call` creates a `ToolRequest`, runs through pipeline, returns result
- [ ] Gate check integration — run harness gate on write/edit results
- [ ] Error mapping — convert tool-harness errors to MCP error codes
- [ ] Integration test: call a real tool (e.g., `ping` or `echo` via bash) through MCP

## How to test

```bash
# Unit tests
cd src-tauri && cargo test -p mcp-server

# Manual test — list tools (should be non-empty now)
curl -X POST http://localhost:3100/json-rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"tools/list"}'
# Expected: {"jsonrpc":"2.0","id":"1","result":{"tools":[{"name":"read",...},{"name":"write",...},...]}}

# Manual test — call a tool
curl -X POST http://localhost:3100/json-rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"2","method":"tools/call","params":{"name":"read","arguments":{"path":"Cargo.toml"}}}'
# Expected: tool result with file contents or error
```