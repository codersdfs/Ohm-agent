//! Configuration for the loader — the contract between today's loader and
//! tomorrow's settings panel.
//!
//! Written to `~/.config/omega-agent/config.json` under the `"loader"` key:
//!
//! ```json
//! {
//!   "loader": {
//!     "style": "shimmer",
//!     "tick_ms": 80,
//!     "anchor": "status_line",
//!     "phrases": null
//!   }
//! }
//! ```
//!
//! Unknown values silently fall back to the default rather than refusing
//! to start — config files outlive code versions and never crashing is
//! more important than honoring a typo strictly.

use serde::{Deserialize, Serialize};

/// Built-in renderers registered by default.
pub const STYLE_BRAILLE: &str = "braille";
pub const STYLE_SHIMMER: &str = "shimmer";

/// Where the loader is drawn. Only `StatusLine` is implemented today.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoaderAnchor {
    #[default]
    StatusLine,
}

/// Loader-specific configuration. All fields are optional; missing keys
/// fall back to the defaults defined by `LoaderConfig::default()`.
///
/// `phrases` is intentionally a flat `Vec<String>` rather than per-state
/// tables for now — the default renderers ship their own phrase banks and
/// override only takes effect when non-empty.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderConfig {
    /// Renderer style name. Built-ins: `"shimmer"` (default), `"braille"`.
    pub style: Option<String>,
    /// Animation tick interval in milliseconds. Defaults to 80ms.
    #[serde(default)]
    pub tick_ms: Option<u64>,
    /// On-screen anchor. Only `"status_line"` is implemented today.
    #[serde(default)]
    pub anchor: Option<LoaderAnchor>,
    /// Optional override phrase bank. When `Some(non_empty)`, replaces the
    /// built-in phrases; when `None` or empty, renderers use defaults.
    #[serde(default)]
    pub phrases: Option<Vec<String>>,
}

impl LoaderConfig {
    /// Resolve the configured style to a known built-in, falling back to
    /// `shimmer` when missing or unrecognised. Returns the canonical name.
    pub fn resolved_style(&self) -> &'static str {
        match self.style.as_deref() {
            Some(STYLE_BRAILLE) => STYLE_BRAILLE,
            Some(STYLE_SHIMMER) => STYLE_SHIMMER,
            _ => STYLE_SHIMMER,
        }
    }

    /// Effective tick interval clamped to a sane range (40–250ms).
    pub fn resolved_tick_ms(&self) -> u64 {
        match self.tick_ms {
            Some(ms) if (40..=250).contains(&ms) => ms,
            _ => 80,
        }
    }

    /// Effective anchor, always `StatusLine` today.
    pub fn resolved_anchor(&self) -> LoaderAnchor {
        self.anchor.unwrap_or(LoaderAnchor::StatusLine)
    }

    /// Effective override phrases, returning `None` when empty/absent.
    pub fn resolved_phrases(&self) -> Option<Vec<String>> {
        self.phrases.as_ref().filter(|v| !v.is_empty()).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_config_default_resolves_to_shimmer() {
        let cfg = LoaderConfig::default();
        assert_eq!(cfg.resolved_style(), STYLE_SHIMMER);
        assert_eq!(cfg.resolved_tick_ms(), 80);
        assert_eq!(cfg.resolved_anchor(), LoaderAnchor::StatusLine);
        assert!(cfg.resolved_phrases().is_none());
    }

    #[test]
    fn test_loader_config_unknown_style_falls_back_to_shimmer() {
        let cfg = LoaderConfig {
            style: Some("rainbow_pulse".to_string()),
            tick_ms: None,
            anchor: None,
            phrases: None,
        };
        assert_eq!(cfg.resolved_style(), STYLE_SHIMMER);
    }

    #[test]
    fn test_loader_config_braille_is_recognised() {
        let cfg = LoaderConfig {
            style: Some(STYLE_BRAILLE.to_string()),
            tick_ms: None,
            anchor: None,
            phrases: None,
        };
        assert_eq!(cfg.resolved_style(), STYLE_BRAILLE);
    }

    #[test]
    fn test_loader_config_tick_ms_out_of_range_falls_back() {
        for ms in [20u64, 2000] {
            let cfg = LoaderConfig {
                style: None,
                tick_ms: Some(ms),
                anchor: None,
                phrases: None,
            };
            assert_eq!(cfg.resolved_tick_ms(), 80);
        }
        let ok = LoaderConfig {
            style: None,
            tick_ms: Some(120),
            anchor: None,
            phrases: None,
        };
        assert_eq!(ok.resolved_tick_ms(), 120);
    }

    #[test]
    fn test_loader_config_empty_phrases_treated_as_none() {
        let cfg = LoaderConfig {
            style: None,
            tick_ms: None,
            anchor: None,
            phrases: Some(vec![]),
        };
        assert!(cfg.resolved_phrases().is_none());
    }

    #[test]
    fn test_loader_config_override_phrases_preserved() {
        let cfg = LoaderConfig {
            style: None,
            tick_ms: None,
            anchor: None,
            phrases: Some(vec!["Hacking…".into()]),
        };
        assert_eq!(cfg.resolved_phrases(), Some(vec!["Hacking…".to_string()]));
    }

    #[test]
    fn test_loader_config_deserialises_minimal_json() {
        let cfg: LoaderConfig = serde_json::from_str(r#"{ "style": "braille" }"#).unwrap();
        assert_eq!(cfg.resolved_style(), STYLE_BRAILLE);
        assert_eq!(cfg.resolved_tick_ms(), 80);
        assert_eq!(cfg.resolved_anchor(), LoaderAnchor::StatusLine);
    }

    #[test]
    fn test_loader_config_deserialises_full_json() {
        let json = r#"{"style":"shimmer","tick_ms":100,"anchor":"status_line",
            "phrases":["Thinking…","Working…"]}"#;
        let cfg: LoaderConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.resolved_style(), STYLE_SHIMMER);
        assert_eq!(cfg.resolved_tick_ms(), 100);
        assert_eq!(cfg.resolved_anchor(), LoaderAnchor::StatusLine);
        assert_eq!(cfg.resolved_phrases().unwrap().len(), 2);
    }

    #[test]
    fn test_loader_config_missing_section_deserialises_default() {
        let cfg: LoaderConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.resolved_style(), STYLE_SHIMMER);
    }
}
