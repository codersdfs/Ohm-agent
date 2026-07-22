// Tool Harness — Production-ready tool calling framework for Omega Agent

mod budget;
mod context;
mod hooks;
mod metadata;
mod orchestrator;
mod permission;
mod pipeline;
mod registry;
mod schema;
mod traits;
mod types;

pub mod box_;
pub mod tools; // Tool Calling Box modules (phased)

pub use budget::{ConversationBudget, ResultBudget};
pub use context::ToolUseContext;
pub use hooks::{HooksRegistry, PostToolUseHook, PreToolUseHook};
pub use metadata::{
    CostCategory, CostHint, DeprecationInfo, LatencyHint, ParamConstraints, ParamSummary,
    ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata, ToolRef, ToolSource,
};
pub use orchestrator::ToolOrchestrator;
pub use permission::{PermissionMode, PermissionResolver, PermissionRule};
pub use pipeline::ExecutionPipeline;
pub use registry::ToolRegistry;
pub use traits::Tool;
pub use types::{
    BudgetCheck, ExecutionOutcome, GateCheckResult, GateViolationInfo, PermissionResult, ToolError,
    ToolErrorKind, ToolInput, ToolRequest, ToolResult,
};

// Re-export types from providers that tool-harness depends on
pub use providers::{
    ChatMessage, ChatRequest, ChatResponse, LlmProvider, ToolCall, ToolDefinition,
};
