// Hook system for pre/post tool execution

use async_trait::async_trait;
use crate::{ToolInput, ToolResult};

/// Hook trait for pre-tool-use callbacks
#[async_trait]
pub trait PreToolUseHook: Send + Sync {
    async fn before(&self, _tool_name: &str, _input: &ToolInput) -> Result<(), String> {
        Ok(())
    }
}

/// Hook trait for post-tool-use callbacks
#[async_trait]
pub trait PostToolUseHook: Send + Sync {
    async fn after(&self, _tool_name: &str, _result: &ToolResult) -> Result<(), String> {
        Ok(())
    }
}

/// Registry for managing hooks
pub struct HooksRegistry {
    pre_hooks: Vec<Box<dyn PreToolUseHook>>,
    post_hooks: Vec<Box<dyn PostToolUseHook>>,
}

impl Default for HooksRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HooksRegistry {
    pub fn new() -> Self {
        Self {
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
        }
    }

    /// Register a pre-tool hook
    pub fn register_pre(&mut self, hook: Box<dyn PreToolUseHook>) {
        self.pre_hooks.push(hook);
    }

    /// Register a post-tool hook
    pub fn register_post(&mut self, hook: Box<dyn PostToolUseHook>) {
        self.post_hooks.push(hook);
    }

    /// Deregister a hook by name (using index for simplicity)
    pub fn unregister_pre(&mut self, index: usize) -> Option<Box<dyn PreToolUseHook>> {
        if index < self.pre_hooks.len() {
            Some(self.pre_hooks.remove(index))
        } else {
            None
        }
    }

    pub fn unregister_post(&mut self, index: usize) -> Option<Box<dyn PostToolUseHook>> {
        if index < self.post_hooks.len() {
            Some(self.post_hooks.remove(index))
        } else {
            None
        }
    }

    /// Run all pre-tool hooks
    pub async fn run_pre_hooks(&self, tool_name: &str, input: &ToolInput) {
        for hook in &self.pre_hooks {
            if let Err(e) = hook.before(tool_name, input).await {
                log::warn!("Pre-tool hook error: {}", e);
            }
        }
    }

    /// Run all post-tool hooks
    pub async fn run_post_hooks(&self, tool_name: &str, result: &ToolResult) {
        for hook in &self.post_hooks {
            if let Err(e) = hook.after(tool_name, result).await {
                log::warn!("Post-tool hook error: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingPreHook {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl PreToolUseHook for CountingPreHook {
        async fn before(&self, _tool_name: &str, _input: &ToolInput) -> Result<(), String> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_hooks_registry_runs_pre_hooks() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut registry = HooksRegistry::new();
        registry.register_pre(Box::new(CountingPreHook { count: count.clone() }));
        registry.register_pre(Box::new(CountingPreHook { count: count.clone() }));

        let input = ToolInput {
            tool: "test".into(),
            args: serde_json::json!({}),
        };

        registry.run_pre_hooks("test", &input).await;
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_hooks_registry_handles_errors() {
        struct FailingHook;

        #[async_trait]
        impl PreToolUseHook for FailingHook {
            async fn before(&self, _tool_name: &str, _input: &ToolInput) -> Result<(), String> {
                Err("hook failed".into())
            }
        }

        let mut registry = HooksRegistry::new();
        registry.register_pre(Box::new(FailingHook));

        let input = ToolInput {
            tool: "test".into(),
            args: serde_json::json!({}),
        };

        // Should not panic
        registry.run_pre_hooks("test", &input).await;
    }
}