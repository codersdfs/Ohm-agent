// Built-in tool implementations

mod read;
mod write;
mod edit;
mod bash;
mod grep;
mod glob;

use crate::ToolRegistry;

pub use read::ReadTool;
pub use write::WriteTool;
pub use edit::EditTool;
pub use bash::BashTool;
pub use grep::GrepTool;
pub use glob::GlobTool;

/// Create default tool registry with all built-in tools
pub fn default_tool_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ReadTool));
    registry.register(Box::new(WriteTool));
    registry.register(Box::new(EditTool));
    registry.register(Box::new(BashTool));
    registry.register(Box::new(GrepTool));
    registry.register(Box::new(GlobTool));
    registry
}