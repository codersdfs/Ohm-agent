//! Tool router — exposes native tool definitions for `tools/list`
//!
//! Routes `tools/call` to the native ExecutionPipeline (the `tools/call`
//! handler in `server.rs` falls through to the pipeline when this router
//! reports "not found"). The remote-MCP bridge was removed — see ticket 03
//! for the analysis that proved `RemoteMcpClient` had zero callers.

use crate::types::*;
use std::collections::HashMap;
use std::sync::RwLock;

/// Index of tool name → entry mapping
#[derive(Default)]
pub struct ToolIndex {
    /// Maps tool name to entry
    tools: HashMap<String, ToolEntry>,
    /// Tool definitions for MCP tools/list response
    definitions: Vec<McpToolDefinition>,
}

#[derive(Clone)]
pub struct ToolEntry {
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

/// Router for tool definitions — used by `tools/list`.
/// `tools/call` is handled by the `ExecutionPipeline` in `server.rs`.
pub struct ToolRouter {
    index: RwLock<ToolIndex>,
}

impl ToolRouter {
    /// Create a new empty tool router
    pub fn new() -> Self {
        Self {
            index: RwLock::new(ToolIndex::new()),
        }
    }

    /// Register native tools from the tool-harness (synchronous version for builder)
    pub fn register_native_tools_blocking(&self, definitions: Vec<McpToolDefinition>) {
        let mut index = self.index.write().unwrap();
        for def in &definitions {
            index.tools.insert(
                def.name.clone(),
                ToolEntry {
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
                    definition: def.clone(),
                },
            );
        }

        // Update definitions list
        index.definitions = definitions;
    }

    /// Get all tool definitions for tools/list
    pub async fn list_tools(&self) -> Vec<McpToolDefinition> {
        self.index.read().unwrap().definitions.clone()
    }

    /// Look up a tool by name. The router only stores definitions; actual
    /// execution is the ExecutionPipeline's job (see `server.rs`).
    pub fn lookup(&self, name: &str) -> Option<McpToolDefinition> {
        self.index.read().unwrap().tools.get(name).map(|e| e.definition.clone())
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
        let defs = vec![McpToolDefinition {
            name: "read".into(),
            description: "Read a file".into(),
            input_schema: serde_json::json!({"type":"object"}),
        }];
        router.register_native_tools(defs).await;
        assert_eq!(router.list_tools().await.len(), 1);
    }

    #[test]
    fn test_lookup_unknown_returns_none() {
        let router = ToolRouter::new();
        assert!(router.lookup("nonexistent").is_none());
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
