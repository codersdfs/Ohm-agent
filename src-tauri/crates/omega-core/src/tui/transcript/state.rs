//! Tool call + scroll state (P5 split).

use super::preview::{EditCodePreview, WriteCodePreview, extract_edit_preview, extract_write_preview};
use super::shell::{fit_to_width, shorten};
use super::toolbox::compute_tool_summary;

// ─── ToolCallState ───────────────────────────────────────────────────────────

/// Tool call execution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCallStatus {
    Pending,
    Running,
    Completed,
    Errored,
}

/// State for a boxed tool call entry.
#[derive(Clone)]
pub struct ToolCallState {
    pub tool_name: String,
    /// Bounded arguments preview. Full write payloads are intentionally not retained.
    pub args: String,
    /// Parsed key-value lines for display (computed from bounded args)
    pub args_kv: Vec<(String, String)>,
    /// Bounded code preview for write/create calls.
    pub write_preview: Option<WriteCodePreview>,
    /// Bounded before/after preview for edit/patch calls.
    pub edit_preview: Option<EditCodePreview>,
    /// One-line summary for box header
    pub tool_summary: String,
    /// Full result text, if available
    pub result: Option<String>,
    /// Preview snippet of the result (first N chars)
    pub result_preview: Option<String>,
    /// Whether arguments are expanded (Ctrl+E to toggle)
    pub expanded: bool,
    /// Execution status
    pub status: ToolCallStatus,
    /// Duration string like "12ms"
    pub duration: Option<String>,
    /// Optional typed error — present only when status == Errored
    pub error: Option<crate::error::ToolCallError>,
}

impl ToolCallState {
    pub fn new(tool_name: String, args: String) -> Self {
        let write_preview = extract_write_preview(&tool_name, &args);
        let edit_preview = extract_edit_preview(&tool_name, &args);
        let tool_summary = compute_tool_summary(&tool_name, &args);
        let bounded_args = if write_preview.is_some() || edit_preview.is_some() {
            // Structured previews contain everything the renderer needs. Avoid
            // retaining arbitrarily large source-changing payloads.
            String::new()
        } else {
            fit_to_width(&args, 512)
        };
        let args_kv = parse_args_kv(&bounded_args);
        Self {
            tool_name,
            args: bounded_args,
            args_kv,
            write_preview,
            edit_preview,
            tool_summary,
            result: None,
            result_preview: None,
            expanded: false, // compact-by-default
            status: ToolCallStatus::Running,
            duration: None,
            error: None,
        }
    }

    /// Compute the title string for the bar.
    pub fn title(&self) -> String {
        let icon = match self.status {
            ToolCallStatus::Pending => "⋯",
            ToolCallStatus::Running => "▶",
            ToolCallStatus::Completed => "✓",
            ToolCallStatus::Errored => self.error.as_ref().map(|e| e.kind.icon()).unwrap_or("✗"),
        };
        let dur = self.duration.as_deref().unwrap_or("");
        if self.expanded {
            format!(" {} {} {} ", icon, self.tool_name, dur)
        } else {
            let kv_count = self.args_kv.len();
            let dur_suffix = if !dur.is_empty() {
                format!(" {}", dur)
            } else {
                String::new()
            };
            if self.result_preview.is_some() {
                format!(
                    " {} {} ({} args){}",
                    icon, self.tool_name, kv_count, dur_suffix
                )
            } else {
                format!(
                    " {} {} ({} args){}",
                    icon, self.tool_name, kv_count, dur_suffix
                )
            }
        }
    }
}

/// Parse a JSON arguments string into key-value pairs for clean display.
pub fn parse_args_kv(args: &str) -> Vec<(String, String)> {
    if args.trim().is_empty() {
        return Vec::new();
    }
    // Try to parse as JSON object
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(args) {
        if let Some(obj) = val.as_object() {
            let mut pairs: Vec<(String, String)> = Vec::new();
            for (k, v) in obj {
                let v_str = match v {
                    serde_json::Value::String(s) => {
                        // Char-safe truncation: byte-slicing here would panic if
                        // the cut point lands inside a multibyte codepoint.
                        // `shorten` already appends the ellipsis when it truncates.
                        if s.chars().count() > 80 {
                            format!("\"{}\"", shorten(s, 79))
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    other => other.to_string(),
                };
                pairs.push((k.clone(), v_str));
            }
            return pairs;
        }
    }
    // Fallback: show raw args as a single entry
    // `shorten` appends the ellipsis itself when it truncates.
    let preview = if args.chars().count() > 100 {
        shorten(args, 100)
    } else {
        args.to_string()
    };
    vec![("args".to_string(), preview)]
}

// ─── Scroll State ────────────────────────────────────────────────────────────

/// Scroll state for the transcript.
pub struct ScrollState {
    pub offset: usize,     // Scroll offset in lines from top
    pub auto_scroll: bool, // Whether to follow new content
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            offset: 0,
            auto_scroll: true,
        }
    }
}

/// Check if user message content contains attachment patterns:
/// URLs (http:// or https://), absolute file paths, or registered skill references.
pub fn has_attachment_content(content: &str) -> bool {
    // URL detection: strict protocol prefix
    if content.contains("http://") || content.contains("https://") {
        return true;
    }

    // File path detection: absolute paths or common file extensions
    for word in content.split_whitespace() {
        let trimmed = word.trim_end_matches(|c: char| matches!(c, ',' | '.' | ';' | ')' | ']' | '"'));
        // Unix absolute path: starts with / followed by more chars
        if trimmed.starts_with('/') && trimmed.len() > 1 {
            return true;
        }
        // Windows absolute path: C:\ or \ (UNC)
        if trimmed.len() >= 3 && trimmed.chars().nth(1) == Some(':') && trimmed.chars().nth(2) == Some('\\') {
            return true;
        }
        if trimmed.starts_with(r"\\") {
            return true;
        }
        // Common file extensions (without path prefix)
        if let Some(ext) = trimmed.rsplit('.').next() {
            let ext_lower = ext.to_lowercase();
            if matches!(ext_lower.as_str(),
                "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" |
                "toml" | "json" | "yaml" | "yml" | "md" | "txt" | "csv" |
                "sh" | "bash" | "zsh" | "fish" | "env" | "cfg" | "ini" |
                "html" | "css" | "scss" | "sass" | "less" | "xml" | "sql" |
                "lock" | "log" | "pdf" | "png" | "jpg" | "jpeg" | "gif" | "svg" |
                "mp4" | "mp3" | "wav" | "mov" | "avi" | "webm" | "mkv" |
                "zip" | "tar" | "gz" | "bz2" | "7z" | "rar"
            ) {
                // Only count as file if there's a dot before the extension
                if trimmed.contains('.') {
                    return true;
                }
            }
        }
    }

    // Skill reference detection: @skillname or bare word matching registered skills
    let skills = crate::commands::mcp::loaded_skills();
    for skill in &skills {
        let name = &skill.name;
        // @mention syntax
        if content.contains(&format!("@{}", name)) {
            return true;
        }
        // Bare word match (word boundary check)
        if content.split_whitespace().any(|w| w == name) {
            return true;
        }
    }

    false
}
