use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct GateConfig {
    #[serde(default = "default_true")]
    pub clippy_enabled: bool,
    #[serde(default = "default_true")]
    pub eslint_enabled: bool,
    #[serde(default = "default_true")]
    pub tsc_enabled: bool,
    #[serde(default)]
    pub ruff_enabled: bool,
    #[serde(default)]
    pub thresholds: HashMap<String, u32>,
}

fn default_true() -> bool {
    true
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            clippy_enabled: true,
            eslint_enabled: true,
            tsc_enabled: true,
            ruff_enabled: false,
            thresholds: HashMap::new(),
        }
    }
}

pub fn load_gate_config(project_root: &str) -> GateConfig {
    let config_path = std::path::Path::new(project_root).join(".omega/gate.toml");
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        match toml::from_str::<GateConfig>(&content) {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Failed to parse gate config: {}", e);
                GateConfig::default()
            }
        }
    } else {
        GateConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_clippy_enabled() {
        let config = GateConfig::default();
        assert!(config.clippy_enabled);
        assert!(config.eslint_enabled);
        assert!(config.tsc_enabled);
        assert!(!config.ruff_enabled);
    }

    #[test]
    fn load_config_from_file() {
        let dir = std::env::temp_dir().join("omega_test_gate_config");
        let _ = std::fs::create_dir_all(dir.join(".omega"));
        std::fs::write(
            dir.join(".omega/gate.toml"),
            r#"
clippy_enabled = false
eslint_enabled = true
tsc_enabled = false
ruff_enabled = true
"#,
        )
        .unwrap();
        let config = load_gate_config(dir.to_str().unwrap());
        assert!(!config.clippy_enabled);
        assert!(config.eslint_enabled);
        assert!(!config.tsc_enabled);
        assert!(config.ruff_enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_missing_file_returns_default() {
        let config = load_gate_config("/nonexistent/path");
        assert!(config.clippy_enabled);
    }
}
