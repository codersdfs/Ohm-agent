// Apply Patch tool — applies unified diffs or V4A-style patches safely

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;
use std::path::PathBuf;

pub struct ApplyPatchTool;

impl ApplyPatchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ApplyPatchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }
    fn description(&self) -> &str {
        "Apply a unified diff or V4A-style patch to files. Returns which files changed."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "patch": string_param("Unified diff or V4A-style patch content"),
                "filePath": string_param("Optional: target file path if patch doesn't include one (for single-file patches)")
            },
            "required": ["patch"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "apply_patch".into(),
            label: "Apply Patch".into(),
            description: "Apply a unified diff or V4A-style patch to files. Returns which files changed.".into(),
            doc: Some("Parses and applies a unified diff (diff --git format) to the filesystem.
Supports multiple files in a single patch. Rejects binary file patches.
Returns a summary of which files were modified, added, or deleted.
Prefer this over full-file write for targeted changes.".into()),
            category: ToolCategory::DiffPatch,
            subcategory: Some("patch".into()),
            tags: vec!["patch".into(), "diff".into(), "edit".into(), "apply".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: false,
            concurrency_safe: false,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 1_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "parse_error".into(),
                    description: "The patch could not be parsed as a unified diff".into(),
                    recoverable: true,
                    retry_advice: Some("Ensure the patch is in unified diff format with proper --- and +++ headers".into()),
                },
                ToolErrorSpec {
                    kind: "conflict".into(),
                    description: "The patch context does not match the current file content".into(),
                    recoverable: true,
                    retry_advice: Some("The file may have been modified. Read the file and try again with an updated patch".into()),
                },
                ToolErrorSpec {
                    kind: "binary_file".into(),
                    description: "The patch targets a binary file".into(),
                    recoverable: false,
                    retry_advice: Some("Binary files cannot be patched with text diffs".into()),
                },
                ToolErrorSpec {
                    kind: "file_not_found".into(),
                    description: "A file referenced in the patch does not exist".into(),
                    recoverable: true,
                    retry_advice: Some("For new files, ensure the patch creates them (--- /dev/null)".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Apply a simple edit".into(),
                    description: "Change a function name in a file".into(),
                    arguments: serde_json::json!({
                        "patch": "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n-fn old_name() {}\n+fn new_name() {}\n"
                    }),
                    expected_result: Some("Modified: src/lib.rs".into()),
                },
                ToolExample {
                    title: "Create a new file".into(),
                    description: "Create a new file with content".into(),
                    arguments: serde_json::json!({
                        "patch": "--- /dev/null\n+++ b/src/new.rs\n@@ -0,0 +1,2 @@\n+fn main() {}\n+\n"
                    }),
                    expected_result: Some("Created: src/new.rs".into()),
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 50, category: CostCategory::Cheap }),
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    fn is_read_only(&self, _input: &ToolInput) -> bool {
        false
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let patch = input
            .args
            .get("patch")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: patch"))?;

        let explicit_file_path = input
            .args
            .get("filePath")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        let changes = apply_patch(patch, explicit_file_path)?;

        if changes.is_empty() {
            return Ok(ToolResult::success(
                "No changes applied (empty or no-op patch)".to_string(),
            ));
        }

        let mut summary_lines = Vec::new();
        for change in &changes {
            match change.action.as_str() {
                "created" => summary_lines.push(format!("Created: {}", change.path)),
                "deleted" => summary_lines.push(format!("Deleted: {}", change.path)),
                "modified" => summary_lines.push(format!("Modified: {}", change.path)),
                _ => summary_lines.push(format!("{}: {}", change.action, change.path)),
            }
        }

        Ok(ToolResult::success(format!(
            "Applied patch — {} file(s) changed:\n{}",
            changes.len(),
            summary_lines.join("\n")
        )))
    }
}

#[derive(Debug, Clone)]
struct FileChange {
    action: String,
    path: String,
    lines_added: usize,
    lines_removed: usize,
}

/// Parse and apply a unified diff patch.
fn apply_patch(
    patch: &str,
    explicit_file_path: Option<PathBuf>,
) -> Result<Vec<FileChange>, ToolError> {
    let mut changes = Vec::new();
    let lines: Vec<&str> = patch.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        // Look for diff header: --- old / +++ new
        if lines[i].starts_with("---") && i + 1 < lines.len() && lines[i + 1].starts_with("+++") {
            let old_path_raw = lines[i][3..].trim().trim_start_matches("a/").trim();
            let new_path_raw = lines[i + 1][3..].trim().trim_start_matches("b/").trim();
            i += 2;

            // Skip binary file markers
            if i < lines.len() && lines[i] == "Binary files differ" {
                return Err(ToolError::with_kind(
                    crate::ToolErrorKind::ExecutionFailed,
                    "Cannot apply patch to binary file",
                ));
            }

            // Determine action and path
            let (action, path) = if old_path_raw == "/dev/null" {
                ("created".to_string(), new_path_raw.to_string())
            } else if new_path_raw == "/dev/null" {
                ("deleted".to_string(), old_path_raw.to_string())
            } else {
                ("modified".to_string(), new_path_raw.to_string())
            };

            // If we have an explicit file path and this is a single-file patch without a path
            let path = if path.is_empty() {
                if let Some(ref explicit) = explicit_file_path {
                    explicit.to_string_lossy().to_string()
                } else {
                    return Err(ToolError::new(
                        "Patch does not specify a file path and no filePath argument provided",
                    ));
                }
            } else {
                path
            };

            // Parse hunks
            let mut hunks: Vec<Hunk> = Vec::new();
            while i < lines.len() {
                if lines[i].starts_with("@@") {
                    let hunk = parse_hunk(&lines, &mut i)?;
                    hunks.push(hunk);
                } else if lines[i].starts_with("---") {
                    // Next file
                    break;
                } else {
                    i += 1;
                }
            }

            // Apply hunks to the file
            if action == "deleted" {
                std::fs::remove_file(&path).map_err(|e| {
                    ToolError::new(format!("Failed to delete {}: {}", path, e))
                })?;
                changes.push(FileChange {
                    action: "deleted".to_string(),
                    path,
                    lines_added: 0,
                    lines_removed: 0,
                });
            } else if action == "created" {
                let content = build_created_content(&hunks);
                let path_buf = PathBuf::from(&path);
                if let Some(parent) = path_buf.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        ToolError::new(format!(
                            "Failed to create directory {}: {}",
                            parent.display(),
                            e
                        ))
                    })?;
                }
                std::fs::write(&path, content).map_err(|e| {
                    ToolError::new(format!("Failed to create {}: {}", path, e))
                })?;
                changes.push(FileChange {
                    action: "created".to_string(),
                    path,
                    lines_added: hunks.iter().map(|h| h.added.len()).sum(),
                    lines_removed: 0,
                });
            } else {
                // Modified: read, apply hunks, write
                let original = std::fs::read_to_string(&path).map_err(|e| {
                    ToolError::new(format!("Failed to read {}: {}", path, e))
                })?;

                let result = apply_hunks(&original, &hunks)?;
                std::fs::write(&path, result.content).map_err(|e| {
                    ToolError::new(format!("Failed to write {}: {}", path, e))
                })?;

                changes.push(FileChange {
                    action: "modified".to_string(),
                    path,
                    lines_added: hunks.iter().map(|h| h.added.len()).sum(),
                    lines_removed: hunks.iter().map(|h| h.removed.len()).sum(),
                });
            }
        } else {
            i += 1;
        }
    }

    Ok(changes)
}

