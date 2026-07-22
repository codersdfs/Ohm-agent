// Built-in tool implementations

mod bash;
mod edit;
mod glob;
mod grep;
mod read;
mod write;

use crate::ToolRegistry;

pub use bash::BashTool;
pub use edit::EditTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use read::ReadTool;
pub use write::WriteTool;

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
