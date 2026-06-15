use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMeta {
    pub description: String,
    pub tags: Vec<String>,
    pub source: String,
    pub stats: HashMap<String, serde_json::Value>,
    pub schema_version: String,
}
