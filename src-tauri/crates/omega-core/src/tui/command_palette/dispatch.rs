//! Command catalog filtering/ranking (P5 split from command_palette.rs).

use super::{CommandEntry, COMMANDS};

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

/// Rank a command entry against a query. Returns `Some(score)` if it matches.
/// Uses substring matching across id, label, aliases, description, and keywords.
pub(super) fn rank_command(entry: &CommandEntry, query: &str) -> Option<i32> {
    if !command_matches(entry, query) {
        return None;
    }
    // All matches get the same score — order is preserved by stable sort.
    Some(1)
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
                "/exit",
                "/fetch",
                "/status",
                "/search",
                "/gate",
                "/rules",
                "/score",
                "/memory",
                "/mem-store",
                "/mem-search",
                "/mem-list",
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

    #[test]
    fn filter_fetch_matches_new_command() {
        let ids: Vec<_> = filter_commands("fetch")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/fetch"), "fetch command should appear when searching 'fetch'");
    }

    #[test]
    fn filter_status_matches_new_command() {
        let ids: Vec<_> = filter_commands("status")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/status"), "status command should appear when searching 'status'");
    }

    #[test]
    fn filter_search_matches_new_command() {
        let ids: Vec<_> = filter_commands("search web")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/search"), "search command should appear when searching 'search web'");
    }

    #[test]
    fn filter_gate_matches() {
        let ids: Vec<_> = filter_commands("gate")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/gate"), "/gate should appear when searching 'gate'");
    }

    #[test]
    fn filter_rules_matches() {
        let ids: Vec<_> = filter_commands("rules")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/rules"), "/rules should appear when searching 'rules'");
    }

    #[test]
    fn filter_score_matches() {
        let ids: Vec<_> = filter_commands("score")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/score"), "/score should appear when searching 'score'");
    }

    #[test]
    fn filter_memory_matches() {
        let ids: Vec<_> = filter_commands("memory")
            .into_iter()
            .map(|i| COMMANDS[i].id)
            .collect();
        assert!(ids.contains(&"/memory"), "/memory should appear when searching 'memory'");
    }
}
