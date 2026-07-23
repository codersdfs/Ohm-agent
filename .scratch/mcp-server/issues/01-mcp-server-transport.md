# 01 — MCP Server Transport

**What to build:** A new `mcp-server` crate in `src-tauri/crates/mcp-server/` that implements the JSON-RPC 2.0 wire protocol for the MCP server side. Supports two transports:
- **HTTP+SSE** — listener on a configurable port, POST for JSON-RPC requests, GET /sse for server-sent events (streaming)
- **Stdio** — read JSON-RPC from stdin, write responses to stdout (for subprocess MCP mode)

Includes request/response framing, error formatting, and connection lifecycle management.

**Blocked by:** None — can start immediately

**Status:** ready-for-agent

- [ ] New `mcp-server` crate with `Cargo.toml` depending on `serde`, `serde_json`, `tokio`, `axum` (for HTTP), `tower-http` (for CORS/logging)
- [ ] `transport/mod.rs` — `McpTransport` trait with `send()` / `receive()` methods
- [ ] `transport/http.rs` — Axum router with POST `/json-rpc` endpoint and GET `/sse` streaming endpoint
- [ ] `transport/stdio.rs` — stdin/stdout line-delimited JSON-RPC
- [ ] `server.rs` — `McpServer` struct that manages transport + session state
- [ ] `types.rs` — JSON-RPC request/response types that match the MCP spec
- [ ] `error.rs` — MCP error codes and error responses

## How to test

```bash
# Unit tests
cd src-tauri && cargo test -p mcp-server

# Manual HTTP test (after implementing):
# Start server in one terminal:
cargo run -p mcp-server -- --port 3100

# In another terminal:
curl -X POST http://localhost:3100/json-rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"ping"}'
# Expected: {"jsonrpc":"2.0","id":"1","result":{}}
```