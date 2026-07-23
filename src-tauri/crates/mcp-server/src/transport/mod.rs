//! Transport layer for MCP server
//!
//! Defines transport abstractions and implementations for MCP connections.
//! Two transports are available:
//! - **HTTP+SSE** — POST JSON-RPC, GET /sse for streaming
//! - **Stdio** — line-delimited JSON-RPC over stdin/stdout

pub mod http;
pub mod stdio;

use async_trait::async_trait;
use crate::types::JsonRpcResponse;

/// A message from a transport connection
#[derive(Debug)]
pub struct TransportMessage {
    pub id: String,
    pub body: String,
}

/// Result of receiving a message
#[derive(Debug)]
pub enum ReceiveResult {
    /// A complete JSON-RPC request string was received
    Message(TransportMessage),
    /// Transport closed
    Closed,
    /// Timeout (for polling-based transports)
    Timeout,
}

/// Transport trait for MCP connections
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC response
    async fn send(&self, response: &JsonRpcResponse) -> Result<(), String>;

    /// Receive a JSON-RPC request (line / frame)
    async fn receive(&self) -> Result<ReceiveResult, String>;

    /// Get the transport type name
    fn transport_type(&self) -> &str;

    /// Close the transport
    async fn close(&self) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receive_result_debug() {
        let msg = TransportMessage {
            id: "test-id".into(),
            body: r#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#.into(),
        };
        let result = ReceiveResult::Message(msg);
        assert!(format!("{:?}", result).contains("test-id"));
    }
}