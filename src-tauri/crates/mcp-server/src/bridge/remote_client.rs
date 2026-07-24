//! Remote MCP client — connects to external MCP servers
//!
//! Handles the full MCP protocol lifecycle:
//! 1. `initialize` — negotiate protocol version and capabilities
//! 2. `tools/list` — discover available tools
//! 3. `tools/call` — invoke tools on the remote server
//! 4. Connection management (connect, reconnect, health check)

use crate::bridge::config::{RemoteServerConfig, TransportType};
use crate::types::*;
use mcp::transport::JsonRpcTransport;
use mcp::McpRequest;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A tool discovered from a remote MCP server
#[derive(Debug, Clone)]
pub struct RemoteTool {
    /// The tool definition from the remote server
    pub definition: McpToolDefinition,

    /// The name of the remote server this tool belongs to
    pub server_name: String,

    /// The tool's name as it should be called on the remote server
    pub remote_name: String,
}

/// Status of a remote MCP server connection
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

/// A remote MCP server client
pub struct RemoteMcpClient {
    config: RemoteServerConfig,
    transport: Arc<RwLock<Option<JsonRpcTransport>>>,
    status: RwLock<ConnectionStatus>,
    cached_tools: RwLock<Vec<RemoteTool>>,
    server_info: RwLock<Option<ServerInfo>>,
}

impl RemoteMcpClient {
    /// Create a new remote MCP client from config
    pub fn new(config: RemoteServerConfig) -> Self {
        Self {
            config,
            transport: Arc::new(RwLock::new(None)),
            status: RwLock::new(ConnectionStatus::Disconnected),
            cached_tools: RwLock::new(Vec::new()),
            server_info: RwLock::new(None),
        }
    }

