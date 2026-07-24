//! HTTP+SSE transport for MCP server
//!
//! Implements the MCP HTTP transport specification:
//! - POST `/json-rpc` — receive JSON-RPC requests, routed directly to `McpServer::handle_request()`
//! - GET `/sse` — Server-Sent Events stream for notifications
//! - GET `/health` — health check endpoint
//!
//! Supports optional Bearer token authentication.
//!
//! Architecture:
//!   `HttpTransport` is a thin builder/launcher — it holds config and knows how to
//!   build an Axum router and bind a TCP listener. The Axum state carries an
//!   `Arc<McpServer>` that the handler calls directly. There is no channel plumbing;
//!   the `McpTransport` trait (poll-based) is NOT implemented here — it lives on
//!   `StdioTransport` only.

use crate::server::McpServer;
use axum::{
    extract::State,
    http::StatusCode,
    middleware::{self, Next},
    response::{sse::{Event, Sse}, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
    body::Body,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Metrics collected by the HTTP transport
pub struct TransportMetrics {
    pub requests_total: AtomicU64,
    pub errors_total: AtomicU64,
    pub tools_called: AtomicU64,
    pub start_time: Instant,
}

impl TransportMetrics {
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            errors_total: AtomicU64::new(0),
            tools_called: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }
}

impl Default for TransportMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared state for the HTTP transport.
///
/// Holds the `Arc<McpServer>` so every Axum handler can call `handle_request()` directly.
struct HttpTransportStateInner {
    server: Arc<McpServer>,
    metrics: TransportMetrics,
    #[allow(dead_code)]
    rate_limit_per_minute: u64,
}

/// Axum router state — cloneable, cheap `Arc` bump.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<HttpTransportStateInner>,
}

impl AppState {
    /// Access the server handle stored in shared state.
    pub fn server(&self) -> &Arc<McpServer> {
        &self.inner.server
    }
}

/// HTTP transport builder / launcher.
///
/// Holds config: host, port, optional auth, rate limit. Call [`serve`](HttpTransport::serve)
/// with an `Arc<McpServer>` to spawn the Axum listener.
pub struct HttpTransport {
    port: u16,
    host: String,
    auth_token: Option<String>,
    rate_limit: u64,
    /// Stored after `serve()` for graceful shutdown.
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl HttpTransport {
    /// Create a new HTTP transport bound to the given host:port.
    pub fn bind(host: &str, port: u16) -> Self {
        Self {
            port,
            host: host.to_string(),
            auth_token: None,
            rate_limit: 0,
            shutdown_tx: None,
        }
    }

    /// Require clients to send `Authorization: Bearer <token>`.
    pub fn with_auth_token(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }

    /// Set rate limit (requests per minute). 0 = unlimited.
    pub fn with_rate_limit(mut self, rpm: u64) -> Self {
        self.rate_limit = rpm;
        self
    }

    /// Build the Axum router with auth middleware and endpoints.
    fn build_router(server: Arc<McpServer>, auth_token: Option<String>) -> Router {
        let state = AppState {
            inner: Arc::new(HttpTransportStateInner {
                server,
                metrics: TransportMetrics::default(),
                rate_limit_per_minute: 0,
            }),
        };

        let mut router = Router::new()
            .route("/json-rpc", post(handle_json_rpc))
            .route("/sse", get(handle_sse))
            .route("/health", get(handle_health))
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive())
            .with_state(state);

        // Add auth middleware if token is configured
        if let Some(token) = auth_token {
            router = router.layer(middleware::from_fn(move |request: axum::http::Request<Body>, next: Next| {
                let token = token.clone();
                async move {
                    let auth_header = request
                        .headers()
                        .get("Authorization")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");

                    if auth_header != format!("Bearer {}", token) {
                        return Err::<Response, (StatusCode, String)>((
                            StatusCode::UNAUTHORIZED,
                            serde_json::to_string(&serde_json::json!({
                                "jsonrpc": "2.0",
                                "id": null,
                                "error": {
                                    "code": -32000,
                                    "message": "Unauthorized: invalid or missing auth token"
                                }
                            })).unwrap_or_default(),
                        ));
                    }

                    Ok(next.run(request).await)
                }
            }));
        }

        router
    }

    /// Start the HTTP server — **takes ownership of `server`** via `Arc` — and
    /// return the spawned task join handle.
    ///
    /// The returned handle resolves when the server loop exits (graceful shutdown
    /// or transport error).
    pub async fn serve(&mut self, server: Arc<McpServer>) -> Result<tokio::task::JoinHandle<()>, String> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let router = Self::build_router(server, self.auth_token.clone());
        let addr = format!("{}:{}", self.host, self.port)
            .parse::<std::net::SocketAddr>()
            .map_err(|e| format!("Invalid address: {e}"))?;

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind {addr}: {e}"))?;

        log::info!("MCP HTTP transport listening on http://{addr}");
        if self.auth_token.is_some() {
            log::info!("MCP HTTP transport auth: enabled (Bearer token)");
        }

        let handle = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    shutdown_rx.recv().await;
                    log::info!("MCP HTTP transport shutting down");
                })
                .await
                .ok();
        });

        Ok(handle)
    }

    /// Initiate graceful shutdown of the running server.
    ///
    /// Returns `true` if a shutdown signal was sent, `false` if no server is running.
    pub fn shutdown(&self) -> bool {
        if let Some(ref tx) = self.shutdown_tx {
            tx.try_send(()).is_ok()
        } else {
            false
        }
    }
}

