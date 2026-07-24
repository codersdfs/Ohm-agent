// Built-in tool implementations

mod apply_patch;
mod ask_user;
mod bash;
mod edit;
mod git_commit;
mod git_diff;
mod git_log;
mod git_status;
mod glob;
mod grep;
mod read;
mod todo;
mod web_fetch;
mod write;

use crate::ToolRegistry;

pub use apply_patch::ApplyPatchTool;
pub use ask_user::AskUserTool;
pub use bash::BashTool;
pub use edit::EditTool;
pub use git_commit::GitCommitTool;
pub use git_diff::GitDiffTool;
pub use git_log::GitLogTool;
pub use git_status::GitStatusTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use read::ReadTool;
pub use todo::{TodoItem, TodoStatus, TodoTool, TodoStore, new_todo_store};
pub use web_fetch::WebFetchTool;
pub use write::WriteTool;

/// Create default tool registry with all built-in tools
pub fn default_tool_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ReadTool));
    registry.register(Box::new(WriteTool));
    registry.register(Box::new(EditTool));
    registry.register(Box::new(ApplyPatchTool));
    registry.register(Box::new(BashTool));
    registry.register(Box::new(GrepTool));
    registry.register(Box::new(GlobTool));
    registry.register(Box::new(GitStatusTool));
    registry.register(Box::new(GitDiffTool));
    registry.register(Box::new(GitLogTool));
    registry.register(Box::new(GitCommitTool));
    registry.register(Box::new(WebFetchTool));
    registry.register(Box::new(TodoTool::new(new_todo_store())));
    registry.register(Box::new(AskUserTool));
    registry
}
