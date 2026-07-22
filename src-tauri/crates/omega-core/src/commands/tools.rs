// Tool command module — metadata-aware tool infrastructure
//
// Bridges the tool-harness metadata system into omega-core.
// Provides the AI-facing tool metadata layer that enriches LLM
// tool definitions with category, tags, examples, and error specs.

use crate::{AppState, MutexExt};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

// Re-export core types from tool-harness
pub use tool_harness::{ToolCategory, ToolMetadata, ToolRef, ToolRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub gate_result: Option<GateCheckResult>,
}

impl ToolResult {
    pub fn ok(output: String, gate_result: Option<GateCheckResult>) -> Self {
        Self {
            success: true,
            output,
            error: None,
            gate_result,
        }
    }
    pub fn err(error: String) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error),
            gate_result: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateViolationInfo {
    pub category: String,
    pub message: String,
    pub tool_hint: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCheckResult {
    pub passed: bool,
    pub score: u32,
    pub violations: Vec<GateViolationInfo>,
}

impl GateCheckResult {
    pub fn from_harness(g: &harness::GateResult) -> Self {
        Self {
            passed: g.passed,
            score: g.score,
            violations: g
                .violations
                .iter()
                .map(|v| GateViolationInfo {
                    category: format!("{:?}", v.category),
                    message: v.message.clone(),
                    tool_hint: v.tool_hint.clone(),
                    line: v.line,
                })
                .collect(),
        }
    }
}

async fn run_gate(state: &AppState, content: &str) -> GateCheckResult {
    let db = state.rules_db.lock_guard();
    let lang = state.detected_language.lock_guard().clone();
    let violations = db.check_content(content, &lang);

    if violations.is_empty() {
        return GateCheckResult {
            passed: true,
            score: 100,
            violations: vec![],
        };
    }

    let gate_result = harness::scoring::calculate_score(&violations);
    GateCheckResult::from_harness(&gate_result)
}

/// Execute a tool through the tool-harness pipeline, then apply omega-core gate checks
pub async fn execute_tool_inner(
    state: &AppState,
    request: ToolRequest,
) -> Result<ToolResult, String> {
    let tool_name = request.tool.clone();

    // Check MCP skills first
    if let Some(skill) = crate::commands::mcp::find_skill(&tool_name) {
        return crate::commands::mcp::invoke_skill(&skill, &request.args).await;
    }

    let tool_input = request.clone().into_input();

    let pipeline = state.tool_pipeline.get_or_init(|| {
        let registry = tool_harness::tools::default_tool_registry();
        tool_harness::ExecutionPipeline::new().with_registry(registry)
    });

    let ctx = tool_harness::ToolUseContext::new("omega-core");

    let (result, _budget) = pipeline
        .execute(&tool_name, tool_input, &ctx)
        .await
        .map_err(|e| e.message)?;

    // Gate check for write/edit operations
    let gate_result = if matches!(tool_name.as_str(), "write" | "edit") {
        let content_to_check = match tool_name.as_str() {
            "write" => request
                .args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "edit" => request
                .args
                .get("newText")
                .or_else(|| request.args.get("newString"))
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            _ => "",
        };
        if !content_to_check.is_empty() {
            Some(run_gate(state, content_to_check).await)
        } else {
            None
        }
    } else {
        None
    };

    // Rule promotion for gate failures
    if let Some(ref g) = gate_result {
        if !g.passed && matches!(tool_name.as_str(), "write" | "edit") {
            let mut db = state.rules_db.lock_guard();
            let lang = state.detected_language.lock_guard().clone();
            for v in &g.violations {
                let cat = v.category.to_lowercase();
                if let Some(pattern) = v.message.rsplit(": ").next() {
                    db.promote_or_increment(&lang, &cat, pattern, &v.message, "error");
                }
            }
        }
    }

    if result.success {
        Ok(ToolResult::ok(result.output, gate_result))
    } else {
        Ok(ToolResult {
            success: false,
            output: String::new(),
            error: result.error,
            gate_result,
        })
    }
}

pub async fn execute_tool(state: &AppState, request: ToolRequest) -> Result<ToolResult, String> {
    execute_tool_inner(state, request).await
}

// ─── Tool Registry Cache ─────────────────────────────────────────────────────

static CACHED_REGISTRY: OnceLock<tool_harness::ToolRegistry> = OnceLock::new();

fn registry() -> &'static tool_harness::ToolRegistry {
    CACHED_REGISTRY.get_or_init(|| tool_harness::tools::default_tool_registry())
}

// ─── Tool Listing ────────────────────────────────────────────────────────────

pub fn list_tools() -> Result<Vec<String>, String> {
    Ok(registry().list())
}

/// Return tools grouped by category.
pub fn list_by_category() -> Vec<(ToolCategory, Vec<ToolRef>)> {
    registry().list_by_category().into_iter().collect()
}

