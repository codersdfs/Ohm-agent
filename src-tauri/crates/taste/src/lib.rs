#![allow(dead_code)]


pub mod model;
pub mod collect;
pub mod storage;
pub mod cli;

// Re-export key types from submodules
pub use model::{RLLossFunction, TrustRegionConstraint, AdvantageLengthPenalty, Experience, Context};
pub use collect::{DataCollector, DefaultDataCollector, FeatureVector, ArtifactType, RawArtifact};
pub use storage::{PreferenceStorage, PreferenceEntry, ExperienceRecord, StorageError, CloudSyncConfig};
pub use cli::{TasteCli, TasteCommand, run};

// ======================
// Core Types
// ======================

/// Configuration for the taste agent
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TasteConfig {
    pub enabled: bool,
    pub max_experience_replays: usize,
    pub min_feedback_count_for_update: usize,
}

impl Default for TasteConfig {
    fn default() -> Self {
        TasteConfig::new()
    }
}

impl TasteConfig {
    pub fn new() -> Self {
        TasteConfig {
            enabled: false,
            max_experience_replays: 1000,
            min_feedback_count_for_update: 10,
        }
    }
}

/// The main taste agent that encapsulates the learning pipeline
pub struct TasteAgent {
    config: TasteConfig,
}

impl TasteAgent {
    pub fn new(config: TasteConfig) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self { config })
    }

    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub async fn process_and_update(
        &mut self,
        _path: &str,
        _content: &str,
        _lang: Language,
        feedback: TasteFeedback,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _ = feedback;
        Ok(())
    }

    pub async fn suggest_corrections(
        &self,
        _path: &str,
        _content: &str,
        _lang: Language,
    ) -> Result<Vec<TasteSuggestion>, Box<dyn std::error::Error>> {
        Ok(Vec::new())
    }

    pub fn apply_taste_score(&self, base_score: u32, _violations: &[Violation], _lang: Language) -> u32 {
        if !self.config.enabled {
            return base_score;
        }
        base_score
    }

    pub fn set_enabled(&self, _enabled: bool) {}
    pub fn config(&self) -> &TasteConfig { &self.config }
}

/// Feedback from developer about a suggestion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TasteFeedback {
    Accepted,
    Rejected,
    Edited,
    Ignored,
}

/// A suggested code modification based on learned taste
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TasteSuggestion {
    pub category: ViolationCategory,
    pub message: String,
    pub correction: String,
    pub confidence: f32,
    pub tool_hint: Option<String>,
}

/// Trait-based model interface
pub trait TraitModel {
    fn predict(&self, features: &[f32]) -> Vec<TasteSuggestion>;
    fn train(&mut self, experiences: &[ExperienceRecord]);
    fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn load(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>>;
}

/// Programming language enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    TypeScriptReact,
    JavaScript,
    Python,
    Go,
    CSharp,
    Java,
    Other(String),
}

impl Language {
    pub fn to_index(&self) -> usize {
        match self {
            Language::Rust => 0,
            Language::TypeScript | Language::JavaScript | Language::TypeScriptReact => 1,
            Language::Python => 2,
            Language::Go => 3,
            Language::CSharp => 4,
            Language::Java => 5,
            Language::Other(_) => 6,
        }
    }
}

/// Violation data used in scoring
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Violation {
    pub category: ViolationCategory,
    pub message: String,
    pub tool_hint: Option<String>,
    pub line: Option<u32>,
}

/// Violation categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ViolationCategory {
    Structural,
    Taste,
    Golden,
    Repeated,
    External,
}
