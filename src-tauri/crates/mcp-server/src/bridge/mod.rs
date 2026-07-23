//! Bridge module — connects to remote MCP servers and exposes their tools
//!
//! This enables the MCP server to act as a hub/aggregator that merges tools
//! from multiple external MCP servers (Figma, 21st.dev, local servers, etc.)
//! with Ohm-agent's native tools.

pub mod config;
pub mod remote_client;
pub mod router;

pub use config::RemoteServerConfig;
pub use remote_client::RemoteMcpClient;
pub use router::ToolRouter;