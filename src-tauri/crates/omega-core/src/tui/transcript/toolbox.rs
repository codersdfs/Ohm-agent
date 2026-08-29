//! Simple tool-call box rendering + summaries (P5 split).

use ratatui::text::Text;

use super::shell::{render_tool_call_compact, shorten};
use super::state::{ToolCallState, ToolCallStatus};

// ─── Shared box-rendering helpers ────────────────────────────────────────────

/// Per-tool icon for tool call boxes: 🔧 for generic, 📖 for read, ✏️ for write/edit,
/// 💻 for bash, 🔍 for grep/glob.

/// Build one interior line of the box: `│ <content><pad>│`.

/// Push a centered `├─ Result ─┤` style divider into `lines`.

// ─── Simple tool call box (for legacy ToolCall variant) ──────────────────────

/// Render a compact box for simple tool call entries (not collapsible).
pub fn render_tool_call_box_simple(
    tool_name: &str,
    args: &str,
    result: &Option<String>,
    avail_width: u16,
) -> Text<'static> {
    let mut state = ToolCallState::new(tool_name.to_string(), args.to_string());
    state.expanded = false;
    if let Some(r) = result.as_ref().filter(|r| !r.trim().is_empty()) {
        state.status = if r.starts_with("ERROR") {
            ToolCallStatus::Errored
        } else {
            ToolCallStatus::Completed
        };
        state.result = Some(r.clone());
        state.result_preview = Some(r.clone());
        if state.status == ToolCallStatus::Errored {
            state.error = crate::error::AgentError::from_flat_string(r)
                .typed_tool_error()
                .or_else(|| {
                    Some(crate::error::ToolCallError::new(
                        tool_name.to_string(),
                        crate::error::ToolErrorKind::ExecutionFailed,
                        r.trim_start_matches("ERROR:").trim().to_string(),
                    ))
                });
        }
    } else if result.is_some() {
        state.status = ToolCallStatus::Completed;
    }
    render_tool_call_compact(&state, avail_width)
}
/// `ToolCallStatus` and the optional typed `ToolCallError`.
pub fn compute_tool_summary(tool_name: &str, args: &str) -> String {
    let parsed: Option<serde_json::Value> = serde_json::from_str(args).ok();
    let obj = parsed.as_ref().and_then(|v| v.as_object());
    match tool_name {
        "bash" | "shell" | "command" | "run" => {
            let cmd = obj
                .and_then(|o| o.get("command").or(o.get("cmd")).or(o.get("shell")))
                .and_then(|v| v.as_str())
                .unwrap_or(args.trim());
            format!("bash {}", shorten(cmd, 40))
        }
        "write" | "create" => {
            let p = obj
                .and_then(|o| o.get("filePath").or(o.get("path")).or(o.get("file")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let content = obj
                .and_then(|o| o.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let first_line = content.lines().next().unwrap_or("");
            if first_line.is_empty() {
                format!("write {}", p)
            } else {
                format!("write {} | {}", p, shorten(first_line, 30))
            }
        }
        "edit" | "patch" | "str_replace" => {
            let p = obj
                .and_then(|o| o.get("filePath").or(o.get("path")).or(o.get("file")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let old_lines = obj
                .and_then(|o| o.get("oldString").or(o.get("oldText")).or(o.get("find")))
                .and_then(|v| v.as_str())
                .map(|s| s.lines().count())
                .unwrap_or(0);
            let new_lines = obj
                .and_then(|o| o.get("newString").or(o.get("newText")).or(o.get("replace")))
                .and_then(|v| v.as_str())
                .map(|s| s.lines().count())
                .unwrap_or(0);
            // Never put edited source into the summary. Only path and counts.
            format!("edit {} · -{} / +{} lines", p, old_lines, new_lines)
        }
        "read" | "view" | "cat" => {
            let p = obj
                .and_then(|o| o.get("filePath").or(o.get("path")).or(o.get("file")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("read {}", p)
        }
        "glob" | "ls" | "list" => {
            let pat = obj
                .and_then(|o| o.get("pattern").or(o.get("glob")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("glob {}", pat)
        }
        "grep" | "search" | "rg" => {
            let q = obj
                .and_then(|o| o.get("pattern").or(o.get("query")).or(o.get("search")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("grep {}", q)
        }
        "web" | "fetch" | "browse" => {
            let u = obj
                .and_then(|o| o.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("fetch {}", u)
        }
        _ => format!("{} {}", tool_name, shorten(args.trim(), 30)),
    }
}

pub const COLLAPSED_SOURCE_LINES: usize = 10;
pub const MAX_RETAINED_SOURCE_LINES: usize = 100;
pub const MAX_SOURCE_COLUMNS: usize = 240;
