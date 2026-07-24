//! Tool router — merges native and remote tools, routes tool calls
//!
//! Maintains a unified view of all available tools from:
//! - Native tool-harness tools
//! - Remote MCP server tools
//! Routes `tools/call` to the correct backend.

use crate::bridge::remote_client::{RemoteMcpClient, RemoteTool};
use crate::types::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A backend that can execute a tool
#[derive(Debug, Clone)]
pub enum ToolBackend {
    /// Tool is from the native tool-harness
    Native,
    /// Tool is from a remote MCP server, identified by client pointer key
    Remote(String),
}

/// Index of tool name → backend mapping
#[derive(Default)]
pub struct ToolIndex {
    /// Maps tool name to backend info
    tools: HashMap<String, ToolEntry>,
    /// Tool definitions for MCP tools/list response
    definitions: Vec<McpToolDefinition>,
}

#[derive(Clone)]
pub struct ToolEntry {
    pub backend: ToolBackend,
    pub remote_name: String, // empty for native tools
    pub definition: McpToolDefinition,
}

impl ToolIndex {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            definitions: Vec::new(),
        }
    }
}

/// Router for tool calls — dispatches to the right backend
pub struct ToolRouter {
    index: RwLock<ToolIndex>,
    remote_clients: Arc<RwLock<HashMap<String, Arc<RemoteMcpClient>>>>,
}

impl ToolRouter {
    /// Create a new empty tool router
    pub fn new() -> Self {
        Self {
            index: RwLock::new(ToolIndex::new()),
            remote_clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a remote MCP server client
    pub async fn register_remote(&self, name: &str, client: Arc<RemoteMcpClient>) {
        let mut clients = self.remote_clients.write().unwrap();
        clients.insert(name.to_string(), client);
    }

    /// Register native tools from the tool-harness (synchronous version for builder)
    pub fn register_native_tools_blocking(&self, definitions: Vec<McpToolDefinition>) {
        let mut index = self.index.write().unwrap();
        for def in &definitions {
            index.tools.insert(
                def.name.clone(),
                ToolEntry {
                    backend: ToolBackend::Native,
                    remote_name: String::new(),
                    definition: def.clone(),
                },
            );
        }
        index.definitions = definitions;
    }

    /// Register native tools from the tool-harness
    pub async fn register_native_tools(&self, definitions: Vec<McpToolDefinition>) {
        let mut index = self.index.write().unwrap();

        for def in &definitions {
            index.tools.insert(
                def.name.clone(),
                ToolEntry {
                    backend: ToolBackend::Native,
                    remote_name: String::new(),
                    definition: def.clone(),
                },
            );
        }

        // Update definitions list
        index.definitions = definitions;
    }

    /// Discover and register tools from all remote servers
    pub async fn discover_remote_tools(&self) -> Result<(), String> {
        // Clone clients out of the lock so we don't hold a guard across .await
        let clients: Vec<Arc<RemoteMcpClient>> = {
            let guard = self.remote_clients.read().unwrap();
            guard.values().cloned().collect()
        };

        let mut all_remote_tools: Vec<RemoteTool> = Vec::new();
        for client in &clients {
            match client.discover_tools().await {
                Ok(tools) => {
                    all_remote_tools.extend(tools);
                }
                Err(e) => {
                    log::warn!("Failed to discover tools from '{}': {e}", client.config().name);
                }
            }
        }

        // Add remote tools to the index
        let mut index = self.index.write().unwrap();
        for tool in &all_remote_tools {
            index.tools.insert(
                tool.definition.name.clone(),
                ToolEntry {
                    backend: ToolBackend::Remote(tool.server_name.clone()),
                    remote_name: tool.remote_name.clone(),
                    definition: tool.definition.clone(),
                },
            );
        }

        // Rebuild definitions list (native + remote)
        index.definitions = index.tools.values().map(|e| e.definition.clone()).collect();

        log::info!(
            "Tool router: {} native + remote tools registered",
            index.definitions.len()
        );

        Ok(())
    }

    /// Get all tool definitions for tools/list
    pub async fn list_tools(&self) -> Vec<McpToolDefinition> {
        self.index.read().unwrap().definitions.clone()
    }

    /// Call a tool by name. Routes to the correct backend.
    pub async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<CallToolResult, String> {
        let entry = {
            let index = self.index.read().unwrap();
            index.tools.get(name).cloned()
        };

        match entry {
            Some(entry) => match entry.backend {
                ToolBackend::Native => {
                    // Native tools can't be called through the router directly
                    // They need the ExecutionPipeline which requires full tool-harness
                    Err(format!("Native tool '{}' cannot be called via bridge proxy — use server's native pipeline", name))
                }
                ToolBackend::Remote(server_name) => {
                    // Clone the client out of the lock to avoid holding guard across .await
                    let client = {
                        let guard = self.remote_clients.read().unwrap();
                        guard.get(&server_name).cloned()
                    };
                    match client {
                        Some(client) => {
                            client.ensure_connected().await?;
                            client.call_tool(&entry.remote_name, arguments).await
                        }
                        None => Err(format!("Remote server '{}' not found", server_name)),
                    }
                }
            },
            None => Err(format!("Tool '{}' not found", name)),
        }
    }

    /// Refresh tools from all registered remote servers
    pub async fn refresh_remote_tools(&self) -> Result<(), String> {
        // Clone clients out of the lock
        let clients: Vec<Arc<RemoteMcpClient>> = {
            let guard = self.remote_clients.read().unwrap();
            guard.values().cloned().collect()
        };

        for client in &clients {
            if let Err(e) = client.discover_tools().await {
                log::warn!("Failed to refresh tools from '{}': {e}", client.config().name);
            }
        }
        Ok(())
    }

    /// Get the number of registered tools
    pub async fn tool_count(&self) -> usize {
        self.index.read().unwrap().definitions.len()
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_router_returns_empty_tools() {
        let router = ToolRouter::new();
        let tools = router.list_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_register_native_tools() {
        let router = ToolRouter::new();
        let defs = vec![
            McpToolDefinition {
                name: "read".into(),
                description: "Read a file".into(),
                input_schema: serde_json::json!({"type":"object"}),
            },
        ];
        router.register_native_tools(defs).await;
        assert_eq!(router.list_tools().await.len(), 1);
    }

    #[tokio::test]
    async fn test_call_unknown_tool() {
        let router = ToolRouter::new();
        let result = router.call_tool("nonexistent", serde_json::json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let router = ToolRouter::new();
        let native = vec![
            McpToolDefinition {
                name: "read".into(),
                description: "Read a file".into(),
                input_schema: serde_json::json!({"type":"object"}),
            },
            McpToolDefinition {
                name: "write".into(),
                description: "Write a file".into(),
                input_schema: serde_json::json!({"type":"object"}),
            },
        ];
        router.register_native_tools(native).await;
        let tools = router.list_tools().await;
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|t| t.name == "read"));
        assert!(tools.iter().any(|t| t.name == "write"));
    }
}