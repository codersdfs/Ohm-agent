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
) -> Result<()> {
    let config = load_provider_config(provider, model, base_url);

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

        // Start serving — server is passed to transport to wire into Axum state
        let handle = transport.serve(server).await
            .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {e}"))?;

        // Ctrl+C handler
        tokio::signal::ctrl_c().await.ok();
        log::info!("Shutting down MCP server");
        handle.await.ok();

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
