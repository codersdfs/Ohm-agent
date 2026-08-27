//! Loader renderer plugin trait — the seam future configuration panels
//! write against.
//!
//! A renderer owns all presentation logic for the loader. It receives a
//! [`LoaderInfo`] snapshot each frame and returns styled spans that the
//! status line composes left-to-right. Renderers must stay within one
//! terminal row.

use ratatui::text::Span;

use super::info::LoaderInfo;

/// Pluggable loader renderer.
///
/// Implementations are registered at App construction via
/// [`super::registry::LoaderRegistry`] and exactly one is active at a time,
/// selected by `LoaderConfig.style`.
pub trait LoaderRenderer {
    /// Stable style name, matching `LoaderConfig`'s `"style"` string.
    fn name(&self) -> &'static str;

    /// Advance any internal animation state (called once per tick).
    fn tick(&mut self);

    /// Build the styled spans for this frame's loader row.
    ///
    /// Returns `(spans, visible_width)` where width equals the sum of
    /// `Span::width()` across spans so callers can lay out around it
    /// without re-measuring.
    fn spans(&self, info: &LoaderInfo) -> (Vec<Span<'static>>, u16);
}
