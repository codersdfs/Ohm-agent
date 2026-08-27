//! Pluggable activity loader ("thinking / streaming" indicator).
//!
//! Architecture:
//! - [`info::LoaderInfo`] — per-frame snapshot passed to renderers
//! - [`renderer::LoaderRenderer`] — the plugin trait (seam for future config panel)
//! - [`registry::LoaderRegistry`] — owns built-ins, exposes one active style
//! - [`config::LoaderConfig`] — serde contract in `config.json` under `"loader"`
//!
//! Built-in renderers: [`shimmer`] (default, comet sweep) and [`braille`]
//! (refined classic). Exactly one is active at a time; unknown styles fall
//! back to shimmer at every layer, never crashing on stale configs.
//!
//! Replaces the former `tui::spinner::OmegaSpinner`, which previously welded
//! presentation into `status.rs`.

pub mod braille;
pub mod config;
pub mod info;
pub mod registry;
pub mod renderer;
pub mod shimmer;
pub mod suffix;

pub use config::{LoaderAnchor, LoaderConfig};
pub use info::{LoaderInfo, SpinnerState};
pub use registry::LoaderRegistry;
pub use renderer::LoaderRenderer;
