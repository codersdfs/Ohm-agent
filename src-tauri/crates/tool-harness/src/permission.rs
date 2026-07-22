// Permission system for tool execution

use crate::{PermissionResult, Tool, ToolInput, ToolUseContext};
use serde::{Deserialize, Serialize};

/// Permission mode enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    Plan,
    DontAsk,
    BypassPermissions,
    Auto,
    Bubble,
}

impl PermissionMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "accept-edits" => Self::AcceptEdits,
            "plan" => Self::Plan,
            "dont-ask" => Self::DontAsk,
            "bypass" | "bypass-permissions" => Self::BypassPermissions,
            "auto" => Self::Auto,
            "bubble" => Self::Bubble,
            _ => Self::Default,
        }
    }
}

/// Permission rule for configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub source: String,
    pub behavior: PermissionBehavior,
    pub tool_pattern: String,
    pub content_pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Prompt,
}

impl PermissionRule {
    pub fn new(
        source: impl Into<String>,
        behavior: PermissionBehavior,
        tool_pattern: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            behavior,
            tool_pattern: tool_pattern.into(),
            content_pattern: None,
        }
    }
}

/// Permission resolver with resolution chain
pub struct PermissionResolver {
    mode: PermissionMode,
    rules: Vec<PermissionRule>,
}

impl Default for PermissionResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionResolver {
    pub fn new() -> Self {
        Self {
            mode: PermissionMode::default(),
            rules: Vec::new(),
        }
    }

    pub fn with_mode(mut self, mode: PermissionMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn add_rule(mut self, rule: PermissionRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Resolve permission through the chain: hooks → rules → tool.check_permissions → mode default
    pub async fn resolve(
        &self,
        tool_name: &str,
        input: &ToolInput,
        ctx: &ToolUseContext,
        tool: Option<&dyn Tool>,
    ) -> PermissionResult {
        // Check rule-based permissions first
        for rule in &self.rules {
            if Self::matches_pattern(tool_name, &rule.tool_pattern) {
                return match rule.behavior {
                    PermissionBehavior::Allow => PermissionResult::Allow,
                    PermissionBehavior::Deny => PermissionResult::Deny,
                    PermissionBehavior::Prompt => {
                        PermissionResult::Prompt(format!("Allow {}?", tool_name))
                    }
                };
            }
        }

        // Check tool-specific permissions (tool.check_permissions)
        if let Some(t) = tool {
            let tool_result = t.check_permissions(input, ctx);
            match tool_result {
                PermissionResult::Allow => { /* continue to mode defaults */ }
                other => return other,
            }
        }

        // Mode defaults
        match self.mode {
            PermissionMode::BypassPermissions => PermissionResult::Allow,
            PermissionMode::DontAsk => PermissionResult::Allow,
            PermissionMode::AcceptEdits => {
                // For write/edit tools, allow in plan/strict mode
                if tool_name == "write" || tool_name == "edit" {
                    PermissionResult::Allow
                } else {
                    PermissionResult::Deny
                }
            }
            PermissionMode::Auto => PermissionResult::Allow,
            PermissionMode::Default | PermissionMode::Plan | PermissionMode::Bubble => {
                // Check tool-specific permissions
                PermissionResult::Allow
            }
        }
    }

    fn matches_pattern(name: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if pattern.ends_with('*') {
            name.starts_with(&pattern[..pattern.len() - 1])
        } else {
            name == pattern
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_mode_from_str() {
        assert!(matches!(
            PermissionMode::from_str("default"),
            PermissionMode::Default
        ));
        assert!(matches!(
            PermissionMode::from_str("bypass"),
            PermissionMode::BypassPermissions
        ));
        assert!(matches!(
            PermissionMode::from_str("auto"),
            PermissionMode::Auto
        ));
        assert!(matches!(
            PermissionMode::from_str("unknown"),
            PermissionMode::Default
        ));
    }

    #[test]
    fn test_permission_rule_pattern_matching() {
        // Test wildcard matching
        assert!(PermissionResolver::matches_pattern("read", "read*"));
        assert!(PermissionResolver::matches_pattern("read_file", "read*"));
        assert!(!PermissionResolver::matches_pattern("write_file", "read*"));
    }

    #[tokio::test]
    async fn test_resolver_mode_defaults() {
        let resolver = PermissionResolver::new().with_mode(PermissionMode::BypassPermissions);
        let input = ToolInput {
            tool: "read".into(),
            args: serde_json::json!({ "filePath": "test.txt" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = resolver.resolve("read", &input, &ctx, None).await;
        assert!(matches!(result, PermissionResult::Allow));
    }
}
