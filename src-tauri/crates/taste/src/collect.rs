#![allow(dead_code)]

use super::*;
use std::collections::HashMap;

/// Raw collected artifact data
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct RawArtifact {
    pub id: String,
    pub artifact_type: ArtifactType,
    pub language: Language,
    pub path: String,
    pub content: String,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

/// Type of artifact being collected
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ArtifactType {
    CodeFile,
    ConfigFile,
    GitCommit,
    PRComment,
    EditorAction,
    BuildCommand,
}

/// Data collector interface
pub trait DataCollector {
    fn extract_features(&self, path: &str, content: &str, lang: Language) -> Result<FeatureVector, Box<dyn std::error::Error>>;
    fn collect_artifacts(&self, repo_path: &str) -> Vec<RawArtifact>;
}

/// Feature vector for ML consumption
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FeatureVector {
    pub id: String,
    pub language_idx: u8,
    pub code_length: usize,
    pub clone_usage_rate: f32,
    pub unwrap_usage_rate: f32,
    pub any_type_rate: f32,
    pub magic_number_rate: f32,
    pub nesting_depth: f32,
    pub test_coverage_estimate: f32,
    pub timestamp: u64,
    pub raw_features: Vec<f32>,
}

/// Default static-analysis based collector
pub struct DefaultDataCollector {}

impl DefaultDataCollector {
    pub fn new() -> Self {
        Self {}
    }
}

impl DataCollector for DefaultDataCollector {
    fn extract_features(
        &self,
        path: &str,
        content: &str,
        lang: Language,
    ) -> Result<FeatureVector, Box<dyn std::error::Error>> {
        let lines = content.lines().count().max(1) as f32;
        let code_length = content.len();

        let clone_count = content.matches(".clone()").count() as f32;
        let unwrap_count = content.matches(".unwrap()").count() as f32;
        let any_count = content.matches(": any").count() as f32;
        let magic_count = content.matches(|c: char| c.is_ascii_digit()).count() as f32;

        let clone_rate = clone_count / lines;
        let unwrap_rate = unwrap_count / lines;
        let any_rate = any_count / lines;
        let magic_rate = magic_count / lines.max(1.0);

        let mut max_depth = 0.0f32;
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("#") {
                let spaces = line.len() - line.trim_start().len();
                let depth = (spaces / 4) as f32;
                if depth > max_depth {
                    max_depth = depth;
                }
            }
        }

        let raw_features = vec![clone_rate, unwrap_rate, any_rate, magic_rate, max_depth, 0.0, 0.0, 0.0];

        Ok(FeatureVector {
            id: format!("feat_{}_{}", path.replace("\\", "_"), lang.to_index()),
            language_idx: lang.to_index() as u8,
            code_length,
            clone_usage_rate: clone_rate,
            unwrap_usage_rate: unwrap_rate,
            any_type_rate: any_rate,
            magic_number_rate: magic_rate,
            nesting_depth: max_depth,
            test_coverage_estimate: 0.0,
            timestamp: 0,
            raw_features,
        })
    }

    fn collect_artifacts(&self, _repo_path: &str) -> Vec<RawArtifact> {
        vec![]
    }
}

impl Default for DefaultDataCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for language index conversion
pub trait LangIndexExt {
    fn to_index(&self) -> usize;
}

impl LangIndexExt for Language {
    fn to_index(&self) -> usize {
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
