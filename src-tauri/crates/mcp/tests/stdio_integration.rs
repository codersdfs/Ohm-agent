use mcp::stdio::StdioTransport;
use mcp::McpRequest;

/// Integration test: spawn the mock MCP server binary and verify
/// end-to-end Content-Length framed communication over stdio.
#[tokio::test]
async fn stdio_end_to_end_with_mock_server() {
    let exe = env!("CARGO_BIN_EXE_mock-mcp-server");
    let transport = StdioTransport::spawn(exe, &[]);
    assert!(
        transport.is_ok(),
        "Failed to spawn mock MCP server: {:?}",
        transport.err()
    );

    let transport = transport.unwrap();

    // Send a ping request
    let request = McpRequest {
        method: "ping".into(),
        params: None,
        id: "test-1".into(),
    };

    let response = transport.send(request).await;
    assert!(
        response.is_ok(),
        "End-to-end stdio transport failed: {:?}",
        response.err()
    );

    let resp = response.unwrap();
    assert_eq!(resp.id, "test-1");
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());

    // The mock returns {"result":"pong"}
    if let Some(result) = resp.result {
        let result_str = result.to_string();
        assert!(
            result_str.contains("pong"),
            "Expected 'pong' in result, got: {}",
            result_str
        );
    }

    let _ = transport.close().await;
}
