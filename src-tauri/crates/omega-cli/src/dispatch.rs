//! Command routing (P5 split from main.rs) — `run_chat` and `run_mcp_server`.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratata::prelude::*;

use omega_core::session::SessionStore;

use crate::app::App;
use crate::config_loader::load_provider_config;

pub fn run_chat(
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    session: Option<String>,
    new_session: bool,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
) -> Result<()> {
    let config = load_provider_config(provider, model, base_url, max_tokens, temperature);

    let model = config.model.clone();
    let kind = config.kind.to_string();

    let (session_store, session_load) = SessionStore::resolve(session, new_session)
        .map_err(|e| anyhow::anyhow!("session: {e}"))?;
    let session_id = session_store.id.clone();

    let app = App::new(config, session_store, session_load);

    // Create a tokio runtime for background streaming tasks
    let rt = tokio::runtime::Runtime::new()?;
    let _guard = rt.enter();

    let backend = CrosstermBackend::new(std::io::stdout());

    Application::new()
        .tick_rate(Duration::from_millis(80))
        .screen(app)
        .on_startup(|| {
            Command::Batch(vec![
                Command::EnableRawMode,
                Command::crossterm(crossterm::terminal::EnterAlternateScreen),
            ])
        })
        .on_shutdown(|| {
            Command::Batch(vec![
                Command::crossterm(crossterm::terminal::LeaveAlternateScreen),
                Command::DisableRawMode,
            ])
        })
        .build(std::io::stdout(), backend)?
        .run::<App>()?;

    // Session summary (tokens from global statics, config captured before run)
    let (tokens_in, tokens_out) = omega_core::commands::cost_tracker::session_token_counts();
    println!();
    println!("Ω Omega Agent — session summary");
    println!("  Model:     {}", model);
    println!("  Provider:  {}", kind);
    println!("  Session:   {}", session_id);
    println!("  Tokens:    {} in / {} out", tokens_in, tokens_out);
    println!();

    Ok(())
}

pub fn run_mcp_server(port: u16, host: String, auth_token: Option<String>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        log::info!("Starting Omega MCP server on {host}:{port}");

        let mut transport = mcp_server::transport::http::HttpTransport::bind(&host, port);

        // Apply auth token if configured
        if let Some(ref token) = auth_token {
            transport = transport.with_auth_token(token.clone());
            println!("  Auth: Bearer token enabled");
        }

        println!("Ω Omega MCP Server");
        println!("  Listening on http://{host}:{port}");
        println!("  Protocol: MCP 2024-11-05");
        println!("  Tools: {}", list_tool_count());
        println!();
        println!("Press Ctrl+C to stop");

        // Build the MCP server with tool harness
        let registry = tool_harness::tools::default_tool_registry();
        let server = Arc::new(
            mcp_server::McpServer::new()
                .with_tool_registry(registry),
        );

        // Start serving — pass shutdown signal for graceful drain
        let serve = transport.serve(server, mcp_server::transport::http::shutdown_signal()).await
            .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {e}"))?;

        // Wait for shutdown signal (Ctrl+C / SIGTERM), then drain
        log::info!("Shutting down MCP server");
        serve.await.ok();

        // Print server stats on shutdown
        println!();
        println!("Ω MCP Server stopped");
        Ok(())
    })
}

fn list_tool_count() -> usize {
    let registry = tool_harness::tools::default_tool_registry();
    registry.list().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that run_chat's argument wiring produces the expected config.
    /// We test the config-building path without launching the TUI.
    #[test]
    fn run_chat_config_building_with_max_tokens_and_temperature() {
        let config = load_provider_config(
            Some("anthropic".into()),
            Some("claude-sonnet-4".into()),
            None,
            Some(8192),
            Some(1.2),
        );
        assert!(matches!(config.kind, providers::ProviderKind::Anthropic));
        assert_eq!(config.model, "claude-sonnet-4");
        assert_eq!(config.max_tokens, 8192);
        assert!((config.temperature - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn run_chat_config_defaults_when_no_overrides() {
        // Clean env to ensure defaults
        std::env::remove_var("OMEGA_API_KEY");
        std::env::remove_var("OMEGA_MODEL");
        std::env::remove_var("OMEGA_MAX_TOKENS");
        std::env::remove_var("OMEGA_TEMPERATURE");

        let config = load_provider_config(None, None, None, None, None);
        // No API key → Local provider
        assert!(matches!(config.kind, providers::ProviderKind::Local));
        assert_eq!(config.max_tokens, 4096);
        assert!((config.temperature - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn run_chat_config_cli_overrides_take_priority() {
        let config = load_provider_config(
            Some("openai".into()),
            Some("gpt-4o".into()),
            Some("https://custom.api.test".into()),
            Some(16384),
            Some(0.1),
        );
        assert!(matches!(config.kind, providers::ProviderKind::OpenAI));
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.base_url.as_deref(), Some("https://custom.api.test"));
        assert_eq!(config.max_tokens, 16384);
        assert!((config.temperature - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn run_chat_config_max_tokens_zero_override() {
        // Edge case: user explicitly sets max_tokens to 0
        let config = load_provider_config(None, None, None, Some(0), None);
        assert_eq!(config.max_tokens, 0);
    }

    #[test]
    fn run_chat_config_temperature_zero_override() {
        // Edge case: user explicitly sets temperature to 0
        let config = load_provider_config(None, None, None, None, Some(0.0));
        assert!((config.temperature).abs() < f32::EPSILON);
    }
}
