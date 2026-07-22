//! Command palette — searchable list of slash commands.

/// One palette row / slash command.
#[derive(Debug, Clone, Copy)]
pub struct CommandEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    /// Extra search terms not shown in the UI.
    pub keywords: &'static [&'static str],
}

/// Canonical v1 catalog. Ids must match `App::handle_slash_command`.
pub static COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        id: "/help",
        label: "Help",
        aliases: &["/?", "/h"],
        description: "Show available commands",
        keywords: &["commands", "usage", "docs"],
    },
    CommandEntry {
        id: "/clear",
        label: "Clear session",
        aliases: &["/cls"],
        description: "Clear transcript and session",
        keywords: &["reset", "new", "session"],
    },
    CommandEntry {
        id: "/tools",
        label: "List tools",
        aliases: &[],
        description: "List available agent tools",
        keywords: &["agent", "capabilities"],
    },
    CommandEntry {
        id: "/model",
        label: "Choose model",
        aliases: &[],
        description: "Open model picker for current provider",
        keywords: &["llm", "gpt", "claude", "switch"],
    },
    CommandEntry {
        id: "/provider",
        label: "Choose provider",
        aliases: &["/providers", "/p"],
        description: "Open provider configuration wizard",
        keywords: &["api", "openai", "anthropic", "google", "endpoint"],
    },
    CommandEntry {
        id: "/cost",
        label: "Session cost",
        aliases: &[],
        description: "Show session token usage",
        keywords: &["tokens", "usage", "billing"],
    },
    CommandEntry {
        id: "/exit",
        label: "Quit",
        aliases: &["/quit"],
        description: "Quit Omega",
        keywords: &["quit", "close", "leave"],
    },
];

/// Build lowercase haystack for keyword search.
fn haystack(entry: &CommandEntry) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(4 + entry.aliases.len() + entry.keywords.len());
    parts.push(entry.id);
    parts.push(entry.label);
    parts.extend(entry.aliases.iter().copied());
    parts.push(entry.description);
    parts.extend(entry.keywords.iter().copied());
    parts.join(" ").to_lowercase()
}

/// True if every whitespace-separated keyword is a substring of the entry haystack.
pub fn command_matches(entry: &CommandEntry, query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    let hay = haystack(entry);
    q.split_whitespace()
        .all(|kw| hay.contains(&kw.to_lowercase()))
}

/// Indices into `COMMANDS` matching `query` (stable registry order).
pub fn filter_commands(query: &str) -> Vec<usize> {
    COMMANDS
        .iter()
        .enumerate()
        .filter(|(_, e)| command_matches(e, query))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_empty_returns_all() {
        let ids: Vec<_> = filter_commands("")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert_eq!(
            ids,
            vec![
                "/help",
                "/clear",
                "/tools",
                "/model",
                "/provider",
                "/cost",
                "/exit"
            ]
        );
    }

    #[test]
    fn filter_substring_matches_clear() {
        let ids: Vec<_> = filter_commands("cle")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert_eq!(ids, vec!["/clear"]);
    }

    #[test]
    fn filter_alias_cls() {
        let ids: Vec<_> = filter_commands("cls")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert_eq!(ids, vec!["/clear"]);
    }

    #[test]
    fn filter_multi_keyword_token_cost() {
        let ids: Vec<_> = filter_commands("token cost")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert_eq!(ids, vec!["/cost"]);
    }

    #[test]
    fn filter_keyword_quit_matches_exit() {
        let ids: Vec<_> = filter_commands("quit")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/exit"));
    }

    #[test]
    fn filter_no_match() {
        assert!(filter_commands("zzz").is_empty());
    }
}