// ─── Tool Metadata & Search ──────────────────────────────────────────────────

/// Get full metadata for a specific tool.
pub fn get_tool_metadata(name: &str) -> Option<ToolMetadata> {
    registry().get_metadata(name)
}

/// Search tools by name, description, or tags.
pub fn search_tools(query: &str) -> Vec<ToolRef> {
    registry().search(query)
}

/// Get metadata for every registered tool.
pub fn all_tool_metadata() -> Vec<ToolMetadata> {
    registry().all_metadata()
}

// ─── System Prompt ───────────────────────────────────────────────────────────

pub const CHAT_SYSTEM_PROMPT: &str = r#"You are Omega Agent — a tool-using coding agent with filesystem and shell access.

## Operating rules
1. Investigate before editing: use read/grep/glob to find real paths. Never invent file paths.
2. Prefer `edit` over full-file `write` when a file already exists.
3. Make the smallest correct change. Do not refactor unrelated code.
4. After non-trivial edits, run relevant tests or `cargo check` / project build when possible.
5. If a tool fails, read the error, adapt, and retry — do not stop after one failure.
6. Be concise. Do not restate the whole task. Report what you changed and why.
7. Never claim you cannot access files or run commands — use tools.
8. Respect permission denials; explain what was blocked and offer alternatives.
9. Do not use destructive shell commands (rm -rf /, format, force-push) unless the user explicitly asks.
10. When output is truncated, re-query with a narrower path/pattern or offset/limit.

## Tools
Tools are provided via the native function-calling API. Call them through the API — do not invent a custom JSON protocol in plain text.
"#;

fn format_tool_help(def: &providers::ToolDefinition) -> String {
    let params: Vec<String> = def
        .function
        .parameters
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|props| {
            props
                .iter()
                .map(|(name, info)| {
                    let ptype = info.get("type").and_then(|v| v.as_str()).unwrap_or("any");
                    format!("{}: {}", name, ptype)
                })
                .collect()
        })
        .unwrap_or_default();
    if params.is_empty() {
        format!("- {}: {}", def.function.name, def.function.description)
    } else {
        format!(
            "- {}({}): {}",
            def.function.name,
            params.join(", "),
            def.function.description
        )
    }
}

/// Load optional project instructions (AGENTS.md / .omega/instructions.md), capped.
fn project_instructions_snippet() -> Option<String> {
    const CAP: usize = 8_000;
    let candidates = ["AGENTS.md", ".omega/instructions.md", "CLAUDE.md"];
    for path in candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                continue;
            }
            let body: String = if trimmed.chars().count() > CAP {
                let mut s: String = trimmed.chars().take(CAP).collect();
                s.push_str("\n...[truncated project instructions]");
                s
            } else {
                trimmed.to_string()
            };
            return Some(format!(
                "\n\n=== PROJECT INSTRUCTIONS ({path}) ===\n{body}\n"
            ));
        }
    }
    None
}

pub fn default_system_prompt() -> String {
    let mut prompt = CHAT_SYSTEM_PROMPT.to_string();
    let tools = tool_definitions();
    if !tools.is_empty() {
        prompt.push_str("\n\n=== AVAILABLE TOOLS ===\n");
        for t in &tools {
            prompt.push_str(&format_tool_help(t));
            prompt.push('\n');
        }
        prompt.push_str(
            "\nUse the provider's native tool/function calling. Do not print raw tool JSON as your only response unless the model has no tool API.\n",
        );
    }
    if let Some(project) = project_instructions_snippet() {
        prompt.push_str(&project);
    }
    prompt
}

static CACHED_TOOL_DEFINITIONS: OnceLock<Vec<providers::ToolDefinition>> = OnceLock::new();

pub fn tool_definitions() -> Vec<providers::ToolDefinition> {
    CACHED_TOOL_DEFINITIONS
        .get_or_init(|| {
            let mut defs = registry().tool_definitions();
            defs.extend(crate::commands::mcp::tool_definitions());
            defs
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_system_prompt_includes_tools() {
        let prompt = default_system_prompt();
        assert!(prompt.contains("read"), "prompt should include read tool");
        assert!(
            prompt.contains("AVAILABLE TOOLS") || prompt.contains("TOOL"),
            "prompt should list tools"
        );
        assert!(prompt.contains("bash"), "prompt should include bash tool");
        assert!(
            prompt.contains("Investigate before editing")
                || prompt.contains("tool-using coding agent"),
            "prompt should include coding-agent rules"
        );
        // Native tool API — should NOT force the old raw JSON protocol as the only path.
        assert!(
            !prompt.contains("Respond with ONLY a JSON function call"),
            "should not force raw JSON-only tool protocol"
        );
    }
}
