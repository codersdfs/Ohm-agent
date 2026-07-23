# 04 — CLI Integration (omega serve-mcp)

**What to build:** Add an `omega serve-mcp` subcommand that starts the MCP server. Configurable via CLI flags and config file. Graceful shutdown on SIGINT/SIGTERM.

**Blocked by:** 03 (needs working server with tools)

**Status:** ready-for-agent

- [ ] Add `serve-mcp` subcommand to `omega-cli` using clap
- [ ] Flags: `--port` (default 3100), `--host` (default 127.0.0.1), `--auth-token`, `--skills-dir`
- [ ] Config file support: `~/.config/omega-agent/mcp-server.json`
- [ ] Startup banner showing connection info and loaded tools count
- [ ] Graceful shutdown with Ctrl+C
- [ ] Logging: enabled/disabled connections, tool calls with timing
- [ ] Integration test: start server, connect, call tools, verify shutdown

## How to test

```bash
# Start server
cargo run -p omega -- serve-mcp --port 3100
# Expected: "Omega MCP Server running on http://127.0.0.1:3100"

# In another terminal, verify it works end-to-end:
curl -X POST http://localhost:3100/json-rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"tools/list"}'
# Expected: non-empty tool list

# Test with auth token:
cargo run -p omega -- serve-mcp --port 3101 --auth-token "secret123"
curl -X POST http://localhost:3101/json-rpc \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer secret123" \
  -d '{"jsonrpc":"2.0","id":"1","method":"ping"}'
# Expected: success

# Test auth failure (wrong token):
curl -X POST http://localhost:3101/json-rpc \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer wrong" \
  -d '{"jsonrpc":"2.0","id":"1","method":"ping"}'
# Expected: 401 Unauthorized
```