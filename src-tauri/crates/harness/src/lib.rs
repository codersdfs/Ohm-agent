pub mod engine;
pub mod external;
pub mod gate_config;
pub mod golden;
pub mod negative_knowledge;
pub mod patterns;
pub mod persistence;
pub mod repeated;
pub mod repomap;
pub mod rules;
pub mod scoring;
pub mod structural;
pub mod taste;
pub mod tree_sitter_metrics;

// Public types re-exported from submodules
pub use violation::{Violation, ViolationCategory, GateResult};
pub use engine::GateEngine;
pub use language::Language;
pub use taste::TasteCheck;

// Internal submodules
mod violation;
mod language;

#[cfg(feature = "taste-system")]
pub mod taste_integration;
