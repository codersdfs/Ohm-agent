// MCP skills integration — load .mcp.json skills, invoke via JSON-RPC

use mcp::transport::JsonRpcTransport;
use mcp::McpRequest;
use mcp::Skill;
use std::sync::OnceLock;

static MCP_SKILLS: OnceLock<(Vec<Skill>, Vec<String>)> = OnceLock::new();

fn skills_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("OMEGA_SKILLS_DIR") {
        return std::path::PathBuf::from(dir);
    }
    directories::ProjectDirs::from("com", "omega", "omega-agent")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("skills")
}

/// Load MCP skills from `skills/` config dir (or `OMEGA_SKILLS_DIR` env).
/// Returns (count, errors). Safe to call multiple times — loads once atomically.
pub fn load_skills() -> (usize, Vec<String>) {
    let (skills, errors) = MCP_SKILLS.get_or_init(|| {
        let dir = skills_dir();
        let (registry, errors) = mcp::skills::SkillsRegistry::load_dir(&dir);
        let skills: Vec<Skill> = registry.list().to_vec();
        (skills, errors)
    });
    (skills.len(), errors.clone())
}

pub fn find_skill(name: &str) -> Option<Skill> {
    MCP_SKILLS
        .get()
        .and_then(|(skills, _)| skills.iter().find(|s| s.name == name).cloned())
}

pub fn loaded_skills() -> Vec<Skill> {
    MCP_SKILLS.get().map(|(s, _)| s.clone()).unwrap_or_default()
}

pub fn tool_definitions() -> Vec<providers::ToolDefinition> {
    loaded_skills()
        .into_iter()
        .map(|skill| providers::ToolDefinition {
            tool_type: "function".into(),
            function: providers::ToolFunctionDef {
                name: skill.name,
                description: skill.description,
                // Use the skill's own parameter schema if provided, otherwise fall back
                // to an open schema so the LLM can still pass arbitrary JSON arguments.
                parameters: skill.parameters.unwrap_or_else(|| {
                    serde_json::json!({
                        "type": "object",
                        "properties": {},
                        "additionalProperties": true,
                    })
                }),
            },
        })
        .collect()
}

/// Invoke an MCP skill by sending a JSON-RPC request to its endpoint.
/// The method is the skill name; params are the tool arguments from the LLM.
pub async fn invoke_skill(
    skill: &Skill,
    args: &serde_json::Value,
) -> Result<crate::commands::tools::ToolResult, String> {
    let transport = JsonRpcTransport::new(&skill.endpoint);

    let params = args.as_object().map(|obj| {
        obj.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<std::collections::HashMap<String, serde_json::Value>>()
    });

    let request = McpRequest {
        method: skill.name.clone(),
        params,
        id: uuid::Uuid::new_v4().to_string(),
    };

    let response = transport.send(request).await?;

    if let Some(err) = response.error {
        Ok(crate::commands::tools::ToolResult::err(err.message))
    } else {
        Ok(crate::commands::tools::ToolResult::ok(
            response.result.map(|r| r.to_string()).unwrap_or_default(),
            None,
        ))
    }
}
