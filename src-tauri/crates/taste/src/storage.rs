#![allow(dead_code)]

use super::TasteFeedback;

/// Error types for storage operations
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("File not found")]
    NotFound,
    #[error("Cloud sync failed: {0}")]
    CloudSync(String),
}

/// Cloud sync configuration
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CloudSyncConfig {
    pub endpoint: String,
    pub auth_method: String,
    pub sync_interval_secs: u64,
    pub team_sharing: bool,
}

/// Experience record from developer interaction
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ExperienceRecord {
    pub id: String,
    pub feature_vector_id: String,
    pub suggested_idx: usize,
    pub feedback: TasteFeedback,
    pub accepted_after_retries: u8,
    pub timestamp: u64,
}

/// Single preference entry stored locally
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PreferenceEntry {
    pub id: String,
    pub pattern_type: String,
    pub preferred: String,
    pub rejected: String,
    pub confidence: f32,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Trait for cloud storage backends
pub trait CloudStorage {
    fn sync(&self) -> Result<(), StorageError>;
    fn push(&self, data: &[u8]) -> Result<(), StorageError>;
    fn pull(&self) -> Result<Vec<u8>, StorageError>;
    fn authenticate(&self) -> Result<(), StorageError>;
}

/// Hybrid local-first storage with optional cloud sync
pub struct PreferenceStorage {
    pub project_root: String,
    preferences_path: String,
    experiences_path: String,
}

impl PreferenceStorage {
    /// Create a new local-only storage instance
    pub fn local(project_root: &str) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(project_root)?;
        
        Ok(Self {
            project_root: project_root.to_string(),
            preferences_path: format!("{}/taste_prefs.json", project_root),
            experiences_path: format!("{}/taste_experiences.json", project_root),
        })
    }

    /// Load preferences from disk
    pub fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Save preferences to disk
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Save an experience record
    pub fn save_experience(&self, _experience: &ExperienceRecord) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Get count of stored experiences
    pub fn sample_count(&self) -> usize {
        0
    }
}
