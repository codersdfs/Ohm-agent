Type: task
Status: closed (no change needed — default IS the configuration)
Closed by: code review

## Resolution

After code review, the ticket's premise is wrong:

- `McpServerConfig::default()` is *not* empty: it sets
  `server_name: "omega-mcp"`, `server_version: "0.1.0"`, and
  `capabilities` with tools/resources entries.
- `McpServer::new()` (server.rs:97) calls `McpServerConfig::default()`
  directly. The default is exactly the configuration the zero-arg
  constructor uses.
- `McpServer::with_config(config)` accepts a fully-built `McpServerConfig`
  for the cases that need custom fields.

The default is the production configuration, not a placeholder. The
ticket's suggestion to delete it would break `McpServer::new()`.

## Acceptance

- [x] `git grep "impl Default for McpServerConfig"` shows the impl
      still exists, because the zero-arg constructor depends on it.
- [x] `McpServer::new` still compiles and works.
- [x] All tests pass.
