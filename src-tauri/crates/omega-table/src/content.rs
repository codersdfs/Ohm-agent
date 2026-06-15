use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableContent {
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}
