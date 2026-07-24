//! MCP Server — core server logic
//!
//! The `McpServer` manages the MCP protocol lifecycle:
//! 1. Accept client connections via a transport
//! 2. Handle initialization handshake
//! 3. Route method calls to registered handlers
//! 4. Manage session state

use crate::bridge;
use crate::error::McpError;
use crate::transport::{McpTransport, ReceiveResult};
use crate::types::*;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use tokio::sync::RwLock as TokioRwLock;

/// Synchronous handler for a specific MCP method
pub type SyncHandler = Arc<
    dyn Fn(serde_json::Value) -> Result<serde_json::Value, McpError> + Send + Sync,
>;

/// Asynchronous handler for a specific MCP method (e.g., tool calls)
pub type AsyncHandlerPinned =
    Arc<
        dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, McpError>> + Send>>
            + Send
            + Sync,
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
    sync_handlers: RwLock<HashMap<String, SyncHandler>>,
    async_handlers: RwLock<HashMap<String, AsyncHandlerPinned>>,
    tool_pipeline: Option<Arc<tool_harness::ExecutionPipeline>>,
    tool_definitions: RwLock<Vec<McpToolDefinition>>,
    tool_router: Arc<bridge::ToolRouter>,
}

impl McpServer {
    /// Create a new MCP server with default configuration
    pub fn new() -> Self {
        let server = Self {
            config: McpServerConfig::default(),
            session: TokioRwLock::new(SessionState::default()),
            sync_handlers: RwLock::new(HashMap::new()),
            async_handlers: RwLock::new(HashMap::new()),
            tool_pipeline: None,
            tool_definitions: RwLock::new(Vec::new()),
            tool_router: Arc::new(bridge::ToolRouter::new()),
        };
        server.register_builtin_handlers();
        server
    }

    /// Create with custom configuration
    pub fn with_config(config: McpServerConfig) -> Self {
        let server = Self {
            config,
            session: TokioRwLock::new(SessionState::default()),
            sync_handlers: RwLock::new(HashMap::new()),
            async_handlers: RwLock::new(HashMap::new()),
            tool_pipeline: None,
            tool_definitions: RwLock::new(Vec::new()),
            tool_router: Arc::new(bridge::ToolRouter::new()),
        };
        server.register_builtin_handlers();
        server
    }

    /// Set the tool registry to expose via MCP
    pub fn with_tool_registry(mut self, registry: tool_harness::ToolRegistry) -> Self {
        // Extract tool definitions upfront
        let defs: Vec<McpToolDefinition> = registry.tool_definitions().into_iter().map(|td| {
            McpToolDefinition {
                name: td.function.name,
                description: td.function.description,
                input_schema: td.function.parameters,
            }
        }).collect();

        // Register with the router (sync — called from builder context)
        self.tool_router.register_native_tools_blocking(defs.clone());

        *self.tool_definitions.write().unwrap() = defs;

        let pipeline = tool_harness::ExecutionPipeline::new()
            .with_registry(registry);
        self.tool_pipeline = Some(Arc::new(pipeline));
        self.register_tool_handlers();
        self
    }

    /// Register a sync method handler
    pub fn register_handler(&self, method: &str, handler: SyncHandler) {
        let mut handlers = self.sync_handlers.write().unwrap();
        handlers.insert(method.to_string(), handler);
    }

