// Omega Tables — .otable format with three-level progressive loading
// Index -> Meta -> Content with LRU caching, FTS5 search, and embedding support.

pub mod content;
pub mod index;
pub mod lru;
pub mod meta;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OTable {
    pub path: String,
    pub index: index::TableIndex,
    pub meta: Option<meta::TableMeta>,
    pub content: Option<content::TableContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

pub const OTABLE_EXTENSION: &str = ".otable";
