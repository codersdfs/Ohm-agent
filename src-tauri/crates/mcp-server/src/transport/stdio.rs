//! Stdio transport for MCP server
//!
//! Reads JSON-RPC requests from stdin, writes responses to stdout.
//! Line-delimited JSON — one JSON object per line.
//! Used for subprocess MCP mode (e.g., when another process spawns the server).

use crate::transport::{McpTransport, ReceiveResult, TransportMessage};
use crate::types::JsonRpcResponse;
use async_trait::async_trait;
use std::io::{self, Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Stdio transport — reads stdin, writes stdout
pub struct StdioTransport {
    stdin: Arc<Mutex<io::Stdin>>,
    stdout: Arc<Mutex<io::Stdout>>,
    buffer_size: usize,
}

impl StdioTransport {
    /// Create a new stdio transport
    pub fn new() -> Self {
        Self {
            stdin: Arc::new(Mutex::new(io::stdin())),
            stdout: Arc::new(Mutex::new(io::stdout())),
            buffer_size: 1024 * 1024, // 1MB max line
        }
    }

    /// Create with a custom buffer size
    pub fn with_buffer_size(size: usize) -> Self {
        Self {
            stdin: Arc::new(Mutex::new(io::stdin())),
            stdout: Arc::new(Mutex::new(io::stdout())),
            buffer_size: size,
        }
    }

    /// Read one JSON-RPC line from stdin (blocking)
    fn read_line_blocking(stdin: &Mutex<io::Stdin>, buffer_size: usize) -> Result<ReceiveResult, String> {
        let mut handle = stdin.try_lock().map_err(|_| "Stdin lock contention")?;
        let mut line = String::with_capacity(4096);

        // Read a single line (up to buffer_size)
        let mut bytes_read: usize = 0;
        loop {
            let mut buf = [0u8; 1];
            match handle.read(&mut buf) {
                Ok(0) => return Ok(ReceiveResult::Closed),
                Ok(_) => {
                    let c = buf[0] as char;
                    if c == '\n' {
                        break;
                    }
                    line.push(c);
                    bytes_read += 1;
                    if bytes_read > buffer_size {
                        return Err("Line exceeds max buffer size".into());
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(format!("Stdin read error: {e}")),
            }
        }

        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            return Ok(ReceiveResult::Timeout);
        }

        Ok(ReceiveResult::Message(TransportMessage {
            id: "stdio".into(),
            body: trimmed,
        }))
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send(&self, response: &JsonRpcResponse) -> Result<(), String> {
        let json = serde_json::to_string(response)
            .map_err(|e| format!("Serialize error: {e}"))?;

        let mut handle = self.stdout.try_lock().map_err(|_| "Stdout lock contention")?;
        writeln!(handle, "{}", json).map_err(|e| format!("Stdout write error: {e}"))?;
        handle.flush().map_err(|e| format!("Stdout flush error: {e}"))?;
        log::debug!("Stdio transport: sent response");
        Ok(())
    }

    async fn receive(&self) -> Result<ReceiveResult, String> {
        // Create an Arc to share the stdin mutex with the blocking task
        let stdin = self.stdin.clone();
        let buffer_size = self.buffer_size;

        tokio::task::spawn_blocking(move || {
            Self::read_line_blocking(&stdin, buffer_size)
        })
        .await
        .map_err(|e| format!("Spawn blocking failed: {e}"))?
    }

    fn transport_type(&self) -> &str {
        "stdio"
    }

    async fn close(&self) -> Result<(), String> {
        log::info!("MCP stdio transport closing");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_type() {
        let transport = StdioTransport::new();
        assert_eq!(transport.transport_type(), "stdio");
    }
}