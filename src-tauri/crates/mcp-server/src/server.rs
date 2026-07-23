//! MCP Server — core server logic
//!
//! The `McpServer` manages the MCP protocol lifecycle:
//! 1. Accept client connections via a transport
//! 2. Handle initialization handshake
//! 3. Route method calls to registered handlers
//! 4. Manage session state

use crate::error::McpError;
use crate::transport::{McpTransport, ReceiveResult};
use crate::types::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::RwLock as TokioRwLock;

/// Handler for a specific MCP method
pub type MethodHandler = Arc<
    dyn Fn(serde_json::Value) -> Result<serde_json::Value, McpError> + Send + Sync,
>;

/// Async handler for a specific MCP method
pub type AsyncMethodHandler = Arc<
    dyn Fn(serde_json::Value) -> Result<serde_json::Value, McpError> + Send + Sync,
>;

/// Session state for a connected MCP client
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Whether the client has completed initialization
    pub initialized: bool,

    /// Client info from initialization
    pub client_info: Option<ClientInfo>,

    /// Protocol version negotiated
    pub protocol_version: String,

    /// Session start time
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            initialized: false,
            client_info: None,
            protocol_version: MCP_PROTOCOL_VERSION.into(),
            started_at: chrono::Utc::now(),
        }
    }
}

/// MCP Server configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub server_name: String,
    pub server_version: String,
    pub capabilities: McpServerCapabilities,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            server_name: "omega-mcp".into(),
            server_version: "0.1.0".into(),
            capabilities: McpServerCapabilities {
                tools: Some([("list".into(), serde_json::json!({}))].into()),
                resources: Some([("list".into(), serde_json::json!({}))].into()),
                prompts: None,
                logging: None,
            },
        }
    }
}

/// The MCP server — routes JSON-RPC requests to registered handlers
pub struct McpServer {
    config: McpServerConfig,
    session: TokioRwLock<SessionState>,
    handlers: RwLock<HashMap<String, AsyncMethodHandler>>,
}

impl McpServer {
    /// Create a new MCP server with default configuration
    pub fn new() -> Self {
        let server = Self {
            config: McpServerConfig::default(),
            session: TokioRwLock::new(SessionState::default()),
            handlers: RwLock::new(HashMap::new()),
        };
        server.register_builtin_handlers();
        server
    }

    /// Create with custom configuration
    pub fn with_config(config: McpServerConfig) -> Self {
        let server = Self {
            config,
            session: TokioRwLock::new(SessionState::default()),
            handlers: RwLock::new(HashMap::new()),
        };
        server.register_builtin_handlers();
        server
    }

    /// Register a method handler
    pub fn register_handler(&self, method: &str, handler: AsyncMethodHandler) {
        let mut handlers = self.handlers.write().unwrap();
        handlers.insert(method.to_string(), handler);
    }

    /// Register a sync method handler (wrapped in a closure)
    pub fn register_sync_handler<F>(&self, method: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> Result<serde_json::Value, McpError> + Send + Sync + 'static,
    {
        self.register_handler(method, Arc::new(handler));
    }

    /// Register built-in handlers (initialize, ping)
    fn register_builtin_handlers(&self) {
        let config = self.config.clone();
        let _session_ptr = Arc::new(RwLock::new(SessionState::default()));

        // We need a way to share session state with handlers
        // For now, use the server's session directly via a captured handle

        // initialize handler
        self.register_sync_handler("initialize", move |params| {
            let init_params: InitializeParams =
                serde_json::from_value(params.clone()).map_err(|e| {
                    McpError::invalid_params(format!("Invalid initialize params: {e}"))
                })?;

            if init_params.protocol_version != MCP_PROTOCOL_VERSION {
                log::warn!(
                    "Client requested protocol version {}, server supports {}",
                    init_params.protocol_version,
                    MCP_PROTOCOL_VERSION
                );
            }

            Ok(serde_json::to_value(InitializeResult {
                protocol_version: MCP_PROTOCOL_VERSION.into(),
                capabilities: config.capabilities.clone(),
                server_info: ServerInfo {
                    name: config.server_name.clone(),
                    version: config.server_version.clone(),
                },
            })
            .map_err(|e| McpError::internal_error(format!("Serialize error: {e}")))?)
        });

        // ping handler
        self.register_sync_handler("ping", |_| {
            Ok(serde_json::json!({}))
        });

        // tools/list handler (stub — returns empty list)
        self.register_sync_handler("tools/list", |_| {
            Ok(serde_json::to_value(ListToolsResult {
                tools: vec![],
            })
            .map_err(|e| McpError::internal_error(format!("Serialize error: {e}")))?)
        });

        // tools/call handler (stub — returns not found)
        self.register_sync_handler("tools/call", |params| {
            let call_params: CallToolParams =
                serde_json::from_value(params).map_err(|e| {
                    McpError::invalid_params(format!("Invalid call params: {e}"))
                })?;

            Err(McpError::tool_not_found(call_params.name))
        });

        // resources/list handler (stub — returns empty list)
        self.register_sync_handler("resources/list", |_| {
            Ok(serde_json::to_value(ListResourcesResult {
                resources: vec![],
            })
            .map_err(|e| McpError::internal_error(format!("Serialize error: {e}")))?)
        });

        // resources/read handler (stub)
        self.register_sync_handler("resources/read", |_| {
            Err(McpError::method_not_found("resources/read"))
        });
    }

    /// Handle a single JSON-RPC request
    pub async fn handle_request(&self, body: &str) -> String {
        // Parse the JSON-RPC request
        let raw: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_e) => {
                let err = McpError::parse_error();
                return serde_json::to_string(
                    &err.to_json_rpc_response(RequestId::Null),
                )
                .unwrap_or_default();
            }
        };

