//! Bridge module — exposes native tool definitions via the `tools/list`
//! JSON-RPC method.
//!
//! The remote-MCP bridge (`RemoteMcpClient`, `ToolBackend::Remote`,
//! `discover_remote_tools`) was removed — see
//! `docs/agents/tool-calling-cleanup/issues/03-*.md`. Production MCP
//! transport is the `.mcp.json` skill path in `omega-core/commands/mcp.rs`.

pub mod router;

pub use router::ToolRouter;
