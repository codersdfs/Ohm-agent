// Tool command module — metadata-aware tool infrastructure
//
// Bridges the tool-harness metadata system into omega-core.
// Provides the AI-facing tool metadata layer that enriches LLM
// tool definitions with category, tags, examples, and error specs.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use crate::{AppState, MutexExt};

// Re-export core types from tool-harness
pub use tool_harness::{ToolRequest, ToolCategory, ToolRef, ToolMetadata};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub gate_result: Option<GateCheckResult>,
}

impl ToolResult {
    pub fn ok(output: String, gate_result: Option<GateCheckResult>) -> Self {
        Self { success: true, output, error: None, gate_result }
    }
    pub fn err(error: String) -> Self {
        Self { success: false, output: String::new(), error: Some(error), gate_result: None }
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
            violations: g.violations.iter().map(|v| GateViolationInfo {
                category: format!("{:?}", v.category),
                message: v.message.clone(),
                tool_hint: v.tool_hint.clone(),
                line: v.line,
            }).collect(),
        }
    }
}

async fn run_gate(state: &AppState, content: &str) -> GateCheckResult {
    let db = state.rules_db.lock_guard();
    let lang = state.detected_language.lock_guard().clone();
    let violations = db.check_content(content, &lang);

    if violations.is_empty() {
        return GateCheckResult { passed: true, score: 100, violations: vec![] };
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
        tool_harness::ExecutionPipeline::new()
            .with_registry(registry)
    });

    let ctx = tool_harness::ToolUseContext::new("omega-core");

    let (result, _budget) = pipeline.execute(&tool_name, tool_input, &ctx)
        .await
        .map_err(|e| e.message)?;

    // Gate check for write/edit operations
    let gate_result = if matches!(tool_name.as_str(), "write" | "edit") {
        let content_to_check = match tool_name.as_str() {
            "write" => request.args.get("content").and_then(|v| v.as_str()).unwrap_or(""),
            "edit" => request.args.get("newText")
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
        Ok(ToolResult { success: false, output: String::new(), error: result.error, gate_result })
    }
}

pub async fn execute_tool(
    state: &AppState,
    request: ToolRequest,
) -> Result<ToolResult, String> {
    execute_tool_inner(state, request).await
}

// ─── Tool Registry Cache ─────────────────────────────────────────────────────

static CACHED_REGISTRY: OnceLock<tool_harness::ToolRegistry> = OnceLock::new();

fn registry() -> &'static tool_harness::ToolRegistry {
    CACHED_REGISTRY.get_or_init(|| {
        tool_harness::tools::default_tool_registry()
    })
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

pub const CHAT_SYSTEM_PROMPT: &str = r#"You are Omega Agent, an AI coding assistant. You have access to tools for reading, writing, editing, and searching files on the user's system.

When the user asks you to do something that requires using tools, call them directly. Do not describe what you would do — just do it.

Tool usage tips:
- Use `read` to inspect files before editing
- Use `edit` for targeted changes, `write` for new files or full rewrites
- Use `grep` to search code and `glob` to find files
- Use `bash` for system commands, builds, tests, and git operations
- Always prefer calling tools over generating code snippets for the user to copy-paste
"#;

pub fn default_system_prompt() -> String {
    CHAT_SYSTEM_PROMPT.to_string()
}

static CACHED_TOOL_DEFINITIONS: OnceLock<Vec<providers::ToolDefinition>> = OnceLock::new();

pub fn tool_definitions() -> Vec<providers::ToolDefinition> {
    CACHED_TOOL_DEFINITIONS.get_or_init(|| {
        let mut defs = registry().tool_definitions();
        defs.extend(crate::commands::mcp::tool_definitions());
        defs
    }).clone()
}