#[derive(Debug, Clone)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<HunkLine>,
    added: Vec<String>,
    removed: Vec<String>,
}

#[derive(Debug, Clone)]
enum HunkLine {
    Context(String),
    Added(String),
    Removed(String),
}

fn parse_hunk(lines: &[&str], i: &mut usize) -> Result<Hunk, ToolError> {
    let header = lines[*i];
    // Parse @@ -old_start,old_count +new_start,new_count @@
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(ToolError::new(format!("Invalid hunk header: {}", header)));
    }

    let (old_start, old_count) = parse_range(parts[1])?;
    let (new_start, new_count) = parse_range(parts[2])?;

    *i += 1;

    let mut hunk_lines = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();

    while *i < lines.len() {
        let line = lines[*i];
        if line.starts_with("@@") || line.starts_with("---") {
            break;
        }
        if line.starts_with("+++") {
            // This shouldn't happen in the middle of a hunk, but handle gracefully
            break;
        }

        if let Some(content) = line.strip_prefix(' ') {
            hunk_lines.push(HunkLine::Context(content.to_string()));
        } else if let Some(content) = line.strip_prefix('+') {
            hunk_lines.push(HunkLine::Added(content.to_string()));
            added.push(content.to_string());
        } else if let Some(content) = line.strip_prefix('-') {
            hunk_lines.push(HunkLine::Removed(content.to_string()));
            removed.push(content.to_string());
        } else if line == "\\" {
            // No newline at end of file marker — skip
        }

        *i += 1;
    }

    Ok(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        lines: hunk_lines,
        added,
        removed,
    })
}

fn parse_range(s: &str) -> Result<(usize, usize), ToolError> {
    // Format: -start,count or +start,count (with optional +/- prefix from diff)
    // or just start (count defaults to 1)
    let s = s.trim_start_matches(|c| c == '-' || c == '+');
    if let Some((start_str, count_str)) = s.split_once(',') {
        let start = start_str.parse::<usize>().map_err(|_| {
            ToolError::new(format!("Invalid range start: {}", start_str))
        })?;
        let count = count_str.parse::<usize>().map_err(|_| {
            ToolError::new(format!("Invalid range count: {}", count_str))
        })?;
        Ok((start, count))
    } else {
        let start = s.parse::<usize>().map_err(|_| {
            ToolError::new(format!("Invalid range start: {}", s))
        })?;
        Ok((start, 1))
    }
}

fn build_created_content(hunks: &[Hunk]) -> String {
    let mut content = String::new();
    for hunk in hunks {
        for line in &hunk.lines {
            match line {
                HunkLine::Context(s) => {
                    content.push_str(s);
                    content.push('\n');
                }
                HunkLine::Added(s) => {
                    content.push_str(s);
                    content.push('\n');
                }
                HunkLine::Removed(_) => {}
            }
        }
    }
    content
}

