//! Configuration for remote MCP server connections

use serde::{Deserialize, Serialize};

/// Authentication method for a remote MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// No authentication
    None,
    /// Header-based auth (e.g., x-api-key)
    Header {
        name: String,
        value: String,
    },
    /// Bearer token
    Bearer {
        token: String,
    },
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::None
    }
}

/// Transport type for a remote MCP server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransportType {
    /// JSON-RPC over HTTP POST
    Http,
    /// HTTP + SSE streaming
    HttpSse,
    /// Local process via stdio
    Stdio {
        command: String,
        args: Vec<String>,
    },
}

impl Default for TransportType {
    fn default() -> Self {
        Self::Http
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteServerConfig {
    pub name: String,

    #[serde(default)]
    pub url: Option<String>,

    #[serde(default)]
    pub transport: TransportType,

    #[serde(default)]
    pub auth: AuthConfig,

    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    #[serde(default)]
    pub allow_tools: Option<Vec<String>>,

    #[serde(default)]
    pub deny_tools: Option<Vec<String>>,

    #[serde(default)]
    pub tool_prefix: Option<String>,
}

fn default_timeout() -> u64 {
    30
}

impl RemoteServerConfig {
    /// Create a simple HTTP remote server config
    pub fn http(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: Some(url.into()),
            transport: TransportType::Http,
            auth: AuthConfig::None,
            timeout_seconds: 30,
            allow_tools: None,
            deny_tools: None,
            tool_prefix: None,
        }
    }

    /// Create an HTTP remote server with header auth
    pub fn http_with_header(
        name: impl Into<String>,
        url: impl Into<String>,
        header_name: impl Into<String>,
        header_value: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            url: Some(url.into()),
            transport: TransportType::Http,
            auth: AuthConfig::Header {
                name: header_name.into(),
                value: header_value.into(),
            },
            timeout_seconds: 30,
            allow_tools: None,
            deny_tools: None,
            tool_prefix: None,
        }
    }

    /// Create a stdio remote server config (local process)
    pub fn stdio(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            url: None,
            transport: TransportType::Stdio {
                command: command.into(),
                args,
            },
            auth: AuthConfig::None,
            timeout_seconds: 30,
            allow_tools: None,
            deny_tools: None,
            tool_prefix: None,
        }
    }
}

impl Default for RemoteServerConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            url: None,
            transport: TransportType::default(),
            auth: AuthConfig::None,
            timeout_seconds: 30,
            allow_tools: None,
            deny_tools: None,
            tool_prefix: None,
        }
    }
}

/// Top-level MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpBridgeConfig {
    /// Remote MCP servers to connect to
    #[serde(default)]
    pub remote_servers: Vec<RemoteServerConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_remote_config() {
        let config = RemoteServerConfig::http("figma", "https://mcp.figma.com/mcp");
        assert_eq!(config.name, "figma");
        assert_eq!(config.url.as_deref(), Some("https://mcp.figma.com/mcp"));
        assert_eq!(config.transport, TransportType::Http);
    }

    #[test]
    fn test_http_with_header_auth() {
        let config = RemoteServerConfig::http_with_header(
            "21st_dev",
            "https://21st.dev/api/mcp",
            "x-api-key",
            "sk-abc123",
        );
        match config.auth {
            AuthConfig::Header { name, value } => {
                assert_eq!(name, "x-api-key");
                assert_eq!(value, "sk-abc123");
            }
            _ => panic!("Expected Header auth"),
        }
    }

    #[test]
    fn test_stdio_config() {
        let config = RemoteServerConfig::stdio(
            "local-server",
            "node",
            vec!["server.js".into()],
        );
        match config.transport {
            TransportType::Stdio { ref command, ref args } => {
                assert_eq!(command, "node");
                assert_eq!(args[0], "server.js");
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_bridge_config_serde() {
        let config = McpBridgeConfig {
            remote_servers: vec![
                RemoteServerConfig::http("figma", "https://mcp.figma.com/mcp"),
                RemoteServerConfig::http_with_header(
                    "21st_dev",
                    "https://21st.dev/api/mcp",
                    "x-api-key",
                    "sk-abc123",
                ),
            ],
        };
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("figma"));
        assert!(json.contains("21st_dev"));
        assert!(json.contains("x-api-key"));

        // Deserialize back
        let deserialized: McpBridgeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.remote_servers.len(), 2);
    }
}