    /// Connect to the remote MCP server (initialize handshake)
    pub async fn connect(&self) -> Result<(), String> {
        *self.status.write().await = ConnectionStatus::Connecting;

        match &self.config.transport {
            TransportType::Http | TransportType::HttpSse => {
                let url = self.config.url.as_ref().ok_or("No URL configured for HTTP transport")?;
                log::info!("Connecting to remote MCP server at {url}");

                let transport = JsonRpcTransport::new(url);
                *self.transport.write().await = Some(transport);

                // Do initialize handshake
                let result = self.send_request("initialize", Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "omega-mcp-bridge",
                        "version": "0.1.0"
                    }
                }))).await;

                match result {
                    Ok(response) => {
                        // Handle successful initialization
                        if let Some(err) = response.error {
                            *self.status.write().await =
                                ConnectionStatus::Failed(format!("Initialize error: {} (code {})", err.message, err.code));
                            return Err(format!("Remote server {} initialize failed: {}", self.config.name, err.message));
                        }

                        // Extract server info
                        if let Some(ref result) = response.result {
                            let si = result.get("serverInfo").and_then(|v| {
                                Some(ServerInfo {
                                    name: v.get("name")?.as_str()?.to_string(),
                                    version: v.get("version")?.as_str()?.to_string(),
                                })
                            });
                            *self.server_info.write().await = si;
                        }

                        *self.status.write().await = ConnectionStatus::Connected;
                        log::info!("Connected to remote MCP server '{}'", self.config.name);

                        // After connect, discover tools
                        self.discover_tools().await?;

                        Ok(())
                    }
                    Err(e) => {
                        *self.status.write().await =
                            ConnectionStatus::Failed(format!("Connection error: {e}"));
                        Err(format!("Failed to connect to {}: {e}", self.config.name))
                    }
                }
            }
            TransportType::Stdio { command, args } => {
                // Stdio transport — spawn process and connect via stdin/stdout
                log::info!("Starting local MCP process: {command} {args:?}");

                // For stdio, we need a different approach
                // The JsonRpcTransport only supports HTTP
                *self.status.write().await =
                    ConnectionStatus::Failed("Stdio transport not yet implemented".into());
                Err("Stdio transport for bridge not yet implemented — use HTTP transport".into())
            }
        }
    }

    /// Discover tools from the remote server via tools/list
    pub async fn discover_tools(&self) -> Result<Vec<RemoteTool>, String> {
        let response = self.send_request("tools/list", None).await?;

        if let Some(err) = response.error {
            return Err(format!("tools/list error: {} (code {})", err.message, err.code));
        }

        let tools_list = response
            .result
            .as_ref()
            .and_then(|r| r.get("tools"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| "No tools field in response".to_string())?;

        let prefix = self.config.tool_prefix.clone().unwrap_or_default();

        let mut remote_tools = Vec::new();
        for tool_val in tools_list {
            let name = tool_val.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let description = tool_val.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let input_schema = tool_val.get("inputSchema").cloned()
                .unwrap_or_else(|| serde_json::json!({"type":"object","properties":{}}));

            // Apply allow/deny filters
            if let Some(ref allow) = self.config.allow_tools {
                if !allow.iter().any(|p| name.contains(p)) {
                    continue;
                }
            }
            if let Some(ref deny) = self.config.deny_tools {
                if deny.iter().any(|p| name.contains(p)) {
                    continue;
                }
            }

            let prefixed_name = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}_{}", prefix, name)
            };

            remote_tools.push(RemoteTool {
                definition: McpToolDefinition {
                    name: prefixed_name,
                    description: format!("[{}] {}", self.config.name, description),
                    input_schema,
                },
                server_name: self.config.name.clone(),
                remote_name: name.to_string(),
            });
        }

        *self.cached_tools.write().await = remote_tools.clone();
        log::info!(
            "Discovered {} tools from remote server '{}'",
            remote_tools.len(),
            self.config.name
        );

        Ok(remote_tools)
    }

    /// Get the cached tools from this remote server
    pub async fn get_cached_tools(&self) -> Vec<RemoteTool> {
        self.cached_tools.read().await.clone()
    }

    /// Call a tool on the remote server
    pub async fn call_tool(&self, remote_name: &str, arguments: serde_json::Value) -> Result<CallToolResult, String> {
        let params = serde_json::json!({
            "name": remote_name,
            "arguments": arguments,
        });

        let response = self.send_request("tools/call", Some(params)).await?;

        if let Some(err) = response.error {
            return Err(format!(
                "Remote tool '{}' error (code {}): {}",
                remote_name, err.code, err.message
            ));
        }

        // Parse the response into CallToolResult
        let result = response.result.unwrap_or(serde_json::json!({"content": []}));
        let call_result: CallToolResult = serde_json::from_value(result)
            .map_err(|e| format!("Failed to parse tool result: {e}"))?;

        Ok(call_result)
    }

    /// Send a JSON-RPC request to the remote server
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<mcp::McpResponse, String> {
        let transport = self.transport.read().await;
        let transport = transport.as_ref().ok_or("Not connected")?;

        // Convert params to HashMap if present
        let params_map = params.and_then(|p| {
            p.as_object().map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<HashMap<String, serde_json::Value>>()
            })
        });

        let request = McpRequest {
            method: method.to_string(),
            params: params_map,
            id: uuid::Uuid::new_v4().to_string(),
        };

        transport.send(request).await
    }

    /// Get the current connection status
    pub async fn status(&self) -> ConnectionStatus {
        self.status.read().await.clone()
    }

    /// Get the server info from initialize response
    pub async fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }

    /// Get the config
    pub fn config(&self) -> &RemoteServerConfig {
        &self.config
    }

    /// Disconnect from the remote server
    pub async fn disconnect(&self) {
        *self.status.write().await = ConnectionStatus::Disconnected;
        *self.transport.write().await = None;
        *self.cached_tools.write().await = Vec::new();
        log::info!("Disconnected from remote MCP server '{}'", self.config.name);
    }

    /// Check if connected and attempt reconnect if not
    pub async fn ensure_connected(&self) -> Result<(), String> {
        for attempt in 0..5 {
            let status = self.status.read().await.clone();
            match status {
                ConnectionStatus::Connected => return Ok(()),
                ConnectionStatus::Disconnected | ConnectionStatus::Failed(_) => {
                    if attempt > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
                    }
                    self.connect().await?;
                }
                ConnectionStatus::Connecting => {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            }
        }
        Err(format!("Failed to connect to '{}' after 5 attempts", self.config.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client_config() {
        let config = RemoteServerConfig::http("test-server", "http://localhost:9999");
        let client = RemoteMcpClient::new(config);
        assert_eq!(client.config().name, "test-server");
    }

    #[tokio::test]
    async fn test_status_starts_disconnected() {
        let config = RemoteServerConfig::http("test-server", "http://localhost:9999");
        let client = RemoteMcpClient::new(config);
        assert_eq!(client.status().await, ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_disconnect_is_idempotent() {
        let config = RemoteServerConfig::http("test-server", "http://localhost:9999");
        let client = RemoteMcpClient::new(config);
        client.disconnect().await;
        client.disconnect().await;
        assert_eq!(client.status().await, ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_connect_to_nonexistent_server_fails_gracefully() {
        // Use a port that's unlikely to be in use, with a short timeout
        let mut config = RemoteServerConfig::http("nowhere", "http://127.0.0.1:1");
        config.timeout_seconds = 2;
        let client = RemoteMcpClient::new(config);
        let result = client.connect().await;
        assert!(result.is_err(), "Connecting to nonexistent server should fail");
    }

    #[test]
    fn test_remote_tool_creation() {
        let tool = RemoteTool {
            definition: McpToolDefinition {
                name: "figma_get_file".into(),
                description: "[figma] Get Figma file data".into(),
                input_schema: serde_json::json!({"type":"object","properties":{}}),
            },
            server_name: "figma".into(),
            remote_name: "get_file".into(),
        };
        assert_eq!(tool.server_name, "figma");
        assert!(tool.definition.description.starts_with("[figma]"));
    }
}