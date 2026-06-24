mod gate;
mod mcp;

use crate::app::App;

pub fn handle_command(app: &mut App, cmd: &str) {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    let command = parts[0];
    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match command {
        "/help" => cmd_help(app),
        "/clear" => {
            app.history.clear();
            app.history
                .push(crate::message::Message::system("Conversation cleared."));
        }
        "/exit" | "/quit" => {
            app.should_quit = true;
        }
        "/provider" => cmd_provider(app, args),
        "/model" => cmd_model(app, args),
        "/api-key" => cmd_api_key(app, args),
        "/base-url" => cmd_base_url(app, args),
        "/config" => cmd_config(app),
        "/connect" => mcp::cmd_connect(app, args),
        "/skills" => mcp::cmd_skills(app),
        "/gate" => gate::cmd_gate(app, args),
        "/gate-score" => gate::cmd_gate_score(app, args),
        _ => {
            app.history
                .push(crate::message::Message::system(format!("Unknown command: {cmd}")));
        }
    }
    app.clear_input();
}

fn cmd_help(app: &mut App) {
    let help = "\
/ help          — show this message
/ provider [name] — show or set provider (mock, anthropic, openai, local, ...)
/ model [name]    — show or set model name
/ api-key <key>   — set API key
/ base-url [url]  — show or set base URL
/ config          — print current configuration
/ connect <url>   — connect to MCP server
/ skills          — list registered MCP skills
/ gate <path>     — run Gate evaluation on file
/ gate-score <path> — show Gate score only
/ clear           — clear conversation
/ exit            — quit";
    app.history
        .push(crate::message::Message::system(help));
}

fn cmd_provider(app: &mut App, args: &str) {
    if args.is_empty() {
        match &app.provider_config {
            Some(cfg) => {
                app.history.push(crate::message::Message::system(format!(
                    "Current provider: {}",
                    cfg.kind
                )));
            }
            None => {
                app.history
                    .push(crate::message::Message::system("Current provider: mock"));
            }
        }
        return;
    }

    let kind = providers::ProviderKind::from_str(args);

    if matches!(kind, providers::ProviderKind::Local) {
        app.switch_to_mock();
        app.history
            .push(crate::message::Message::system("Provider set to: mock"));
        return;
    }

    let api_key = app
        .provider_config
        .as_ref()
        .and_then(|c| c.api_key.clone());

    if api_key.is_none() {
        app.history.push(crate::message::Message::system(format!(
            "Provider set to: {kind}. Use /api-key <key> to set the API key."
        )));
        return;
    }

    let base_url = app
        .provider_config
        .as_ref()
        .and_then(|c| c.base_url.clone());
    let model = app
        .provider_config
        .as_ref()
        .map(|c| c.model.clone())
        .unwrap_or_else(|| default_model(&kind));

    let config = providers::ProviderConfig {
        kind: kind.clone(),
        api_key,
        base_url,
        model,
        max_tokens: 4096,
        temperature: 0.7,
    };

    app.switch_provider(config);
    app.history
        .push(crate::message::Message::system(format!("Provider set to: {kind}")));
}

fn cmd_model(app: &mut App, args: &str) {
    if args.is_empty() {
        match &app.provider_config {
            Some(cfg) => {
                app.history.push(crate::message::Message::system(format!(
                    "Current model: {}",
                    cfg.model
                )));
            }
            None => {
                app.history
                    .push(crate::message::Message::system("Current model: (mock mode)"));
            }
        }
        return;
    }

    if let Some(cfg) = app.provider_config.clone() {
        let mut config = cfg;
        config.model = args.to_string();
        app.switch_provider(config);
        app.history
            .push(crate::message::Message::system(format!("Model set to: {args}")));
    } else {
        app.history.push(crate::message::Message::system(
            "Set a provider first with /provider <name>.",
        ));
    }
}

fn cmd_api_key(app: &mut App, args: &str) {
    if args.is_empty() {
        app.history
            .push(crate::message::Message::system("Usage: /api-key <key>"));
        return;
    }

    let masked = if args.len() > 4 {
        format!("{}****{}", &args[..2], &args[args.len() - 4..])
    } else {
        "****".to_string()
    };

    if let Some(cfg) = app.provider_config.clone() {
        let mut config = cfg;
        config.api_key = Some(args.to_string());
        app.switch_provider(config);
        app.history
            .push(crate::message::Message::system(format!("API key set to: {masked}")));
    } else {
        let kind = providers::ProviderKind::OpenAI;
        let config = providers::ProviderConfig {
            kind: kind.clone(),
            api_key: Some(args.to_string()),
            base_url: None,
            model: default_model(&kind),
            max_tokens: 4096,
            temperature: 0.7,
        };
        app.switch_provider(config);
        app.history.push(crate::message::Message::system(format!(
            "API key set to: {masked}. Provider: {kind}."
        )));
    }
}

fn cmd_base_url(app: &mut App, args: &str) {
    if args.is_empty() {
        match &app.provider_config {
            Some(cfg) => match &cfg.base_url {
                Some(url) => {
                    app.history.push(crate::message::Message::system(format!(
                        "Current base URL: {url}"
                    )));
                }
                None => {
                    app.history.push(crate::message::Message::system(
                        "Current base URL: (default)",
                    ));
                }
            },
            None => {
                app.history
                    .push(crate::message::Message::system("No provider configured."));
            }
        }
        return;
    }

    if let Some(cfg) = app.provider_config.clone() {
        let mut config = cfg;
        config.base_url = Some(args.to_string());
        app.switch_provider(config);
        app.history
            .push(crate::message::Message::system(format!("Base URL set to: {args}")));
    } else {
        app.history.push(crate::message::Message::system(
            "Set a provider first with /provider <name>.",
        ));
    }
}

fn cmd_config(app: &mut App) {
    match &app.provider_config {
        Some(cfg) => {
            let masked_key = cfg.api_key.as_ref().map(|k| {
                if k.len() > 4 {
                    format!("{}****{}", &k[..2], &k[k.len() - 4..])
                } else {
                    "****".to_string()
                }
            });
            let text = format!(
                "Provider:  {}
Model:     {}
API key:   {}
Base URL:  {}
Max tokens: {}
Temperature: {}",
                cfg.kind,
                cfg.model,
                masked_key.as_deref().unwrap_or("(none)"),
                cfg.base_url.as_deref().unwrap_or("(default)"),
                cfg.max_tokens,
                cfg.temperature,
            );
            app.history
                .push(crate::message::Message::system(text));
        }
        None => {
            app.history
                .push(crate::message::Message::system("Provider: mock (no config)"));
        }
    }
}

fn default_model(kind: &providers::ProviderKind) -> String {
    match kind {
        providers::ProviderKind::Anthropic => "claude-sonnet-4-20250514".into(),
        providers::ProviderKind::OpenAI => "gpt-4o".into(),
        providers::ProviderKind::Google => "gemini-pro".into(),
        providers::ProviderKind::Local => "llama3.1:8b".into(),
        _ => "default".into(),
    }
}