    /// Register a sync method handler (wrapped in a closure)
    pub fn register_sync_handler<F>(&self, method: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> Result<serde_json::Value, McpError> + Send + Sync + 'static,
    {
        self.register_handler(method, Arc::new(handler));
    }

    /// Register an async method handler
    pub fn register_async_handler<F, Fut>(&self, method: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<serde_json::Value, McpError>> + Send + 'static,
    {
        let mut handlers = self.async_handlers.write().unwrap();
        handlers.insert(method.to_string(), Arc::new(move |params| {
            Box::pin(handler(params))
        }));
    }

    /// Register built-in handlers (initialize, ping, resources)
    fn register_builtin_handlers(&self) {
        let config = self.config.clone();

        // initialize handler
        let caps_for_init = config.capabilities.clone();
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
                capabilities: caps_for_init.clone(),
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

        // resources/list handler (stub — returns empty list)
        let res_caps = config.capabilities.clone();
        self.register_sync_handler("resources/list", move |_| {
            let resources = res_caps.resources.as_ref()
                .and_then(|r| r.get("list"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let defs: Vec<ResourceDefinition> = resources.iter().filter_map(|r| {
                let uri = r.get("uri")?.as_str()?.to_string();
                let name = r.get("name")?.as_str()?.to_string();
                Some(ResourceDefinition {
                    uri,
                    name,
                    description: r.get("description").and_then(|v| v.as_str()).map(String::from),
                    mime_type: r.get("mimeType").and_then(|v| v.as_str()).map(String::from),
                })
            }).collect();
            Ok(serde_json::to_value(ListResourcesResult { resources: defs })
                .map_err(|e| McpError::internal_error(format!("Serialize error: {e}")))?)
        });

        // resources/read handler (stub)
        self.register_sync_handler("resources/read", |_| {
            Err(McpError::method_not_found("resources/read"))
        });

        // Register tool handlers as stubs initially (overridden if tool registry is set)
        self.register_stub_tool_handlers();
    }

    /// Register stub tool handlers (return empty list / not found)
    fn register_stub_tool_handlers(&self) {
        self.register_sync_handler("tools/list", |_| {
            Ok(serde_json::to_value(ListToolsResult {
                tools: vec![],
            })
            .map_err(|e| McpError::internal_error(format!("Serialize error: {e}")))?)
        });

        self.register_sync_handler("tools/call", |params| {
            let call_params: CallToolParams =
                serde_json::from_value(params).map_err(|e| {
                    McpError::invalid_params(format!("Invalid call params: {e}"))
                })?;
            Err(McpError::tool_not_found(call_params.name))
        });
    }

    /// Register real tool handlers backed by the tool-harness pipeline
    fn register_tool_handlers(&self) {
        let pipeline = match &self.tool_pipeline {
            Some(p) => p.clone(),
            None => return,
        };

        // tools/list — return merged tool definitions from router
        let router_for_list = self.tool_router.clone();
        self.register_async_handler("tools/list", move |_params| {
            let router = router_for_list.clone();
            async move {
                let tools = router.list_tools().await;
                // Fall back to stored definitions if router is empty
                if tools.is_empty() {
                    // Try fresh discovery from remote servers
                    let _ = router.discover_remote_tools().await;
                    let _tools = router.list_tools().await;
                }
                Ok(serde_json::to_value(ListToolsResult { tools })
                    .map_err(|e| McpError::internal_error(format!("Serialize error: {e}")))?)
            }
        });

        // tools/call — try router first, then native pipeline
        let router_for_call = self.tool_router.clone();
        let pipeline_for_call = pipeline.clone();
        self.register_async_handler("tools/call", move |params| {
            let router = router_for_call.clone();
            let pipeline = pipeline_for_call.clone();
            async move {
                let call_params: CallToolParams =
                    serde_json::from_value(params).map_err(|e| {
                        McpError::invalid_params(format!("Invalid call params: {e}"))
                    })?;

                let tool_name = call_params.name;
                let tool_args = call_params.arguments.unwrap_or(serde_json::json!({}));

                // First, try the router (covers remote tools)
                match router.call_tool(&tool_name, tool_args.clone()).await {
                    Ok(result) => {
                        return serde_json::to_value(result)
                            .map_err(|e| McpError::internal_error(format!("Serialize: {e}")));
                    }
                    Err(e) if e.contains("not found") || e.contains("Native tool") => {
                        // Tool not found in router — try native pipeline
                    }
                    Err(e) => {
                        return Err(McpError::tool_execution_error(&tool_name, e));
                    }
                }

                // Fall back to native tool-harness pipeline
                let input = tool_harness::ToolInput {
                    tool: tool_name.clone(),
                    args: tool_args,
                };
                let ctx = tool_harness::ToolUseContext::new("mcp-server");

                match pipeline.execute(&tool_name, input, &ctx).await {
                    Ok((tool_result, _budget)) => {
                        if tool_result.success {
                            Ok(serde_json::to_value(CallToolResult::success(tool_result.output))
                                .map_err(|e| McpError::internal_error(format!("Serialize: {e}")))?)
                        } else {
                            let msg = tool_result.error.unwrap_or_else(|| "Unknown error".into());
                            Err(McpError::tool_execution_error(&tool_name, msg))
                        }
                    }
                    Err(e) => {
                        if matches!(e.kind, tool_harness::ToolErrorKind::NotFound) {
                            Err(McpError::tool_not_found(&tool_name))
                        } else {
                            Err(McpError::tool_execution_error(&tool_name, e.message))
                        }
                    }
                }
            }
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

        // Look up the handler — try async handlers first, then sync.
        // IMPORTANT: `std::sync::RwLockReadGuard` is NOT `Send` — we must
        // clone the `Arc` and drop the guard BEFORE any `.await` point so
        // the future remains `Send` (required by axum).
        let response = {
            let async_handler = {
                let async_handlers = self.async_handlers.read().unwrap();
                async_handlers.get(&method).cloned()
            }; // guard dropped here

            if let Some(handler) = async_handler {
                match handler(params).await {
                    Ok(result) => JsonRpcResponse::success(request_id, result),
                    Err(err) => err.to_json_rpc_response(request_id),
                }
            } else {
                let sync_handlers = self.sync_handlers.read().unwrap();
                match sync_handlers.get(&method) {
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
    async fn test_tools_list_with_registry() {
        use tool_harness::tools::default_tool_registry;

        let registry = default_tool_registry();
        let server = McpServer::new().with_tool_registry(registry);

        let response = server
            .handle_request(r#"{"jsonrpc":"2.0","id":"1","method":"tools/list"}"#)
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        let tools = parsed["result"]["tools"].as_array().unwrap();
        assert!(!tools.is_empty(), "Should return at least one tool");
        assert!(
            tools.iter().any(|t| t["name"] == "read"),
            "Should include the 'read' tool"
        );
    }

    #[tokio::test]
    async fn test_tools_call_with_registry() {
        use tool_harness::tools::default_tool_registry;

        let registry = default_tool_registry();
        let server = McpServer::new().with_tool_registry(registry);

        // Try calling the 'ping' tool (no side effects)
        let response = server
            .handle_request(
                r#"{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"glob","arguments":{"pattern":"*.toml"}}}"#,
            )
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        // Should succeed or fail gracefully (depends on CWD)
        assert!(
            parsed["error"].is_null() || parsed["error"]["code"] == -32002,
            "Should succeed or report execution error, got: {:?}",
            parsed
        );
    }

    #[tokio::test]
    async fn test_tools_call_unknown_tool() {
        use tool_harness::tools::default_tool_registry;

        let registry = default_tool_registry();
        let server = McpServer::new().with_tool_registry(registry);

        let response = server
            .handle_request(
                r#"{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"nonexistent_tool","arguments":{}}}"#,
            )
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(parsed["error"]["code"], MCP_TOOL_NOT_FOUND,
            "Unknown tool should return tool_not_found");
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