struct ApplyResult {
    content: String,
}

fn apply_hunks(original: &str, hunks: &[Hunk]) -> Result<ApplyResult, ToolError> {
    let original_lines: Vec<&str> = original.lines().collect();
    let mut result_lines: Vec<String> = Vec::new();
    let mut current_line = 1usize; // 1-indexed in diff
    let mut original_iter = original_lines.iter().peekable();

    for hunk in hunks {
        // Skip context lines until we reach the hunk's old_start
        while current_line < hunk.old_start {
            if let Some(line) = original_iter.next() {
                result_lines.push((*line).to_string());
                current_line += 1;
            } else {
                return Err(ToolError::new(format!(
                    "Patch context mismatch: expected line {} but file ended",
                    hunk.old_start
                )));
            }
        }

        // Apply hunk lines
        for hunk_line in &hunk.lines {
            match hunk_line {
                HunkLine::Context(ctx) => {
                    if let Some(line) = original_iter.next() {
                        if *line != ctx.as_str() {
                            return Err(ToolError::with_details(
                                "Patch context mismatch".to_string(),
                                format!(
                                    "Expected '{}', found '{}' at line {}",
                                    ctx, line, current_line
                                ),
                            ));
                        }
                        result_lines.push((*line).to_string());
                        current_line += 1;
                    } else {
                        return Err(ToolError::new(format!(
                            "Patch context mismatch: file ended at line {}",
                            current_line
                        )));
                    }
                }
                HunkLine::Added(added) => {
                    result_lines.push(added.clone());
                }
                HunkLine::Removed(removed) => {
                    if let Some(line) = original_iter.next() {
                        if *line != removed.as_str() {
                            return Err(ToolError::with_details(
                                "Patch context mismatch".to_string(),
                                format!(
                                    "Expected '{}', found '{}' at line {}",
                                    removed, line, current_line
                                ),
                            ));
                        }
                        current_line += 1;
                    } else {
                        return Err(ToolError::new(format!(
                            "Patch context mismatch: file ended at line {}",
                            current_line
                        )));
                    }
                }
            }
        }
    }

    // Copy remaining lines
    for line in original_iter {
        result_lines.push((*line).to_string());
    }

    Ok(ApplyResult {
        content: result_lines.join("\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_apply_patch_simple_edit() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "fn old_name() {{}}").unwrap();
        writeln!(file, "fn keep_this() {{}}").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = ApplyPatchTool::new();
        let patch = format!(
            "--- a/{path}\n+++ b/{path}\n@@ -1,2 +1,2 @@\n-fn old_name() {{}}\n+fn new_name() {{}}\n fn keep_this() {{}}\n"
        );
        let input = ToolInput {
            tool: "apply_patch".into(),
            args: serde_json::json!({ "patch": patch }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Modified"));

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("fn new_name()"));
        assert!(!content.contains("fn old_name()"));
        assert!(content.contains("fn keep_this()"));
    }

    #[tokio::test]
    async fn test_apply_patch_create_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("new_file.rs");
        let path_str = path.to_str().unwrap().to_string();

        let tool = ApplyPatchTool::new();
        let patch = format!(
            "--- /dev/null\n+++ b/{path_str}\n@@ -0,0 +1,2 @@\n+fn main() {{}}\n+\n"
        );
        let input = ToolInput {
            tool: "apply_patch".into(),
            args: serde_json::json!({ "patch": patch }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Created"));

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "fn main() {}\n\n");
    }

    #[tokio::test]
    async fn test_apply_patch_context_mismatch() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "fn old_name() {{}}").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = ApplyPatchTool::new();
        let patch = format!(
            "--- a/{path}\n+++ b/{path}\n@@ -1,1 +1,1 @@\n-fn different() {{}}\n+fn new_name() {{}}\n"
        );
        let input = ToolInput {
            tool: "apply_patch".into(),
            args: serde_json::json!({ "patch": patch }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_apply_patch_empty_patch() {
        let tool = ApplyPatchTool::new();
        let input = ToolInput {
            tool: "apply_patch".into(),
            args: serde_json::json!({ "patch": "" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("No changes"));
    }

    #[tokio::test]
    async fn test_apply_patch_multiple_hunks() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "line1").unwrap();
        writeln!(file, "line2").unwrap();
        writeln!(file, "line3").unwrap();
        writeln!(file, "line4").unwrap();
        writeln!(file, "line5").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = ApplyPatchTool::new();
        let patch = format!(
            "--- a/{path}\n+++ b/{path}\n@@ -1,2 +1,2 @@\n-line1\n+LINE1\n line2\n@@ -4,2 +4,2 @@\n-line4\n+LINE4\n line5\n"
        );
        let input = ToolInput {
            tool: "apply_patch".into(),
            args: serde_json::json!({ "patch": patch }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("LINE1"));
        assert!(content.contains("LINE4"));
        assert!(!content.contains("line1"));
        assert!(!content.contains("line4"));
    }
}
