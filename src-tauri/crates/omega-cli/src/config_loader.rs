//! Config loading + validation (P5 split from main.rs).

use providers::ProviderConfig;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CliConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

pub fn config_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("com", "omega", "omega-agent")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

pub fn load_config() -> CliConfig {
    let path = config_dir().join("config.json");
    let _ = std::fs::create_dir_all(config_dir());
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(CliConfig {
            provider: None,
            model: None,
            base_url: None,
            max_tokens: None,
            temperature: None,
        })
}

pub fn save_config(config: &providers::ProviderConfig) {
    let cli = CliConfig {
        provider: Some(config.kind.to_string()),
        model: Some(config.model.clone()),
        base_url: config.base_url.clone(),
        max_tokens: Some(config.max_tokens),
        temperature: Some(config.temperature),
    };
    let path = config_dir().join("config.json");
    if let Ok(json) = serde_json::to_string_pretty(&cli) {
        let _ = std::fs::write(&path, json);
    }
}

/// Persist API key to `~/.config/omega-agent/.env` (plain key body).
/// Empty / None removes the file so load falls back to env-only.
pub fn save_api_key(api_key: Option<&str>) {
    let path = config_dir().join(".env");
    let _ = std::fs::create_dir_all(config_dir());
    match api_key.map(str::trim).filter(|s| !s.is_empty()) {
        Some(key) => {
            let _ = std::fs::write(&path, format!("{key}\n"));
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

pub fn load_provider_config(
    override_provider: Option<String>,
    override_model: Option<String>,
    override_base_url: Option<String>,
    override_max_tokens: Option<u32>,
    override_temperature: Option<f32>,
) -> ProviderConfig {
    let mut cli_cfg = load_config();

    // Apply CLI overrides on top of config file
    if let Some(p) = override_provider {
        cli_cfg.provider = Some(p);
    }
    if let Some(m) = override_model {
        cli_cfg.model = Some(m);
    }
    if let Some(b) = override_base_url {
        cli_cfg.base_url = Some(b);
    }

    // Resolve provider kind
    let kind = cli_cfg
        .provider
        .as_deref()
        .map(providers::ProviderKind::from_str)
        .unwrap_or_else(|| {
            // Auto-detect: if API key is set, use OpenAI; otherwise Local (Ollama)
            let has_api_key =
                std::env::var("OMEGA_API_KEY").is_ok() || config_dir().join(".env").exists();
            if has_api_key {
                providers::ProviderKind::OpenAI
            } else {
                providers::ProviderKind::Local
            }
        });

    // Resolve model
    let model = cli_cfg
        .model
        .or_else(|| std::env::var("OMEGA_MODEL").ok())
        .unwrap_or_else(|| match kind {
            providers::ProviderKind::OpenAI => "gpt-4o-mini".into(),
            providers::ProviderKind::Anthropic => "claude-sonnet-4-20250514".into(),
            providers::ProviderKind::Google => "gemini-2.0-flash".into(),
            providers::ProviderKind::Local => "llama3.1:8b".into(),
            providers::ProviderKind::Custom => "custom-model".into(),
            _ => "gpt-4o-mini".into(),
        });

    // Resolve base URL
    let base_url = cli_cfg
        .base_url
        .or_else(|| std::env::var("OMEGA_BASE_URL").ok());
    // Resolve max_tokens: CLI flag → config file → env var → hardcoded default
    let max_tokens = override_max_tokens
        .or(cli_cfg.max_tokens)
        .or_else(|| {
            std::env::var("OMEGA_MAX_TOKENS")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(4096);

    // Resolve temperature: CLI flag → config file → env var → hardcoded default
    let temperature = override_temperature
        .or(cli_cfg.temperature)
        .or_else(|| {
            std::env::var("OMEGA_TEMPERATURE")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0.7);

    // Resolve API key
    let api_key = std::env::var("OMEGA_API_KEY").ok().or_else(|| {
        let p = config_dir().join(".env");
        std::fs::read_to_string(&p)
            .ok()
            .map(|s| s.trim().to_string())
    });

    ProviderConfig {
        kind,
        api_key,
        base_url,
        model,
        max_tokens,
        temperature,
    }
}

/// Read `config.json` from a specific directory (for testability).
fn load_config_from_dir(cfg_dir: &std::path::Path) -> CliConfig {
    let path = cfg_dir.join("config.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(CliConfig {
            provider: None,
            model: None,
            base_url: None,
            max_tokens: None,
            temperature: None,
        })
}
