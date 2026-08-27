//! Loader registry — owns every built-in renderer and exposes exactly one
//! active renderer selected by [`LoaderConfig`].
//!
//! This is the write-target a future settings panel uses: swap the active
//! style by writing `"loader": { "style": "..." }` in config.json. Unknown
//! style names resolve to shimmer here (belt) *and* at `resolved_style`
//! (braces), so a hand-edited config can never break startup.

use ratatui::text::Span;

use super::braille::BrailleRenderer;
use super::config::{LoaderConfig, STYLE_BRAILLE};
use super::info::LoaderInfo;
use super::renderer::LoaderRenderer;
use super::shimmer::ShimmerRenderer;

/// Holds all built-in renderers plus which one is active.
///
/// Inactive renderers stay registered (dormant) so tests and future panels
/// can introspect the available styles without reconstructing them.
pub struct LoaderRegistry {
    shimmer: ShimmerRenderer,
    braille: BrailleRenderer,
    /// Canonical name of the active style ("shimmer" | "braille").
    active: &'static str,
}

impl LoaderRegistry {
    /// Build a registry from config. Unknown/missing styles → shimmer.
    pub fn from_config(cfg: &LoaderConfig) -> Self {
        let phrases = cfg.resolved_phrases();
        let active = cfg.resolved_style();
        Self {
            shimmer: ShimmerRenderer::new(phrases.clone()),
            braille: BrailleRenderer::new(phrases),
            active,
        }
    }

    /// Default registry: shimmer active, no overrides.
    pub fn default_registry() -> Self {
        Self::from_config(&LoaderConfig::default())
    }

    /// Canonical names of every registered renderer.
    pub fn registered_styles(&self) -> Vec<&'static str> {
        vec![super::config::STYLE_SHIMMER, STYLE_BRAILLE]
    }

    /// Canonical name of the currently active renderer.
    pub fn active_style(&self) -> &'static str {
        self.active
    }

    /// Point the registry at another built-in (no-op when unknown).
    ///
    /// Used by future settings panels and tests; falls back silently to
    /// keep the loader always renderable.
    pub fn set_active(&mut self, style: &str) {
        if style == super::config::STYLE_SHIMMER || style == STYLE_BRAILLE {
            self.active = match style {
                STYLE_BRAILLE => STYLE_BRAILLE,
                _ => super::config::STYLE_SHIMMER,
            };
        }
    }

    fn dispatch_active(&self) -> &dyn LoaderRenderer {
        match self.active {
            STYLE_BRAILLE => &self.braille,
            _ => &self.shimmer,
        }
    }

    /// Advance the active renderer's animation state.
    pub fn tick(&mut self) {
        // Renderers are currently stateless per-tick (all animation derives
        // from the global counter), so ticking is a no-op kept on the trait
        // contract for future effectful renderers.
    }

    /// Render the active loader into spans + visible width.
    pub fn spans(&self, info: &LoaderInfo) -> (Vec<Span<'static>>, u16) {
        self.dispatch_active().spans(info)
    }

    /// Direct access to a named dormant renderer (tests / panel preview).
    pub fn spans_for_style(
        &self,
        style: &str,
        info: &LoaderInfo,
    ) -> Option<(Vec<Span<'static>>, u16)> {
        match style {
            super::config::STYLE_SHIMMER => Some(self.shimmer.spans(info)),
            STYLE_BRAILLE => Some(self.braille.spans(info)),
            _ => None,
        }
    }
}

impl Default for LoaderRegistry {
    fn default() -> Self {
        Self::default_registry()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_registry_defaults_to_shimmer() {
        let r = LoaderRegistry::default_registry();
        assert_eq!(r.active_style(), "shimmer");
        assert_eq!(r.registered_styles(), vec!["shimmer", "braille"]);
    }

    #[test]
    fn test_loader_registry_from_config_respects_braille() {
        let mut cfg = LoaderConfig::default();
        cfg.style = Some(STYLE_BRAILLE.to_string());
        let r = LoaderRegistry::from_config(&cfg);
        assert_eq!(r.active_style(), STYLE_BRAILLE);
    }

    #[test]
    fn test_loader_registry_unknown_config_style_falls_back() {
        let mut cfg = LoaderConfig::default();
        cfg.style = Some("neon".to_string());
        let r = LoaderRegistry::from_config(&cfg);
        assert_eq!(r.active_style(), "shimmer");
    }

    #[test]
    fn test_loader_registry_set_active_toggles_and_ignores_junk() {
        let mut r = LoaderRegistry::default_registry();
        r.set_active("braille");
        assert_eq!(r.active_style(), "braille");
        r.set_active("junk");
        assert_eq!(r.active_style(), "braille"); // unchanged
        r.set_active("shimmer");
        assert_eq!(r.active_style(), "shimmer");
    }

    #[test]
    fn test_loader_registry_spans_differ_by_active_style() {
        let info = LoaderInfo {
            state: super::super::info::SpinnerState::Thinking,
            tool_name: None,
            elapsed_secs: 1,
            tokens_out: 0,
            tick: 40,
        };
        let r = LoaderRegistry::default_registry();
        let (a, _) = r.spans(&info);
        let (b, _) = r.spans_for_style("braille", &info).unwrap();
        // Compare styled spans, not just joined text — both renderers can
        // emit identical characters while differing in per-char styling.
        let dbg = |v: &[Span<'static>]| format!("{:?}", v);
        assert_ne!(dbg(&a), dbg(&b));
    }

    #[test]
    fn test_loader_registry_spans_for_unknown_style_is_none() {
        let info = LoaderInfo::idle(0);
        let r = LoaderRegistry::default_registry();
        assert!(r.spans_for_style("neon", &info).is_none());
    }
}
