//! Command palette — searchable list of slash commands.
//!
//! Split from the original monolith (P5 god-object split):
//! - [`mod`](self): handler variants, command catalog, lookup
//! - [`dispatch`]: filtering/ranking
//! - [`state`]: `CommandPaletteState`
//! - [`ui`]: key handling + rendering

mod dispatch;
mod state;
mod ui;

pub use dispatch::{command_matches, filter_commands};
pub use state::CommandPaletteState;
pub use ui::{handle_key, render, render_panel, PaletteAction};

/// Handler variant for each slash command. This is the single source of truth
/// for command dispatch — the palette and the handler in `main.rs` both use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandHandler {
    Help,
    Clear,
    Tools,
    Model,
    Provider,
    Cost,
    Exit,
    Fetch,
    Status,
    Search,
    Gate,
    Rules,
    Score,
    Memory,
    MemStore,
    MemSearch,
    MemList,
}

/// One palette row / slash command.
#[derive(Debug, Clone, Copy)]
pub struct CommandEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    /// Extra search terms not shown in the UI.
    pub keywords: &'static [&'static str],
    /// Handler variant for dispatch.
    pub handler: CommandHandler,
}

/// Canonical v1 catalog. The `handler` field is the single source of truth
/// for command dispatch — both the palette and `App::handle_slash_command`
/// use it.
pub static COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        id: "/help",
        label: "Help",
        aliases: &["/?", "/h"],
        description: "Show available commands",
        keywords: &["commands", "usage", "docs"],
        handler: CommandHandler::Help,
    },
    CommandEntry {
        id: "/clear",
        label: "Clear session",
        aliases: &["/cls"],
        description: "Clear transcript and session",
        keywords: &["reset", "new", "session"],
        handler: CommandHandler::Clear,
    },
    CommandEntry {
        id: "/tools",
        label: "List tools",
        aliases: &[],
        description: "List available agent tools",
        keywords: &["agent", "capabilities"],
        handler: CommandHandler::Tools,
    },
    CommandEntry {
        id: "/model",
        label: "Choose model",
        aliases: &[],
        description: "Open model picker for current provider",
        keywords: &["llm", "gpt", "claude", "switch"],
        handler: CommandHandler::Model,
    },
    CommandEntry {
        id: "/provider",
        label: "Choose provider",
        aliases: &["/providers", "/p"],
        description: "Open provider configuration wizard",
        keywords: &["api", "openai", "anthropic", "google", "endpoint"],
        handler: CommandHandler::Provider,
    },
    CommandEntry {
        id: "/cost",
        label: "Session cost",
        aliases: &[],
        description: "Show session token usage",
        keywords: &["tokens", "usage", "billing"],
        handler: CommandHandler::Cost,
    },
    CommandEntry {
        id: "/exit",
        label: "Quit",
        aliases: &["/quit"],
        description: "Quit Omega",
        keywords: &["quit", "close", "leave"],
        handler: CommandHandler::Exit,
    },
    CommandEntry {
        id: "/fetch",
        label: "Fetch URL",
        aliases: &["/web", "/url"],
        description: "Fetch and display content from a URL",
        keywords: &["http", "web", "internet", "download", "curl", "get"],
        handler: CommandHandler::Fetch,
    },
    CommandEntry {
        id: "/status",
        label: "System status",
        aliases: &["/ping", "/health", "/net"],
        description: "Check network connectivity and provider status",
        keywords: &["network", "connectivity", "reachable", "health", "ping"],
        handler: CommandHandler::Status,
    },
    CommandEntry {
        id: "/search",
        label: "Web search",
        aliases: &["/google", "/websearch"],
        description: "Search the web",
        keywords: &["google", "duckduckgo", "web", "browse", "find"],
        handler: CommandHandler::Search,
    },
    CommandEntry {
        id: "/gate",
        label: "Run Gate",
        aliases: &[],
        description: "Run Mechanized Gate on a file",
        keywords: &["gate", "lint", "rules", "check", "score", "violations"],
        handler: CommandHandler::Gate,
    },
    CommandEntry {
        id: "/rules",
        label: "List rules",
        aliases: &["/pattern"],
        description: "List promoted negative patterns",
        keywords: &["rules", "pattern", "negative", "promoted", "frequency"],
        handler: CommandHandler::Rules,
    },
    CommandEntry {
        id: "/score",
        label: "Quality score",
        aliases: &[],
        description: "Quick quality score for a file",
        keywords: &["score", "quality", "grade", "pass", "fail"],
        handler: CommandHandler::Score,
    },
    CommandEntry {
        id: "/memory",
        label: "Search memory",
        aliases: &["/mem"],
        description: "Search Hermes memory",
        keywords: &["memory", "hermes", "search", "remember", "session", "project", "user"],
        handler: CommandHandler::Memory,
    },
    CommandEntry {
        id: "/mem-store",
        label: "Store memory",
        aliases: &["/memstore"],
        description: "Store a key-value pair in project memory",
        keywords: &["memory", "store", "save", "kv", "key", "value"],
        handler: CommandHandler::MemStore,
    },
    CommandEntry {
        id: "/mem-search",
        label: "Search project memory",
        aliases: &["/memfind"],
        description: "Search project-layer memory",
        keywords: &["memory", "search", "find", "query", "project"],
        handler: CommandHandler::MemSearch,
    },
    CommandEntry {
        id: "/mem-list",
        label: "List project memory",
        aliases: &["/memls"],
        description: "List all project-layer memories",
        keywords: &["memory", "list", "show", "all", "entries", "project"],
        handler: CommandHandler::MemList,
    },
];

/// Look up a command by its id or alias and return the matching entry.
/// This is the single source of truth for command dispatch — both the
/// palette and `App::handle_slash_command` use it.
pub fn lookup_command(cmd: &str) -> Option<&'static CommandEntry> {
    let normalized = cmd.to_lowercase();
    COMMANDS.iter().find(|entry| {
        entry.id == normalized
            || entry.aliases.iter().any(|alias| alias == &normalized)
    })
}
