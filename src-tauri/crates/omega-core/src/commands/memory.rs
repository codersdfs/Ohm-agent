use serde::{Deserialize, Serialize};
use crate::{AppState, MutexExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStoreRequest {
    pub key: String,
    pub value: String,
    pub layer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchRequest {
    pub query: String,
    pub layer: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResponse {
    pub entries: Vec<memory::MemoryEntry>,
    pub relevance: Vec<f64>,
}

pub async fn memory_store(
    state: &AppState,
    request: MemoryStoreRequest,
) -> Result<String, String> {
    log::info!("memory_store: key={}, layer={}", request.key, request.layer);

    let layer = memory::MemoryLayer::from_str(&request.layer);
    let store = state.memory_store.lock_guard();
    store.store(layer, &request.key, &request.value)
}

pub async fn memory_search(
    state: &AppState,
    request: MemorySearchRequest,
) -> Result<MemorySearchResponse, String> {
    log::info!("memory_search: query={}", request.query);

    let store = state.memory_store.lock_guard();
    let result = store.search(&request.query, request.layer.as_deref(), request.limit.unwrap_or(10))?;

    Ok(MemorySearchResponse {
        entries: result.entries,
        relevance: result.relevance,
    })
}

pub async fn memory_remember(
    state: &AppState,
    key: String,
    layer: Option<String>,
) -> Result<Option<String>, String> {
    log::info!("memory_remember: key={}", key);

    let store = state.memory_store.lock_guard();
    store.remember(&key, layer.as_deref())
}

pub async fn memory_count(
    state: &AppState,
    layer: Option<String>,
) -> Result<usize, String> {
    let store = state.memory_store.lock_guard();
    store.count(layer.as_deref())
}

pub async fn memory_delete(
    state: &AppState,
    id: String,
) -> Result<(), String> {
    let store = state.memory_store.lock_guard();
    store.delete(&id)
}

pub async fn memory_clear(
    state: &AppState,
    layer: Option<String>,
) -> Result<usize, String> {
    let store = state.memory_store.lock_guard();
    store.clear(layer.as_deref())
}
