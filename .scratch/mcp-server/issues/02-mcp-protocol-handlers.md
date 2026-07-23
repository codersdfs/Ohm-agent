# 02 — MCP Protocol Handlers

**What to build:** Implement the core MCP protocol methods on top of the transport from Ticket 01. The server should handle standard MCP methods:
- `initialize` — version negotiation, return server capabilities (tools, resources, prompts)
- `ping` — return empty success
- `tools/list` — return list of available tool definitions
- `tools/call` — invoke a tool by name with parameters
- `resources/list` — return available resources (files, directories)
- `resources/read` — read a resource by URI
- `notifications/initialized` — acknowledge client initialization

Uses a `ProtocolHandler` trait so new method handlers can be registered.

**Blocked by:** 01 (needs transport)

**Status:** ready-for-agent

- [ ] `handler.rs` — `ProtocolHandler` trait with `handle(method, params) -> Result`
- [ ] `handlers/initialize.rs` — protocol version, server capabilities
- [ ] `handlers/tools.rs` — `tools/list` and `tools/call` stubs (return empty lists initially)
- [ ] `handlers/resources.rs` — `resources/list` and `resources/read` stubs
- [ ] `handlers/ping.rs` — health check
- [ ] `router.rs` — dispatch incoming JSON-RPC methods to handlers
- [ ] Integration tests covering all protocol methods

## How to test

```bash
# Unit tests
cd src-tauri && cargo test -p mcp-server

# Manual test sequence:
# 1. Initialize
curl -X POST http://localhost:3100/json-rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
# Expected: {"jsonrpc":"2.0","id":"1","result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{},"resources":{}},"serverInfo":{"name":"omega-mcp","version":"0.1.0"}}}

# 2. List tools (should return empty array for now)
curl -X POST http://localhost:3100/json-rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"2","method":"tools/list"}'
# Expected: {"jsonrpc":"2.0","id":"2","result":{"tools":[]}}
```