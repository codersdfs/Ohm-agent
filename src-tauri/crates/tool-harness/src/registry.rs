// Tool registry for managing built-in and MCP tools

use crate::metadata::{ToolCategory, ToolMetadata, ToolRef};
use crate::Tool;
use providers::ToolDefinition;
use std::collections::HashMap;

/// Tool registry for registration, lookup, and listing
pub struct ToolRegistry {
    built_ins: HashMap<String, Box<dyn Tool>>,
    mcp_tools: HashMap<String, Box<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            built_ins: HashMap::new(),
            mcp_tools: HashMap::new(),
        }
    }

    /// Register a built-in tool
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.built_ins.insert(tool.name().to_string(), tool);
    }

    /// Unregister a tool by name
    pub fn unregister(&mut self, name: &str) -> Option<Box<dyn Tool>> {
        self.built_ins.remove(name)
    }

    /// Get a tool by name (built-ins take precedence)
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.built_ins
            .get(name)
            .map(|t| t.as_ref())
            .or_else(|| self.mcp_tools.get(name).map(|t| t.as_ref()))
    }

    /// List all tool names (built-ins first, then MCP)
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .built_ins
            .keys()
            .chain(self.mcp_tools.keys())
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// Get tool definitions for all registered tools
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = self
            .built_ins
            .values()
            .chain(self.mcp_tools.values())
            .map(|t| ToolDefinition {
                tool_type: "function".into(),
                function: providers::ToolFunctionDef {
                    name: t.name().into(),
                    description: t.description().into(),
                    parameters: t.parameters_schema(),
                },
            })
            .collect();
        // Sort by name for consistent ordering
        defs.sort_by(|a, b| a.function.name.cmp(&b.function.name));
        defs
    }

    // ── Metadata-aware extensions ────────────────────────────────

    /// Get metadata for a single tool by name.
    pub fn get_metadata(&self, name: &str) -> Option<ToolMetadata> {
        self.get(name).map(|t| t.metadata())
    }

    /// Return metadata for every registered tool.
    pub fn all_metadata(&self) -> Vec<ToolMetadata> {
        let mut meta: Vec<ToolMetadata> = self
            .built_ins
            .values()
            .chain(self.mcp_tools.values())
            .map(|t| t.metadata())
            .collect();
        meta.sort_by(|a, b| a.name.cmp(&b.name));
        meta
    }

    /// Return lightweight references for all tools (cheaper than full metadata).
    pub fn all_refs(&self) -> Vec<ToolRef> {
        self.all_metadata().iter().map(ToolRef::from).collect()
    }

    /// List tools grouped by category.
    /// Returns a BTreeMap so categories are in a consistent order.
    pub fn list_by_category(&self) -> std::collections::BTreeMap<ToolCategory, Vec<ToolRef>> {
        let mut map: std::collections::BTreeMap<ToolCategory, Vec<ToolRef>> =
            std::collections::BTreeMap::new();
        for r in self.all_refs() {
            map.entry(r.category).or_default().push(r);
        }
        map
    }

    /// Search tools by name, description, or tags.
    /// Simple substring matching — Phase 2 will add fuzzy/ranked search.
    pub fn search(&self, query: &str) -> Vec<ToolRef> {
        let q = query.to_lowercase();
        let mut results: Vec<(ToolRef, u32)> = self
            .all_refs()
            .into_iter()
            .filter_map(|r| {
                let mut score = 0u32;
                let name_lower = r.name.to_lowercase();
                let desc_lower = r.description.to_lowercase();

                // Exact name match tops the list
                if name_lower == q {
                    score += 100;
                }
                // Name starts with query
                if name_lower.starts_with(&q) {
                    score += 50;
                }
                // Name contains query
                if name_lower.contains(&q) {
                    score += 30;
                }
                // Description contains query
                if desc_lower.contains(&q) {
                    score += 10;
                }
                // Tags contain query
                for tag in &r.tags {
                    if tag.to_lowercase().contains(&q) {
                        score += 15;
                        break;
                    }
                }

                if score > 0 {
                    Some((r, score))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.into_iter().map(|(r, _)| r).collect()
    }

    /// Count tools by source type.
    pub fn count_by_source(&self) -> (usize, usize, usize) {
        let builtin = self.built_ins.len();
        let mcp = self.mcp_tools.len();
        (builtin, mcp, builtin + mcp)
    }

    // ── End metadata extensions ──────────────────────────────────

    /// Merge MCP tools into registry
    pub fn merge_mcp_tools(&mut self, tools: Vec<Box<dyn Tool>>) {
        for tool in tools {
            self.mcp_tools.insert(tool.name().to_string(), tool);
        }
    }

    /// Get a mutable reference to the MCP tools map for modification
    pub fn mcp_tools_mut(&mut self) -> &mut HashMap<String, Box<dyn Tool>> {
        &mut self.mcp_tools
    }

    /// Feature flag filtering (stub - accept all for now)
    pub fn filter_by_feature(&self, _feature_flags: &[&str]) -> Vec<String> {
        self.list()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToolError, ToolInput, ToolResult, ToolUseContext};
    use async_trait::async_trait;

    struct MockTool;

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            "mock_tool"
        }
        fn description(&self) -> &str {
            "A mock tool for testing"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(
            &self,
            _input: ToolInput,
            _ctx: &ToolUseContext,
        ) -> Result<ToolResult, ToolError> {
            Ok(ToolResult::success("mock output"))
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool));

        let tool = registry.get("mock_tool");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "mock_tool");
    }

    #[test]
    fn test_registry_list() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool));
        registry.register(Box::new(AnotherMockTool));

        let names = registry.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"mock_tool".to_string()));
        assert!(names.contains(&"another_mock".to_string()));
    }

    struct AnotherMockTool;

    #[async_trait]
    impl Tool for AnotherMockTool {
        fn name(&self) -> &str {
            "another_mock"
        }
        fn description(&self) -> &str {
            "Another mock tool"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(
            &self,
            _input: ToolInput,
            _ctx: &ToolUseContext,
        ) -> Result<ToolResult, ToolError> {
            Ok(ToolResult::success("another output"))
        }
    }
}
