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
    pub verification: Option<EditVerification>,
}

/// Deterministic post-write verification result.
/// After every `write`/`edit`, we read back the target file and run a
/// lightweight syntax sanity check (bracket balance +, for Rust, rustfmt --check
/// if available) to catch truncated or corrupted edits before they propagate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditVerification {
    pub passed: bool,
    pub language: String,
    pub issues: Vec<String>,
}

impl ToolResult {
    pub fn ok(output: String, gate_result: Option<GateCheckResult>) -> Self {
        Self {
            success: true,
            output,
            error: None,
            gate_result,
            verification: None,
        }
    }
    pub fn err(error: String) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error),
            gate_result: None,
            verification: None,
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

/// Rules-database-only check used AFTER execution for bookkeeping: it feeds
/// `ToolResult.gate_result` and drives negative-knowledge promotion.
///
/// Enforcement is NOT here — the live GateHook runs the full
/// `harness::GateEngine` (structural, taste, golden, external included) before
/// write/edit/apply_patch executes (see `gate_hook_from_state`). This helper
/// remains because promotion needs per-violation records against the rules DB.
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

    // Check agent skills (load_skill tool)
    if tool_name == "load_skill" {
        let name = request
            .args
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let skill_args = request.args.get("args").and_then(|v| v.as_str()).unwrap_or("");
        return match crate::commands::agent_skills::load_skill(name, skill_args) {
            Some(skill) => {
                let injection = crate::commands::agent_skills::format_skill_for_injection(&skill);
                Ok(ToolResult::ok(injection, None))
            }
            None => Ok(ToolResult::err(format!(
                "Skill '{}' not found. Available skills: {}",
                name,
                crate::commands::agent_skills::list_skills()
                    .iter()
                    .map(|s| s.frontmatter.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        };
    }

    let tool_input = request.clone().into_input();

    // Shared pipeline: registry + live Gate enforcement hook + real hook
    // context, all built once. The GateHook scores write/edit/apply_patch
    // through the FULL harness GateEngine (structural, taste, golden, rules,
    // repeated, external) BEFORE the tool runs, so a sub-threshold write never
    // touches disk (mode from OMEGA_GATE_MODE, default warn).
    let pipeline = state.tool_pipeline.get_or_init(|| {
        let registry = tool_harness::tools::default_tool_registry();
        let mut hooks = tool_harness::HooksRegistry::new();
        hooks.register(Box::new(crate::gate_hook::gate_hook_from_state(state)));
        let session_id = state.session_id().unwrap_or_default();
        let hook_ctx = tool_harness::HookContext {
            session_id,
            turn_id: None,
            workspace: state.workspace_root.clone(),
        };
        tool_harness::ExecutionPipeline::new()
            .with_registry(registry)
            .with_hooks(hooks)
            .with_hook_context(hook_ctx)
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
        let verification = if matches!(tool_name.as_str(), "write" | "edit") {
            let lang = state.detected_language.lock_guard().clone();
            verify_edit(&request, &lang).await
        } else {
            None
        };
        let mut tr = ToolResult::ok(result.output, gate_result);
        tr.verification = verification;
        Ok(tr)
    } else {
        Ok(ToolResult {
            success: false,
            output: String::new(),
            error: result.error,
            gate_result,
            verification: None,
        })
    }
}

/// Read back the written/edited file and run a lightweight syntax sanity check.
/// ponytail: minimal verification — bracket balance (all langs) + rustfmt --check (Rust only).
/// Catches truncated/corrupted edits in microseconds. Upgrade path: delegate to
/// real parser on demand.
async fn verify_edit(request: &ToolRequest, lang: &harness::Language) -> Option<EditVerification> {
    let path = request
        .args
        .get("filePath")
        .and_then(|v| v.as_str())
        .or_else(|| request.args.get("path").and_then(|v| v.as_str()))?;

    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(e) => {
            return Some(EditVerification {
                passed: false,
                language: lang.label(),
                issues: vec![format!("Failed to read back {}: {}", path, e)],
            });
        }
    };

    let mut issues = verify_brackets(&content);

    // For Rust files, try `rustfmt --check` if available — catches syntax errors.
    if matches!(lang, harness::Language::Rust) {
        issues.extend(verify_rustfmt(path));
    }

    Some(EditVerification {
        passed: issues.is_empty(),
        language: lang.label(),
        issues,
    })
}

/// Check that (), {}, [] are balanced — catches truncated edits.
fn verify_brackets(content: &str) -> Vec<String> {
    let mut issues = vec![];
    let mut paren = 0i32;
    let mut brace = 0i32;
    let mut bracket = 0i32;
    for ch in content.chars() {
        match ch {
            '(' => paren += 1,
            ')' => paren -= 1,
            '{' => brace += 1,
            '}' => brace -= 1,
            '[' => bracket += 1,
            ']' => bracket -= 1,
            _ => {}
        }
        if paren < 0 || brace < 0 || bracket < 0 {
            issues.push("Unmatched closing bracket — file may be truncated".into());
            return issues;
        }
    }
    if paren != 0 {
        issues.push(format!("Unbalanced parentheses: {} remaining", paren));
    }
    if brace != 0 {
        issues.push(format!("Unbalanced braces: {} remaining", brace));
    }
    if bracket != 0 {
        issues.push(format!("Unbalanced brackets: {} remaining", bracket));
    }
    issues
}

/// Run `rustfmt --check` on a Rust file and return issues if there are syntax errors.
/// Exit code 2 = hard parse error; exit code 1 = formatting diff (not an error).
/// If rustfmt isn't installed, returns empty (silently skipped).
/// ponytail: best-effort, zero-cost when rustfmt absent.
fn verify_rustfmt(path: &str) -> Vec<String> {
    use std::process::Command;
    match Command::new("rustfmt").arg("--check").arg(path).status() {
        Ok(status) if !status.success() => {
            if status.code() == Some(2) {
                vec![format!("rustfmt reports syntax error in {}", path)]
            } else {
                vec![]
            }
        }
        _ => vec![],
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

## Agent Skills
The AGENT SKILLS section at the end of this prompt lists available skills with short descriptions. When a user's task matches a skill description, call `load_skill` with that skill's name to load its full instructions. Use the loaded content to guide your work. Do not load skills that are irrelevant to the task.
"#;

pub fn format_tool_help(def: &providers::ToolDefinition) -> String {
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
///
/// Cached by file mtime: the snippet is re-read from disk only when a
/// candidate instruction file actually changes on disk, so repeated
/// `default_system_prompt()`/`send_message` calls don't do redundant I/O or
/// re-embed an unchanged (potentially ~2k-token) instructions block.
pub fn project_instructions_snippet() -> Option<String> {
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

static CACHED_SYSTEM_PROMPT: std::sync::Mutex<Option<(Option<(String, u64)>, String)>> =
    std::sync::Mutex::new(None);

pub fn default_system_prompt() -> String {
    // Fingerprint of the project-instructions sources (mtime of any candidate
    // that exists). `None` sentinel = no instructions loaded.
    fn instructions_mtime() -> Option<(String, u64)> {
        for path in ["AGENTS.md", ".omega/instructions.md", "CLAUDE.md"] {
            if let Ok(meta) = std::fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(since_epoch) = modified.duration_since(std::time::UNIX_EPOCH) {
                        return Some((path.to_string(), since_epoch.as_millis() as u64));
                    }
                }
            }
        }
        None
    }

    let sig = instructions_mtime();
    if let Ok(mut guard) = CACHED_SYSTEM_PROMPT.lock() {
        if let Some((cached_sig, cached_prompt)) = guard.as_ref() {
            if *cached_sig == sig {
                return cached_prompt.clone();
            }
        }
        let prompt = build_system_prompt();
        *guard = Some((sig, prompt.clone()));
        return prompt;
    }

    build_system_prompt()
}

/// Static base agent instructions. Never changes at runtime.
pub fn build_base_prompt() -> String {
    CHAT_SYSTEM_PROMPT.to_string()
}

/// Formats the available tools section of the system prompt. Cached at the
/// `tool_definitions()` level (OnceLock); recomputed only when the process
/// restarts. Returns `None` when no tools are registered.
pub fn build_tools_section() -> Option<String> {
    let tools = tool_definitions();
    if tools.is_empty() {
        return None;
    }
    let mut section = String::from("\n\n=== AVAILABLE TOOLS ===\n");
    for t in &tools {
        section.push_str(&format_tool_help(t));
        section.push('\n');
    }
    section.push_str(
        "\nUse the provider's native tool/function calling. Do not print raw tool JSON as your only response unless the model has no tool API.\n",
    );
    Some(section)
}

/// Reads and formats the project-instructions snippet (AGENTS.md /
/// .omega/instructions.md / CLAUDE.md). Returns `None` when no candidate
/// file exists. Disk I/O on every call — callers are expected to cache by
/// mtime (see `default_system_prompt`).
pub fn build_project_instructions() -> Option<String> {
    project_instructions_snippet()
}

/// Assembles the full system prompt from its independent parts.
fn build_system_prompt() -> String {
    let mut prompt = build_base_prompt();
    if let Some(tools) = build_tools_section() {
        prompt.push_str(&tools);
    }
    if let Some(project) = build_project_instructions() {
        prompt.push_str(&project);
    }
    if let Some(skills) = crate::commands::agent_skills::skill_index() {
        prompt.push_str(&skills);
    }
    prompt
}

static CACHED_TOOL_DEFINITIONS: OnceLock<Vec<providers::ToolDefinition>> = OnceLock::new();

pub fn tool_definitions() -> Vec<providers::ToolDefinition> {
    CACHED_TOOL_DEFINITIONS
        .get_or_init(|| {
            let mut defs = registry().tool_definitions();
            defs.extend(crate::commands::mcp::tool_definitions());
            // Agent skills tool: lets the LLM load skill content on demand
            defs.push(providers::ToolDefinition {
                tool_type: "function".into(),
                function: providers::ToolFunctionDef {
                    name: "load_skill".into(),
                    description: "Load an agent skill by name to get its full instructions. Use this when the skill index in the system prompt matches the user's task. The skill content will be returned as context for your response.".into(),
                    parameters: serde_json::json!({
                        "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The skill name from the index (e.g. 'omega-agent')"
                        },
                        "args": {
                            "type": "string",
                            "description": "Arguments to pass to the skill (substituted into $ARGUMENTS, $0, $1, etc.)",
                            "default": ""
                        }
                    },
                    "required": ["name"]
                    }),
                },
            });
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

    #[test]
    fn test_verify_brackets_balanced() {
        assert!(verify_brackets("fn x() { let v = vec![1, 2]; }").is_empty());
    }

    #[test]
    fn test_verify_brackets_unbalanced() {
        let issues = verify_brackets("fn x() { let v = vec![1, 2]; ");
        assert!(!issues.is_empty());
        assert!(issues[0].contains("braces"));
    }

    #[test]
    fn test_verify_brackets_extra_closing() {
        let issues = verify_brackets("fn x() { } }");
        assert!(issues.iter().any(|i| i.contains("truncated")));
    }
}
