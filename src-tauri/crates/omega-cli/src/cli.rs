//! Clap CLI definitions — `Cli` struct + `CliAction` enum (P5 split).

use clap::Parser;

/// Omega Agent actions
#[derive(clap::Subcommand, Debug, Clone)]
pub enum CliAction {
    /// Start the chat TUI (default)
    Chat {
        #[arg(
            short = 'p',
            long,
            help = "Provider (openai, anthropic, google, local, ollama, groq, etc.)"
        )]
        provider: Option<String>,

        #[arg(
            short = 'm',
            long,
            help = "Model name (e.g. gpt-4o-mini, llama3.1:8b, claude-sonnet-4)"
        )]
        model: Option<String>,

        #[arg(short = 'b', long, help = "Base URL for the provider API")]
        base_url: Option<String>,

        /// Resume a specific conversation session by id
        #[arg(long = "session", value_name = "ID", help = "Resume session <id>")]
        session: Option<String>,

        /// Force a brand-new conversation session (ignore last-session marker)
        #[arg(
            long = "new-session",
            help = "Start a new session instead of resuming the last one"
        )]
        new_session: bool,

        /// Maximum tokens to generate in the response (default: 4096)
        #[arg(long = "max-tokens", value_name = "N", help = "Max tokens to generate")]
        max_tokens: Option<u32>,

        /// Sampling temperature 0.0–2.0 (default: 0.7)
        #[arg(long = "temperature", value_name = "T", help = "Sampling temperature")]
        temperature: Option<f32>,
    },

    /// Start the MCP server to expose agent tools via Model Context Protocol
    ServeMcp {
        #[arg(long, default_value = "3100", help = "Port to listen on")]
        port: u16,

        #[arg(long, default_value = "127.0.0.1", help = "Host address to bind")]
        host: String,

        #[arg(long, help = "Authentication token for MCP requests")]
        auth_token: Option<String>,

        #[arg(long, help = "Directory to load custom MCP skills from")]
        skills_dir: Option<String>,
    },
}