        // Extract request ID
        let request_id = raw
            .get("id")
            .and_then(extract_request_id)
            .unwrap_or(RequestId::Null);

        // Check if it's a notification (no id field)
        let is_notification = raw.get("id").is_none();

        // Extract method and params
        let method = match raw.get("method").and_then(|v| v.as_str()) {
            Some(m) => m.to_string(),
            None => {
                let err = McpError::invalid_request();
                return serde_json::to_string(&err.to_json_rpc_response(request_id))
                    .unwrap_or_default();
            }
        };

        let params = raw.get("params").cloned().unwrap_or(serde_json::Value::Null);

        // Handle notifications specially
        if is_notification {
            // For notifications (no id), we don't send a response
            if method == "notifications/initialized" {
                let mut session = self.session.write().await;
                session.initialized = true;
                log::info!("Client initialized");
            }
            // Notifications get no response
            return String::new();
        }

        // Look up the handler
        let response = {
            let handlers = self.handlers.read().unwrap();
            match handlers.get(&method) {
                Some(handler) => {
                    match handler(params) {
                        Ok(result) => JsonRpcResponse::success(request_id, result),
                        Err(err) => err.to_json_rpc_response(request_id),
                    }
                }
                None => {
                    let err = McpError::method_not_found(&method);
                    err.to_json_rpc_response(request_id)
                }
            }
        };

        serde_json::to_string(&response).unwrap_or_default()
    }

    /// Run the server loop over a transport
    pub async fn run(&self, transport: &dyn McpTransport) -> Result<(), String> {
        log::info!(
            "MCP server '{}' v{} starting on {}",
            self.config.server_name,
            self.config.server_version,
            transport.transport_type()
        );

        loop {
            match transport.receive().await {
                Ok(ReceiveResult::Message(msg)) => {
                    let response = self.handle_request(&msg.body).await;
                    if !response.is_empty() {
                        // Create a JsonRpcResponse from the response string
                        if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&response) {
                            transport.send(&resp).await.ok();
                        }
                    }
                }
                Ok(ReceiveResult::Closed) => {
                    log::info!("MCP transport closed");
                    break;
                }
                Ok(ReceiveResult::Timeout) => {
                    // Timeout is normal — continue polling
                    continue;
                }
                Err(e) => {
                    log::error!("MCP transport error: {e}");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Get the server config
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }

    /// Get session state
    pub async fn session(&self) -> SessionState {
        self.session.read().await.clone()
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a RequestId from a JSON value
fn extract_request_id(value: &serde_json::Value) -> Option<RequestId> {
    match value {
        serde_json::Value::String(s) => Some(RequestId::Str(s.clone())),
        serde_json::Value::Number(n) => n.as_i64().map(RequestId::Num),
        _ => Some(RequestId::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_default_config() {
        let server = McpServer::new();
        assert_eq!(server.config().server_name, "omega-mcp");
        assert!(server.config().capabilities.tools.is_some());
    }

    #[tokio::test]
    async fn test_handle_ping() {
        let server = McpServer::new();
        let response = server
            .handle_request(r#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#)
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["id"], "1");
        assert!(parsed["result"].is_object());
        assert!(parsed["error"].is_null());
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let server = McpServer::new();
        let response = server
            .handle_request(
                r#"{"jsonrpc":"2.0","id":"1","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#,
            )
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["id"], "1");
        assert_eq!(
            parsed["result"]["protocolVersion"],
            "2024-11-05"
        );
        assert_eq!(parsed["result"]["serverInfo"]["name"], "omega-mcp");
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let server = McpServer::new();
        let response = server
            .handle_request(r#"{"jsonrpc":"2.0","id":"1","method":"foobar"}"#)
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn test_handle_parse_error() {
        let server = McpServer::new();
        let response = server.handle_request("not valid json").await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], PARSE_ERROR);
    }

    #[tokio::test]
    async fn test_handle_notification() {
        let server = McpServer::new();
        // Notifications have no id field
        let response = server
            .handle_request(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .await;
        assert!(response.is_empty(), "Notifications should not produce a response");
    }

    #[tokio::test]
    async fn test_tools_list_stub() {
        let server = McpServer::new();
        let response = server
            .handle_request(r#"{"jsonrpc":"2.0","id":"1","method":"tools/list"}"#)
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert!(parsed["result"]["tools"].is_array());
        assert!(parsed["result"]["tools"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_tools_call_stub_returns_not_found() {
        let server = McpServer::new();
        let response = server
            .handle_request(
                r#"{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"read","arguments":{"path":"test.txt"}}}"#,
            )
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], MCP_TOOL_NOT_FOUND);
    }

    #[tokio::test]
    async fn test_session_starts_uninitialized() {
        let server = McpServer::new();
        let session = server.session().await;
        assert!(!session.initialized);
    }

    #[tokio::test]
    async fn test_initialization_sets_session() {
        let server = McpServer::new();
        // Send initialize
        server
            .handle_request(
                r#"{"jsonrpc":"2.0","id":"1","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-cli","version":"1.0"}}}"#,
            )
            .await;
        // Send notification (no id)
        server
            .handle_request(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .await;
        let session = server.session().await;
        assert!(session.initialized); // This will fail until we wire initialize to update session
    }
}