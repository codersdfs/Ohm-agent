use crate::app::App;
use crate::message::Message;

pub fn cmd_connect(app: &mut App, endpoint: &str) {
    if endpoint.is_empty() {
        app.history
            .push(Message::system("Usage: /connect <mcp-endpoint-url>"));
        return;
    }

    let transport = mcp::transport::JsonRpcTransport::new(endpoint);
    app.mcp_transport = Some(std::sync::Arc::new(transport));

    let skills_dir = find_skills_dir();
    let (registry, errors) = mcp::skills::SkillsRegistry::load_dir(&skills_dir);

    for err in errors {
        app.history.push(Message::system(format!(
            "Skills warning: {err}"
        )));
    }

    let skill_count = registry.list().len();
    app.mcp_registry = registry;

    app.history.push(Message::system(format!(
        "Connected to MCP: {endpoint} — {skill_count} skill(s) loaded"
    )));
}

pub fn cmd_skills(app: &mut App) {
    let skills = app.mcp_registry.list();
    if skills.is_empty() {
        app.history
            .push(Message::system("No skills registered. Use /connect <endpoint> to load."));
        return;
    }

    let mut output = format!("{} registered skill(s):", skills.len());
    for skill in skills {
        output.push_str(&format!("\n  {} — {}", skill.name, skill.description));
    }
    app.history.push(Message::system(output));
}

fn find_skills_dir() -> std::path::PathBuf {
    if let Ok(env_path) = std::env::var("OMEGA_CLI_SKILLS_DIR") {
        let p = std::path::PathBuf::from(env_path);
        if p.exists() {
            return p;
        }
    }

    let cwd_skills = std::path::PathBuf::from("skills");
    if cwd_skills.exists() {
        return cwd_skills;
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let exe_skills = parent.join("skills");
            if exe_skills.exists() {
                return exe_skills;
            }
        }
    }

    cwd_skills
}
