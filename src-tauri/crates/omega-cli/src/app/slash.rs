//! Slash-command dispatch (P5 split from main.rs).

use super::App;
use omega_core::commands;
use omega_core::commands::web;
use omega_core::tui::command_palette::CommandHandler;
use omega_core::tui::provider_panel::{ProviderPanelState, WizardStep};

impl App {
    /// Handle a slash command.
    /// Uses the unified `CommandHandler` dispatch from `command_palette::lookup_command`.
    pub fn handle_slash_command(&mut self, cmd: &str) {
        let entry = omega_core::tui::command_palette::lookup_command(cmd);
        let handler = match entry {
            Some(e) => e.handler,
            None => {
                self.transcript.add_notice(
                    format!("Unknown command: {}. Type /help for commands.", cmd),
                    true,
                );
                return;
            }
        };

        match handler {
            CommandHandler::Help => {
                self.transcript.add_notice("Commands: /help, /clear, /tools, /model, /provider, /cost, /exit, /fetch, /status, /search, /gate, /rules, /score, /memory".into(), false);
            }
            CommandHandler::Clear => {
                self.transcript.clear_transcript();
                self.editor.buffer.clear();
                match self.state.clear_session() {
                    Ok(()) => {
                        self.transcript.add_notice("Session cleared.".into(), false);
                    }
                    Err(e) => {
                        log::error!("session clear failed: {e}");
                        self.transcript
                            .add_notice(format!("Failed to clear session file: {e}"), true);
                    }
                }
            }
            CommandHandler::Tools => match commands::tools::list_tools() {
                Ok(tools) => {
                    let list = tools.join(", ");
                    self.transcript
                        .add_notice(format!("Available tools: {}", list), false);
                }
                Err(e) => {
                    self.transcript
                        .add_notice(format!("Error listing tools: {}", e), true);
                }
            },
            CommandHandler::Model => {
                if self.is_streaming {
                    self.transcript
                        .add_notice("Can't open provider panel while streaming.".into(), true);
                } else {
                    // Model-first: jump straight to model picker for current provider.
                    self.provider_panel_state =
                        ProviderPanelState::from_config_at(&self.config, WizardStep::Model);
                    self.show_provider_panel = true;
                }
            }
            CommandHandler::Provider => {
                if self.is_streaming {
                    self.transcript
                        .add_notice("Can't open provider panel while streaming.".into(), true);
                } else {
                    // Provider list is step 1 — show that when user asks for providers.
                    self.provider_panel_state = ProviderPanelState::from_config(&self.config);
                    self.show_provider_panel = true;
                }
            }
            CommandHandler::Cost => {
                self.transcript.add_notice(
                    format!(
                        "Session tokens — {} in / {} out ({} messages)",
                        self.session_tokens_in, self.session_tokens_out, self.session_messages
                    ),
                    false,
                );
            }
            CommandHandler::Exit => {
                self.should_quit = true;
            }
            CommandHandler::Fetch => {
                // Parse URL from the command (everything after /fetch )
                let url = cmd
                    .trim_start_matches("/fetch")
                    .trim_start_matches("/web")
                    .trim_start_matches("/url")
                    .trim();
                if url.is_empty() {
                    self.transcript.add_notice(
                        "Usage: /fetch <url> — e.g. /fetch https://example.com".into(),
                        true,
                    );
                    return;
                }

                self.transcript
                    .add_notice(format!("Fetching {url} …"), false);

                // Use `omega_core::commands::web::fetch_url` to fetch the URL content.
                // `handle_slash_command` is sync (called from the TUI event loop),
                // so use tokio's block_in_place + Handle::current().block_on() to
                // run the async HTTP call synchronously — the UI freezes briefly
                // but this is acceptable for explicit user-invoked commands.
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(web::fetch_url(url))
                });
                match result {
                    Ok(body) => {
                        let display = if body.len() > 2000 {
                            let d: String = body.chars().take(2000).collect();
                            format!("{d}\n\n[... truncated]")
                        } else {
                            body
                        };
                        self.transcript
                            .add_notice(format!("Content from {url}:\n{display}"), false);
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Error fetching {url}: {e}"), true);
                    }
                }
            }
            CommandHandler::Status => {
                self.transcript
                    .add_notice("Checking connectivity …".into(), false);
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(web::check_status())
                });
                match result {
                    Ok(status) => {
                        let internet = status["internet_reachable"].as_bool().unwrap_or(false);
                        let endpoints = status["provider_endpoints"]
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| {
                                        let ok = v.as_bool().unwrap_or(false);
                                        format!(
                                            "  {k}: {}",
                                            if ok {
                                                "✓ reachable"
                                            } else {
                                                "✗ unreachable"
                                            }
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_default();
                        self.transcript.add_notice(
                            format!(
                                "Status:\n  Internet: {}\n{}",
                                if internet {
                                    "✓ connected"
                                } else {
                                    "✗ disconnected"
                                },
                                endpoints
                            ),
                            false,
                        );
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Status check failed: {e}"), true);
                    }
                }
            }
            CommandHandler::Search => {
                let query = cmd
                    .trim_start_matches("/search")
                    .trim_start_matches("/google")
                    .trim_start_matches("/websearch")
                    .trim();
                if query.is_empty() {
                    self.transcript.add_notice(
                        "Usage: /search <query> — e.g. /search rust async programming".into(),
                        true,
                    );
                    return;
                }
                self.transcript
                    .add_notice(format!("Searching for \"{query}\" …"), false);
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(web::search_web(query))
                });
                match result {
                    Ok(results) => {
                        if results.is_empty() {
                            self.transcript
                                .add_notice(format!("No results found for \"{query}\""), true);
                        } else {
                            let mut lines =
                                format!("Search results for \"{query}\":\n").to_string();
                            for (i, r) in results.iter().enumerate() {
                                lines.push_str(&format!(
                                    "\n{}. {} — {}\n   {}",
                                    i + 1,
                                    r.title,
                                    r.url,
                                    r.snippet
                                ));
                            }
                            self.transcript.add_notice(lines, false);
                        }
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Search failed: {e}"), true);
                    }
                }
            }
            CommandHandler::Gate => {
                let (path, content) = match self.read_file_or_buffer(cmd, "/gate") {
                    Ok(v) => v,
                    Err(e) => {
                        self.transcript.add_notice(e, true);
                        return;
                    }
                };
                self.transcript
                    .add_notice(format!("Running gate on {path}…"), false);
                let request = commands::gate::GateCheckRequest {
                    content,
                    context: path.to_string(),
                    language: None,
                };
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(commands::gate::check_gate(&self.state, request))
                });
                match result {
                    Ok(g) => {
                        let status = if g.passed { "PASSED" } else { "FAILED" };
                        let mut lines = format!("Gate {path}: {}/100 — {status}\n", g.score);
                        if g.violations.is_empty() {
                            lines.push_str("No violations");
                        } else {
                            lines.push_str(&format!("{} violation(s):\n", g.violations.len()));
                            for v in &g.violations {
                                let line = v.line.map(|l| format!(" L{l}")).unwrap_or_default();
                                lines.push_str(&format!(
                                    "  [{}]{}: {}\n",
                                    v.category, line, v.message
                                ));
                            }
                        }
                        self.transcript.add_notice(lines, false);
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Gate failed: {e}"), true);
                    }
                }
            }
            CommandHandler::Rules => {
                self.transcript.add_notice("Loading rules…".into(), false);
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(commands::gate::get_rules(&self.state))
                });
                match result {
                    Ok(rules) => {
                        if rules.is_empty() {
                            self.transcript
                                .add_notice("No promoted rules yet".into(), false);
                        } else {
                            let header = format!("Promoted rules ({} total):\n", rules.len());
                            let lines = rules
                                .iter()
                                .map(|r| format!("  {r}"))
                                .collect::<Vec<_>>()
                                .join("\n");
                            self.transcript
                                .add_notice(format!("{header}{lines}"), false);
                        }
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Failed to load rules: {e}"), true);
                    }
                }
            }
            CommandHandler::Score => {
                let (path, content) = match self.read_file_or_buffer(cmd, "/score") {
                    Ok(v) => v,
                    Err(e) => {
                        self.transcript.add_notice(e, true);
                        return;
                    }
                };
                let request = commands::gate::GateCheckRequest {
                    content,
                    context: path.to_string(),
                    language: None,
                };
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(commands::gate::check_gate(&self.state, request))
                });
                match result {
                    Ok(g) => {
                        let status = if g.passed { "PASSED" } else { "FAILED" };
                        self.transcript.add_notice(
                            format!(
                                "Score: {}/100 — {} ({} violations)",
                                g.score,
                                status,
                                g.violations.len()
                            ),
                            !g.passed,
                        );
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Score failed: {e}"), true);
                    }
                }
            }
            CommandHandler::MemStore => {
                let parts: Vec<&str> = cmd.splitn(3, ' ').collect();
                if parts.len() < 3 {
                    self.transcript
                        .add_notice("Usage: /mem-store <key> <value>".into(), true);
                    return;
                }
                let key = parts[1].to_string();
                let value = parts[2].to_string();
                let state = self.state.clone();
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(commands::memory::memory_store_project(&state, key, value))
                });
                match result {
                    Ok(_) => {
                        self.transcript.add_notice("Memory stored.".into(), false);
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Failed to store memory: {e}"), true);
                    }
                }
            }
            CommandHandler::MemSearch => {
                let query = cmd
                    .trim_start_matches("/mem-search")
                    .trim_start_matches("/memfind")
                    .trim();
                if query.is_empty() {
                    self.transcript
                        .add_notice("Usage: /mem-search <query>".into(), true);
                    return;
                }
                self.transcript
                    .add_notice(format!("Searching project memory for \"{query}\"…"), false);
                let state = self.state.clone();
                let query_owned = query.to_string();
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(commands::memory::memory_search_project(&state, query_owned))
                });
                match result {
                    Ok(response) => {
                        if response.entries.is_empty() {
                            self.transcript
                                .add_notice(format!("No results found for \"{query}\""), false);
                        } else {
                            let mut lines =
                                format!("Project memory results ({}):\n", response.entries.len());
                            for (i, entry) in response.entries.iter().enumerate() {
                                let rel = response
                                    .relevance
                                    .get(i)
                                    .map(|r| format!(" [{:.2}]", r))
                                    .unwrap_or_default();
                                lines.push_str(&format!(
                                    "  {} — {}{}\n",
                                    entry.key,
                                    entry.value.chars().take(80).collect::<String>(),
                                    rel
                                ));
                            }
                            self.transcript.add_notice(lines, false);
                        }
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Memory search failed: {e}"), true);
                    }
                }
            }
            CommandHandler::MemList => {
                self.transcript
                    .add_notice("Listing project memories…".into(), false);
                let state = self.state.clone();
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(commands::memory::memory_list_project(&state))
                });
                match result {
                    Ok(entries) => {
                        if entries.is_empty() {
                            self.transcript
                                .add_notice("No project memories stored.".into(), false);
                        } else {
                            let mut lines = format!("Project memories ({}):\n", entries.len());
                            for entry in &entries {
                                lines.push_str(&format!(
                                    "  {} — {}\n",
                                    entry.key,
                                    entry.value.chars().take(80).collect::<String>()
                                ));
                            }
                            self.transcript.add_notice(lines, false);
                        }
                    }
                    Err(e) => {
                        self.transcript
                            .add_notice(format!("Failed to list memories: {e}"), true);
                    }
                }
            }
            CommandHandler::Memory => {
                let query = cmd
                    .trim_start_matches("/memory")
                    .trim_start_matches("/mem")
                    .trim();
                if query.is_empty() {
                    // Show memory stats
                    let count_session = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(commands::memory::memory_count(
                            &self.state,
                            Some("session".into()),
                        ))
                    });
                    let count_project = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(commands::memory::memory_count(
                            &self.state,
                            Some("project".into()),
                        ))
                    });
                    let count_user = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(commands::memory::memory_count(
                            &self.state,
                            Some("user".into()),
                        ))
                    });
                    let s = count_session.unwrap_or(0);
                    let p = count_project.unwrap_or(0);
                    let u = count_user.unwrap_or(0);
                    self.transcript.add_notice(format!("Memory stats:\n  session: {s} entries\n  project: {p} entries\n  user:    {u} entries"), false);
                } else {
                    self.transcript
                        .add_notice(format!("Searching memory for \"{query}\"…"), false);
                    let request = commands::memory::MemorySearchRequest {
                        query: query.to_string(),
                        layer: None,
                        limit: Some(10),
                    };
                    let result = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current()
                            .block_on(commands::memory::memory_search(&self.state, request))
                    });
                    match result {
                        Ok(response) => {
                            if response.entries.is_empty() {
                                self.transcript
                                    .add_notice(format!("No results found for \"{query}\""), false);
                            } else {
                                let mut lines =
                                    format!("Memory results ({}):\n", response.entries.len());
                                for (i, entry) in response.entries.iter().enumerate() {
                                    let rel = response
                                        .relevance
                                        .get(i)
                                        .map(|r| format!(" [{:.2}]", r))
                                        .unwrap_or_default();
                                    lines.push_str(&format!(
                                        "  [{}] {} — {}{}\n",
                                        entry.layer.as_str(),
                                        entry.key,
                                        entry.value.chars().take(80).collect::<String>(),
                                        rel
                                    ));
                                }
                                self.transcript.add_notice(lines, false);
                            }
                        }
                        Err(e) => {
                            self.transcript
                                .add_notice(format!("Memory search failed: {e}"), true);
                        }
                    }
                }
            }
        }
    }
}
