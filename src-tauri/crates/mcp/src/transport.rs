use crate::{McpRequest, McpResponse};

pub struct JsonRpcTransport {
    endpoint: String,
}

impl JsonRpcTransport {
    pub fn new(endpoint: &str) -> Self {
        Self { endpoint: endpoint.to_string() }
    }

    pub async fn send(&self, _request: McpRequest) -> Result<McpResponse, String> {
        Ok(McpResponse {
            id: "1".into(),
            result: None,
            error: None,
        })
    }
}
