//! Integration tests for memory management commands.

use omega_core::commands::memory;

#[tokio::test]
async fn test_memory_store_and_search() {
    let state = omega_core::AppState::new(":memory:");
    let id = memory::memory_store_project(&state, "test_key".into(), "test_value".into())
        .await
        .unwrap();
    assert!(!id.is_empty());

    let result = memory::memory_search_project(&state, "test".into()).await.unwrap();
    assert!(!result.entries.is_empty());
}

#[tokio::test]
async fn test_memory_list() {
    let state = omega_core::AppState::new(":memory:");
    memory::memory_store_project(&state, "key1".into(), "val1".into()).await.unwrap();
    memory::memory_store_project(&state, "key2".into(), "val2".into()).await.unwrap();

    let entries = memory::memory_list_project(&state).await.unwrap();
    assert!(entries.len() >= 2);
}
