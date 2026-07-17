// Tool Harness — Production-ready tool calling framework for Omega Agent

mod types;
mod traits;
mod context;
mod registry;
mod permission;
mod budget;
mod hooks;
mod schema;
mod metadata;
mod pipeline;
mod orchestrator;

pub mod tools;
pub mod box_; // Tool Calling Box modules (phased)

pub use types::{ToolError, ToolErrorKind, ToolResult, ToolInput, ToolRequest, ExecutionOutcome, PermissionResult, BudgetCheck, GateCheckResult, GateViolationInfo};
pub use traits::Tool;
pub use context::ToolUseContext;
pub use registry::ToolRegistry;
pub use permission::{PermissionMode, PermissionRule, PermissionResolver};
pub use budget::{ResultBudget, ConversationBudget};
pub use hooks::{PreToolUseHook, PostToolUseHook, HooksRegistry};
pub use pipeline::ExecutionPipeline;
pub use orchestrator::ToolOrchestrator;
pub use metadata::{ToolMetadata, ToolCategory, ToolRef, ParamSummary, LatencyHint, ToolErrorSpec, ToolExample, CostHint, CostCategory, ToolSource, DeprecationInfo, ParamConstraints};

// Re-export types from providers that tool-harness depends on
pub use providers::{ToolDefinition, ToolCall, ChatMessage, ChatRequest, ChatResponse, LlmProvider};