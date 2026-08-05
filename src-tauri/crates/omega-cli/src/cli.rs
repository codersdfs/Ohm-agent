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
