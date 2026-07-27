use crate::{AppState, MutexExt};
use serde::{Deserialize, Serialize};

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

pub async fn memory_store(state: &AppState, request: MemoryStoreRequest) -> Result<String, String> {
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
    let result = store.search(
        &request.query,
        request.layer.as_deref(),
        request.limit.unwrap_or(10),
    )?;

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

pub async fn memory_count(state: &AppState, layer: Option<String>) -> Result<usize, String> {
    let store = state.memory_store.lock_guard();
    store.count(layer.as_deref())
}

pub async fn memory_delete(state: &AppState, id: String) -> Result<(), String> {
    let store = state.memory_store.lock_guard();
    store.delete(&id)
}

pub async fn memory_clear(state: &AppState, layer: Option<String>) -> Result<usize, String> {
    let store = state.memory_store.lock_guard();
    store.clear(layer.as_deref())
}

/// Store a memory in the project layer, keyed by git root.
pub async fn memory_store_project(
    state: &AppState,
    key: String,
    value: String,
) -> Result<String, String> {
    let proj_key = crate::memory_retriever::project_key();
    let full_key = format!("{}:{}", proj_key, key);
    let store = state.memory_store.lock_guard();
    store.store(memory::MemoryLayer::Project, &full_key, &value)
}

/// Search the project layer for memories.
pub async fn memory_search_project(
    state: &AppState,
    query: String,
) -> Result<MemorySearchResponse, String> {
    let store = state.memory_store.lock_guard();
    let result = store.search(&query, Some("project"), 20)?;
    Ok(MemorySearchResponse {
        entries: result.entries,
        relevance: result.relevance,
    })
}

/// List all project-layer memories.
pub async fn memory_list_project(state: &AppState) -> Result<Vec<memory::MemoryEntry>, String> {
    let store = state.memory_store.lock_guard();
    let result = store.search("", Some("project"), 100)?;
    Ok(result.entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AppState {
        AppState::new(":memory:")
    }

    #[tokio::test]
    async fn test_memory_store_project() {
        let state = test_state();
        let result = memory_store_project(&state, "build_cmd".into(), "cargo tauri dev".into()).await;
        assert!(result.is_ok(), "should store project memory");
        let id = result.unwrap();
        assert!(!id.is_empty(), "should return a non-empty id");
    }

    #[tokio::test]
    async fn test_memory_search_project() {
        let state = test_state();
        memory_store_project(&state, "api_url".into(), "https://api.example.com".into()).await.unwrap();
        let result = memory_search_project(&state, "api".into()).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.entries.is_empty(), "should find at least one result");
    }

    #[tokio::test]
    async fn test_memory_list_project() {
        let state = test_state();
        memory_store_project(&state, "key1".into(), "value1".into()).await.unwrap();
        memory_store_project(&state, "key2".into(), "value2".into()).await.unwrap();
        let result = memory_list_project(&state).await;
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert!(entries.len() >= 2, "should have at least 2 entries");
    }
}
