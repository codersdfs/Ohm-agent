//! Integration tests for web commands.
//! These tests require network access and are marked #[ignore] by default.

use omega_core::commands::web;

#[tokio::test]
#[ignore]
async fn test_fetch_integration_httpbin() {
    let result = web::fetch_url("https://httpbin.org/get").await;
    assert!(result.is_ok(), "httpbin should be reachable: {:?}", result.err());
    let body = result.unwrap();
    assert!(body.contains("url"), "response should contain 'url' field");
}

#[tokio::test]
#[ignore]
async fn test_status_integration() {
    let result = web::check_status().await;
    assert!(result.is_ok());
}

#[tokio::test]
#[ignore]
async fn test_fetch_truncation() {
    // httpbin.org/bytes/N returns N random bytes
    let result = web::fetch_url("https://httpbin.org/bytes/15000").await;
    assert!(result.is_ok());
    let body = result.unwrap();
    assert!(body.contains("truncated"), "large response should be truncated");
}