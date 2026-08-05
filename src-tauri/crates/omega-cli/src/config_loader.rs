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
        .and_then(|s| providers::ProviderKind::from_str(s).ok())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: write a config.json to a temp dir and return the dir.
    fn setup_config_dir(json: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.json"), json).unwrap();
        dir
    }

    // ── CliConfig serde ──────────────────────────────────────────────

    #[test]
    fn cli_config_roundtrip_all_fields() {
        let cfg = CliConfig {
            provider: Some("anthropic".into()),
            model: Some("claude-sonnet-4".into()),
            base_url: Some("https://api.example.com".into()),
            max_tokens: Some(8192),
            temperature: Some(1.0),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: CliConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider, Some("anthropic".into()));
        assert_eq!(parsed.model, Some("claude-sonnet-4".into()));
        assert_eq!(parsed.base_url, Some("https://api.example.com".into()));
        assert_eq!(parsed.max_tokens, Some(8192));
        assert_eq!(parsed.temperature, Some(1.0));
    }

    #[test]
    fn cli_config_backward_compat_missing_new_fields() {
        // Old config files without max_tokens/temperature should parse fine
        let json = r#"{"provider":"openai","model":"gpt-4o"}"#;
        let cfg: CliConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.provider, Some("openai".into()));
        assert_eq!(cfg.max_tokens, None);
        assert_eq!(cfg.temperature, None);
    }

    #[test]
    fn cli_config_empty_object() {
        let cfg: CliConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.provider, None);
        assert_eq!(cfg.max_tokens, None);
        assert_eq!(cfg.temperature, None);
    }

    // ── load_config_from_dir ─────────────────────────────────────────

    #[test]
    fn load_config_from_dir_reads_fixture() {
        let fixture = fs::read_to_string("../../tests/fixtures/full_config.json")
            .unwrap_or_else(|_| {
                // Fallback: read from crate root
                let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                fs::read_to_string(crate_dir.join("tests/fixtures/full_config.json")).unwrap()
            });
        let dir = setup_config_dir(&fixture);
        let cfg = load_config_from_dir(dir.path());
        assert_eq!(cfg.provider, Some("openai".into()));
        assert_eq!(cfg.model, Some("gpt-4o".into()));
        assert_eq!(cfg.base_url, Some("https://custom.api.example.com/v1".into()));
        assert_eq!(cfg.max_tokens, Some(8192));
        assert_eq!(cfg.temperature, Some(1.2));
    }

    #[test]
    fn load_config_from_dir_missing_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = load_config_from_dir(dir.path());
        assert_eq!(cfg.provider, None);
        assert_eq!(cfg.max_tokens, None);
        assert_eq!(cfg.temperature, None);
    }

    #[test]
    fn load_config_from_dir_invalid_json_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.json"), "not json {{{").unwrap();
        let cfg = load_config_from_dir(dir.path());
        assert_eq!(cfg.provider, None);
    }

    #[test]
    fn load_config_from_dir_partial_config() {
        let dir = setup_config_dir(r#"{"provider":"local","max_tokens":2048}"#);
        let cfg = load_config_from_dir(dir.path());
        assert_eq!(cfg.provider, Some("local".into()));
        assert_eq!(cfg.max_tokens, Some(2048));
        assert_eq!(cfg.temperature, None); // not set
    }

    // ── Provider resolution via load_provider_config ──────────────────

    #[test]
    fn provider_override_cli_overrides_config() {
        let dir = setup_config_dir(r#"{"provider":"local"}"#);
        let cfg = load_provider_config_inner(
            Some("anthropic".into()), // CLI override
            None, None, None, None,
            dir.path(),
        );
        assert!(matches!(cfg.kind, providers::ProviderKind::Anthropic));
    }

    #[test]
    fn provider_auto_detects_openai_when_env_key_set() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("OMEGA_API_KEY", "sk-test");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert!(matches!(cfg.kind, providers::ProviderKind::OpenAI));
        std::env::remove_var("OMEGA_API_KEY");
    }

    #[test]
    fn provider_falls_back_to_local_when_no_key() {
        let dir = tempfile::tempdir().unwrap();
        std::env::remove_var("OMEGA_API_KEY");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert!(matches!(cfg.kind, providers::ProviderKind::Local));
    }

    // ── Model resolution ─────────────────────────────────────────────

    #[test]
    fn model_cli_overrides_config_and_env() {
        let dir = setup_config_dir(r#"{"model":"from-config"}"#);
        std::env::set_var("OMEGA_MODEL", "from-env");
        let cfg = load_provider_config_inner(
            None, Some("from-cli".into()), None, None, None, dir.path(),
        );
        assert_eq!(cfg.model, "from-cli");
        std::env::remove_var("OMEGA_MODEL");
    }

    #[test]
    fn model_config_overrides_env() {
        let dir = setup_config_dir(r#"{"model":"from-config"}"#);
        std::env::set_var("OMEGA_MODEL", "from-env");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.model, "from-config");
        std::env::remove_var("OMEGA_MODEL");
    }

    #[test]
    fn model_env_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("OMEGA_MODEL", "from-env");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.model, "from-env");
        std::env::remove_var("OMEGA_MODEL");
    }

    #[test]
    fn model_default_per_provider() {
        let dir = tempfile::tempdir().unwrap();
        std::env::remove_var("OMEGA_MODEL");
        let cfg = load_provider_config_inner(
            Some("anthropic".into()), None, None, None, None, dir.path(),
        );
        assert_eq!(cfg.model, "claude-sonnet-4-20250514");
    }

    // ── max_tokens override chain ────────────────────────────────────

    #[test]
    fn max_tokens_cli_overrides_all() {
        let dir = setup_config_dir(r#"{"max_tokens":2048}"#);
        std::env::set_var("OMEGA_MAX_TOKENS", "1024");
        let cfg = load_provider_config_inner(
            None, None, None, Some(16384), None, dir.path(),
        );
        assert_eq!(cfg.max_tokens, 16384);
        std::env::remove_var("OMEGA_MAX_TOKENS");
    }

    #[test]
    fn max_tokens_config_overrides_env() {
        let dir = setup_config_dir(r#"{"max_tokens":8192}"#);
        std::env::set_var("OMEGA_MAX_TOKENS", "1024");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.max_tokens, 8192);
        std::env::remove_var("OMEGA_MAX_TOKENS");
    }

    #[test]
    fn max_tokens_env_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("OMEGA_MAX_TOKENS", "512");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.max_tokens, 512);
        std::env::remove_var("OMEGA_MAX_TOKENS");
    }

    #[test]
    fn max_tokens_default_when_unset() {
        let dir = tempfile::tempdir().unwrap();
        std::env::remove_var("OMEGA_MAX_TOKENS");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.max_tokens, 4096);
    }

    // ── temperature override chain ───────────────────────────────────

    #[test]
    fn temperature_cli_overrides_all() {
        let dir = setup_config_dir(r#"{"temperature":0.3}"#);
        std::env::set_var("OMEGA_TEMPERATURE", "0.5");
        let cfg = load_provider_config_inner(
            None, None, None, None, Some(1.5), dir.path(),
        );
        assert_eq!(cfg.temperature, 1.5);
        std::env::remove_var("OMEGA_TEMPERATURE");
    }

    #[test]
    fn temperature_config_overrides_env() {
        let dir = setup_config_dir(r#"{"temperature":0.9}"#);
        std::env::set_var("OMEGA_TEMPERATURE", "0.5");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.temperature, 0.9);
        std::env::remove_var("OMEGA_TEMPERATURE");
    }

    #[test]
    fn temperature_env_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("OMEGA_TEMPERATURE", "1.8");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.temperature, 1.8);
        std::env::remove_var("OMEGA_TEMPERATURE");
    }

    #[test]
    fn temperature_default_when_unset() {
        let dir = tempfile::tempdir().unwrap();
        std::env::remove_var("OMEGA_TEMPERATURE");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.temperature, 0.7);
    }

    // ── API key loading ──────────────────────────────────────────────

    #[test]
    fn api_key_from_env() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("OMEGA_API_KEY", "sk-env-key");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.api_key.as_deref(), Some("sk-env-key"));
        std::env::remove_var("OMEGA_API_KEY");
    }

    #[test]
    fn api_key_from_dotenv_file() {
        let dir = tempfile::tempdir().unwrap();
        std::env::remove_var("OMEGA_API_KEY");
        fs::write(dir.path().join(".env"), "sk-file-key\n").unwrap();
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.api_key.as_deref(), Some("sk-file-key"));
    }

    #[test]
    fn api_key_env_overrides_dotenv() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("OMEGA_API_KEY", "sk-env-key");
        fs::write(dir.path().join(".env"), "sk-file-key\n").unwrap();
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.api_key.as_deref(), Some("sk-env-key"));
        std::env::remove_var("OMEGA_API_KEY");
    }

    #[test]
    fn api_key_none_when_nothing_set() {
        let dir = tempfile::tempdir().unwrap();
        std::env::remove_var("OMEGA_API_KEY");
        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert_eq!(cfg.api_key, None);
    }

    // ── Full integration: all overrides at once ──────────────────────

    #[test]
    fn full_override_chain() {
        let dir = setup_config_dir(
            r#"{"provider":"google","model":"gemini-pro","max_tokens":2048,"temperature":0.4}"#,
        );
        std::env::set_var("OMEGA_MODEL", "gemini-flash");
        std::env::set_var("OMEGA_MAX_TOKENS", "1024");
        std::env::set_var("OMEGA_TEMPERATURE", "0.9");

        // CLI overrides beat everything
        let cfg = load_provider_config_inner(
            Some("openai".into()),
            Some("gpt-4o".into()),
            Some("https://custom.api.test".into()),
            Some(16384),
            Some(1.2),
            dir.path(),
        );
        assert!(matches!(cfg.kind, providers::ProviderKind::OpenAI));
        assert_eq!(cfg.model, "gpt-4o");
        assert_eq!(cfg.base_url.as_deref(), Some("https://custom.api.test"));
        assert_eq!(cfg.max_tokens, 16384);
        assert_eq!(cfg.temperature, 1.2);

        std::env::remove_var("OMEGA_MODEL");
        std::env::remove_var("OMEGA_MAX_TOKENS");
        std::env::remove_var("OMEGA_TEMPERATURE");
    }

    #[test]
    fn config_used_when_no_cli_overrides() {
        let dir = setup_config_dir(
            r#"{"provider":"groq","model":"llama3-70b","max_tokens":4096,"temperature":0.8}"#,
        );
        std::env::remove_var("OMEGA_MODEL");
        std::env::remove_var("OMEGA_MAX_TOKENS");
        std::env::remove_var("OMEGA_TEMPERATURE");

        let cfg = load_provider_config_inner(None, None, None, None, None, dir.path());
        assert!(matches!(cfg.kind, providers::ProviderKind::Groq));
        assert_eq!(cfg.model, "llama3-70b");
        assert_eq!(cfg.max_tokens, 4096);
        assert_eq!(cfg.temperature, 0.8);
    }
}
