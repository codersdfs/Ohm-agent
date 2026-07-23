//! MCP error type and error code constants

use crate::types;
use std::fmt;

/// MCP server error
#[derive(Debug, Clone)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl McpError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn parse_error() -> Self {
        Self::new(types::PARSE_ERROR, "Parse error")
    }

    pub fn invalid_request() -> Self {
        Self::new(types::INVALID_REQUEST, "Invalid Request")
    }

    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::new(
            types::METHOD_NOT_FOUND,
            format!("Method not found: {}", method.into()),
        )
    }

    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self::new(types::INVALID_PARAMS, format!("Invalid params: {}", detail.into()))
    }

    pub fn internal_error(detail: impl Into<String>) -> Self {
        Self::new(types::INTERNAL_ERROR, format!("Internal error: {}", detail.into()))
    }

    pub fn tool_not_found(name: impl Into<String>) -> Self {
        Self::new(
            types::MCP_TOOL_NOT_FOUND,
            format!("Tool not found: {}", name.into()),
        )
    }

    pub fn tool_execution_error(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(
            types::MCP_TOOL_EXECUTION_ERROR,
            format!("Tool '{}' execution failed: {}", name.into(), detail.into()),
        )
    }

    pub fn to_json_rpc_response(&self, id: types::RequestId) -> types::JsonRpcResponse {
        match &self.data {
            Some(data) => types::JsonRpcResponse::error_with_data(
                id,
                self.code,
                &self.message,
                data.clone(),
            ),
            None => types::JsonRpcResponse::error(id, self.code, &self.message),
        }
    }
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for McpError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_error_defaults() {
        let err = McpError::parse_error();
        assert_eq!(err.code, types::PARSE_ERROR);
        assert_eq!(err.message, "Parse error");
    }

    #[test]
    fn test_mcp_error_to_response() {
        let err = McpError::method_not_found("foo");
        let resp = err.to_json_rpc_response(types::RequestId::Str("1".into()));
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["error"]["code"], types::METHOD_NOT_FOUND);
        assert!(json["error"]["message"].as_str().unwrap().contains("foo"));
    }

    #[test]
    fn test_mcp_error_display() {
        let err = McpError::new(-1, "something broke");
        assert_eq!(format!("{}", err), "[-1] something broke");
    }
}