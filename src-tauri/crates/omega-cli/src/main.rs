// ── Omega Agent TUI ───────────────────────────────────────────────────────────
// Ratatui + ratata full-screen terminal UI.

mod app;
mod cli;
mod config_loader;
mod dispatch;
#[cfg(test)]
mod testutil;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, CliAction};

fn main() -> Result<()> {
    // Full-screen TUI owns stdout/stderr via the alternate screen. Log output at
    // `info` would write behind Ratatui and corrupt the layout, so default to
    // `error` unless the caller explicitly exports RUST_LOG.
    let default_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "error".to_string());
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&default_filter))
        .init();
    let cli = Cli::parse();

    match cli.action.unwrap_or_default() {
        CliAction::Chat {
            provider,
            model,
            base_url,
            session,
            new_session,
            max_tokens,
            temperature,
        } => dispatch::run_chat(provider, model, base_url, session, new_session, max_tokens, temperature),
        CliAction::ServeMcp {
            port,
            host,
            auth_token,
            skills_dir: _,
        } => dispatch::run_mcp_server(port, host, auth_token),
    }
}
