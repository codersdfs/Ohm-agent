use clap::Parser;
use colored::Colorize;
use omega_core::{commands, default_db_path, AppState, TerminalPrinter};
use serde::{Deserialize, Serialize};
use std::io::{stdout, Write};
use std::path::PathBuf;
use tokio::io::AsyncBufReadExt;

// ── Enhanced REPL with interactive UI ──────────────────────────────────

struct ReplState {
    tool_call_count: usize,
    file_operations: usize,
    model_name: String,
    provider_name: String,
}

impl ReplState {
    fn new(model_name: String, provider_name: String) -> Self {
        Self { tool_call_count: 0, file_operations: 0, model_name, provider_name }
    }
    fn record_tool_call(&mut self) { self.tool_call_count += 1; }
    fn record_file_op(&mut self) { self.file_operations += 1; }
}

fn print_welcome_banner(cfg: &providers::ProviderConfig) {
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════╗".bright_cyan());
    println!("{}", "║                    OMEGA AI CODING ASSISTANT                  ║".bright_green().bold());
    println!("{}", "╠══════════════════════════════════════════════════════════╣".bright_cyan());
    println!("{}", format!("║  Provider:    {}                               ║", cfg.kind).bright_yellow());
    println!("{}", format!("║  Model:       {}                               ║", cfg.model).bright_yellow());
    println!("{}", format!("║  Max Tokens:  {}                               ║", cfg.max_tokens).bright_yellow());
    println!("{}", format!("║  Temperature: {}                               ║", cfg.temperature).bright_yellow());
    println!("{}", "╚══════════════════════════════════════════════════════════╝".bright_cyan());
    println!();
}

fn print_help() {
    println!();
    println!("{}", "╔══════════════════════════════════════════════════════════╗".bright_cyan());
    println!("{}", "║                         HELP MENU                         ║".bright_green().bold());
    println!("{}", "╠══════════════════════════════════════════════════════════╣".bright_cyan());
    println!("{}", "║  help / ?       Show this help                          ║");
    println!("{}", "║  exit / quit    Exit Omega REPL                        ║");
    println!("{}", "║  clear / cls    Clear screen                           ║");
    println!("{}", "║  tools          List all available tools               ║");
    println!("{}", "╚══════════════════════════════════════════════════════════╝".bright_cyan());
    println!();
    println!("{}", "💡 Just type what you want me to do!".bright_white());
    println!("{}", "   Examples: 'read README.md', 'list files', 'fix the bug'".dimmed());
}

async fn enhanced_repl(override_provider: Option<String>) -> anyhow::Result<()> {
    let cfg = load_provider_config(override_provider);
    print_welcome_banner(&cfg);

    let state = AppState::new_with_provider_config(&default_db_path(), cfg.clone());
    let mut repl_state = ReplState::new(cfg.model.clone(), format!("{:?}", cfg.kind));

    println!("{}", "✨ Ready! Type your request or 'help' for commands:".bright_white());
    println!();

    let mut messages: Vec<providers::ChatMessage> = Vec::new();

    // Load MCP skills once
    let (mcp_loaded, mcp_errors) = omega_core::commands::mcp::load_skills();
    if mcp_loaded > 0 {
        eprintln!("  \u{25c6} {} MCP skills loaded", mcp_loaded);
    }
    for err in &mcp_errors {
        eprintln!("  \u{25c6} MCP: {}", err);
    }

    loop {
        print!("{}>{} ", "omega".bright_blue().bold(), " ".dimmed());
        stdout().flush()?;

        let mut input = String::new();
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
        if reader.read_line(&mut input).await.is_err() { break; }
        let trimmed = input.trim().to_string();
        if trimmed.is_empty() { continue; }

        match trimmed.to_lowercase().as_str() {
            "exit" | "quit" | "q" => { println!("👋 Goodbye!"); break; }
            "help" | "?" | "h" => { print_help(); continue; }
            "clear" | "cls" => { print!("\x1B[2J\x1B[1;1H"); print_welcome_banner(&cfg); continue; }
            "tools" => { show_tools(); continue; }
            _ => {}
        }

        let request = commands::chat::StreamMessageRequest {
            content: trimmed,
            agent_type: "chat".into(),
            provider: Some(cfg.clone()),
            system_prompt: None,
            permission_mode: "off".into(),
        };
        let emitter = TerminalPrinter::new();
        commands::chat::stream_message_with_history(&state, request, &emitter, &mut messages).await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        repl_state.record_tool_call();
    }
    Ok(())
}

// ── Config & Provider helpers ──────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct CliConfig { provider: Option<String>, model: Option<String>, base_url: Option<String> }

fn config_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "omega", "omega-agent")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn load_config() -> CliConfig {
    let path = config_dir().join("config.json");
    std::fs::create_dir_all(config_dir()).ok();
    std::fs::read_to_string(&path).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(CliConfig { provider: None, model: None, base_url: None })
}

fn load_provider_config(override_provider: Option<String>) -> providers::ProviderConfig {
    let mut cli_cfg = load_config();
    if let Some(p) = override_provider { cli_cfg.provider = Some(p); }
    let kind = cli_cfg.provider.as_deref().map(providers::ProviderKind::from_str).unwrap_or(providers::ProviderKind::OpenAI);
    providers::ProviderConfig {
        kind,
        api_key: std::env::var("OMEGA_API_KEY").ok().or_else(|| {
            let p = config_dir().join(".env");
            std::fs::read_to_string(&p).ok().map(|s| s.trim().to_string())
        }),
        base_url: cli_cfg.base_url,
        model: cli_cfg.model.unwrap_or_else(|| "llama3.1:8b".into()),
        max_tokens: 4096,
        temperature: 0.7,
    }
}

fn show_tools() {
    let registry = tool_harness::tools::default_tool_registry();
    let tools = registry.list();
    println!("{}", "Available tools:".bright_green().bold());
    for t in tools { println!("  ─ {}", t); }
}

// ── Entry point ────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "omega", version, about = "Omega Agent CLI — AI coding assistant")]
struct Cli {
    #[arg(short = 'p', long, help = "Override provider")]
    provider: Option<String>,
}

// entry point
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    enhanced_repl(cli.provider).await
}