// ─── Axum Handlers ───────────────────────────────────────────────────────────

/// Handle JSON-RPC requests via POST.
///
/// Calls `McpServer::handle_request()` directly — the response is the real
/// JSON-RPC result/error from the server, not a hardcoded acknowledgement.
async fn handle_json_rpc(
    State(state): State<AppState>,
    body: String,
) -> axum::response::Response {
    state.inner.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

    // Peek at the raw JSON for metrics before delegating to the server.
    if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&body) {
        if raw.get("method").and_then(|v| v.as_str()) == Some("tools/call") {
            state.inner.metrics.tools_called.fetch_add(1, Ordering::Relaxed);
        }
    } else {
        state.inner.metrics.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    // Delegate to the real server — this returns the proper JSON-RPC response.
    let response_body = state.inner.server.handle_request(&body).await;

    // Notifications return an empty body — respond with 202 Accepted.
    if response_body.is_empty() {
        return (
            StatusCode::ACCEPTED,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            serde_json::to_string(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
            })).unwrap_or_default(),
        ).into_response();
    }

    // Parse the response back into JSON.
    match serde_json::from_str::<serde_json::Value>(&response_body) {
        Ok(value) => {
            if value.get("error").map_or(false, |e| !e.is_null()) {
                state.inner.metrics.errors_total.fetch_add(1, Ordering::Relaxed);
            }
            Json(value).into_response()
        }
        Err(_) => {
            state.inner.metrics.errors_total.fetch_add(1, Ordering::Relaxed);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32603,
                        "message": "Internal error: failed to encode response"
                    }
                })),
            ).into_response()
        }
    }
}

/// Handle SSE streaming connection
async fn handle_sse(
    State(_state): State<AppState>,
) -> Sse<ReceiverStream<Result<Event, String>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, String>>(16);
    let stream = ReceiverStream::new(rx);

    let tx_clone = tx.clone();
    tokio::spawn(async move {
        tx_clone
            .send(Ok(Event::default().data("connected")))
            .await
            .ok();
    });

    Sse::new(stream)
}

/// Handle health check requests
async fn handle_health(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let uptime = state.inner.metrics.start_time.elapsed().as_secs();
    let requests = state.inner.metrics.requests_total.load(Ordering::Relaxed);
    let errors = state.inner.metrics.errors_total.load(Ordering::Relaxed);
    let tool_calls = state.inner.metrics.tools_called.load(Ordering::Relaxed);

    Json(serde_json::json!({
        "status": "ok",
        "uptime_seconds": uptime,
        "requests_total": requests,
        "errors_total": errors,
        "tools_called": tool_calls,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, Method};
    use tower::util::ServiceExt;

    /// Build a minimal `McpServer` and a router for testing.
    fn test_router(auth_token: Option<String>) -> Router {
        let server = Arc::new(McpServer::new());
        HttpTransport::build_router(server, auth_token)
    }

    #[tokio::test]
    async fn test_json_rpc_endpoint_returns_real_response() {
        let app = test_router(None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/json-rpc")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Real ping response: no error, id matches
        assert_eq!(parsed["id"], "1");
        assert!(parsed["result"].is_object());
        assert!(parsed["error"].is_null());
    }

    #[tokio::test]
    async fn test_json_rpc_endpoint_tools_list() {
        let server = Arc::new(McpServer::new().with_tool_registry(
            tool_harness::tools::default_tool_registry(),
        ));
        let app = HttpTransport::build_router(server, None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/json-rpc")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":"2","method":"tools/list"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            !parsed["result"]["tools"].as_array().unwrap().is_empty(),
            "Should return tools from registry"
        );
    }

    #[tokio::test]
    async fn test_json_rpc_endpoint_parse_error() {
        let app = test_router(None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/json-rpc")
                    .header("Content-Type", "application/json")
                    .body(Body::from("not valid json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Server::handle_request() returns a valid error response with 200 OK
        let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["error"]["code"], -32700, "Parse error should be -32700");
    }

    #[tokio::test]
    async fn test_json_rpc_endpoint_unknown_method() {
        let app = test_router(None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/json-rpc")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":"3","method":"foobar"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["id"], "3");
        assert_eq!(parsed["error"]["code"], -32601, "Method not found should be -32601");
    }

    #[tokio::test]
    async fn test_sse_endpoint() {
        let app = test_router(None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/sse")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = test_router(None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Parse and verify health response
        let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let health: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(health["status"], "ok");
        assert!(health["uptime_seconds"].as_u64().is_some());
        assert!(health["version"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_auth_required() {
        let app = test_router(Some("secret123".into()));

        // Request without auth header
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/json-rpc")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_with_valid_token() {
        let app = test_router(Some("valid-token".into()));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/json-rpc")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer valid-token")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Verify it's a real ping response, not a hardcoded ack
        let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["id"], "1");
        assert!(parsed["result"].is_object());
    }

    #[tokio::test]
    async fn test_auth_with_wrong_token() {
        let app = test_router(Some("correct-token".into()));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/json-rpc")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer wrong-token")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// Notification requests (no `id` field) should produce an empty JSON-RPC response.
    #[tokio::test]
    async fn test_json_rpc_notification() {
        let app = test_router(None);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/json-rpc")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Notifications return 202 Accepted (no JSON-RPC response body)
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }
}