impl Default for CliAction {
    fn default() -> Self {
        Self::Chat {
            provider: None,
            model: None,
            base_url: None,
            session: None,
            new_session: false,
            max_tokens: None,
            temperature: None,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "omega",
    version,
    about = "Omega Agent TUI — AI coding assistant",
    subcommand_negates_reqs = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub action: Option<CliAction>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // ── Default ──────────────────────────────────────────────────────

    #[test]
    fn default_is_chat_with_all_none() {
        let action = CliAction::default();
        match action {
            CliAction::Chat {
                provider,
                model,
                base_url,
                session,
                new_session,
                max_tokens,
                temperature,
            } => {
                assert_eq!(provider, None);
                assert_eq!(model, None);
                assert_eq!(base_url, None);
                assert_eq!(session, None);
                assert!(!new_session);
                assert_eq!(max_tokens, None);
                assert_eq!(temperature, None);
            }
            _ => panic!("default should be Chat"),
        }
    }

    // ── No subcommand → default Chat ─────────────────────────────────

    #[test]
    fn no_subcommand_defaults_to_chat() {
        // Bare invocation (no subcommand) parses to `None`; the Chat default
        // is applied at dispatch time via `CliAction::default()` in main.rs.
        let cli = Cli::parse_from(["omega"]);
        assert!(cli.action.is_none());
        match cli.action.unwrap_or_default() {
            CliAction::Chat { provider, .. } => {
                assert_eq!(provider, None);
            }
            _ => panic!("no subcommand should default to Chat"),
        }
    }

    // ── Provider flag ────────────────────────────────────────────────

    #[test]
    fn parse_provider_short() {
        let cli = Cli::parse_from(["omega", "chat", "-p", "anthropic"]);
        match cli.action.unwrap() {
            CliAction::Chat { provider, .. } => {
                assert_eq!(provider.as_deref(), Some("anthropic"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn parse_provider_long() {
        let cli = Cli::parse_from(["omega", "chat", "--provider", "groq"]);
        match cli.action.unwrap() {
            CliAction::Chat { provider, .. } => {
                assert_eq!(provider.as_deref(), Some("groq"));
            }
            _ => unreachable!(),
        }
    }

    // ── Model flag ───────────────────────────────────────────────────

    #[test]
    fn parse_model_short() {
        let cli = Cli::parse_from(["omega", "chat", "-m", "gpt-4o"]);
        match cli.action.unwrap() {
            CliAction::Chat { model, .. } => {
                assert_eq!(model.as_deref(), Some("gpt-4o"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn parse_model_long() {
        let cli = Cli::parse_from(["omega", "chat", "--model", "llama3.1:8b"]);
        match cli.action.unwrap() {
            CliAction::Chat { model, .. } => {
                assert_eq!(model.as_deref(), Some("llama3.1:8b"));
            }
            _ => unreachable!(),
        }
    }

    // ── max_tokens flag ──────────────────────────────────────────────

    #[test]
    fn parse_max_tokens() {
        let cli = Cli::parse_from(["omega", "chat", "--max-tokens", "8192"]);
        match cli.action.unwrap() {
            CliAction::Chat { max_tokens, .. } => {
                assert_eq!(max_tokens, Some(8192));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn max_tokens_absent_is_none() {
        let cli = Cli::parse_from(["omega", "chat"]);
        match cli.action.unwrap() {
            CliAction::Chat { max_tokens, .. } => {
                assert_eq!(max_tokens, None);
            }
            _ => unreachable!(),
        }
    }

    // ── temperature flag ─────────────────────────────────────────────

    #[test]
    fn parse_temperature() {
        let cli = Cli::parse_from(["omega", "chat", "--temperature", "1.5"]);
        match cli.action.unwrap() {
            CliAction::Chat { temperature, .. } => {
                assert!((temperature.unwrap() - 1.5).abs() < f32::EPSILON);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn temperature_absent_is_none() {
        let cli = Cli::parse_from(["omega", "chat"]);
        match cli.action.unwrap() {
            CliAction::Chat { temperature, .. } => {
                assert_eq!(temperature, None);
            }
            _ => unreachable!(),
        }
    }

    // ── session / new-session flags ──────────────────────────────────

    #[test]
    fn parse_session() {
        let cli = Cli::parse_from(["omega", "chat", "--session", "abc-123"]);
        match cli.action.unwrap() {
            CliAction::Chat { session, .. } => {
                assert_eq!(session.as_deref(), Some("abc-123"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn parse_new_session() {
        let cli = Cli::parse_from(["omega", "chat", "--new-session"]);
        match cli.action.unwrap() {
            CliAction::Chat { new_session, .. } => {
                assert!(new_session);
            }
            _ => unreachable!(),
        }
    }

    // ── base_url flag ────────────────────────────────────────────────

    #[test]
    fn parse_base_url() {
        let cli = Cli::parse_from(["omega", "chat", "-b", "http://localhost:11434"]);
        match cli.action.unwrap() {
            CliAction::Chat { base_url, .. } => {
                assert_eq!(base_url.as_deref(), Some("http://localhost:11434"));
            }
            _ => unreachable!(),
        }
    }

    // ── All flags combined ───────────────────────────────────────────

    #[test]
    fn parse_all_chat_flags() {
        let cli = Cli::parse_from([
            "omega", "chat",
            "-p", "openai",
            "-m", "gpt-4o",
            "-b", "https://api.test.com/v1",
            "--session", "sess-42",
            "--new-session",
            "--max-tokens", "16384",
            "--temperature", "0.3",
        ]);
        match cli.action.unwrap() {
            CliAction::Chat {
                provider,
                model,
                base_url,
                session,
                new_session,
                max_tokens,
                temperature,
            } => {
                assert_eq!(provider.as_deref(), Some("openai"));
                assert_eq!(model.as_deref(), Some("gpt-4o"));
                assert_eq!(base_url.as_deref(), Some("https://api.test.com/v1"));
                assert_eq!(session.as_deref(), Some("sess-42"));
                assert!(new_session);
                assert_eq!(max_tokens, Some(16384));
                assert!((temperature.unwrap() - 0.3).abs() < f32::EPSILON);
            }
            _ => unreachable!(),
        }
    }

    // ── MCP subcommand ───────────────────────────────────────────────

    #[test]
    fn parse_serve_mcp_defaults() {
        let cli = Cli::parse_from(["omega", "serve-mcp"]);
        match cli.action.unwrap() {
            CliAction::ServeMcp {
                port,
                host,
                auth_token,
                skills_dir,
            } => {
                assert_eq!(port, 3100);
                assert_eq!(host, "127.0.0.1");
                assert_eq!(auth_token, None);
                assert_eq!(skills_dir, None);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn parse_serve_mcp_custom_port_and_auth() {
        let cli = Cli::parse_from([
            "omega", "serve-mcp",
            "--port", "8080",
            "--host", "0.0.0.0",
            "--auth-token", "secret123",
        ]);
        match cli.action.unwrap() {
            CliAction::ServeMcp {
                port,
                host,
                auth_token,
                ..
            } => {
                assert_eq!(port, 8080);
                assert_eq!(host, "0.0.0.0");
                assert_eq!(auth_token.as_deref(), Some("secret123"));
            }
            _ => unreachable!(),
        }
    }
}
