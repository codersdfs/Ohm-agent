//! Wire protocol types — ChatMessage/ChatRequest/ChatResponse and tool shapes.
//! Split out of `lib.rs` (P5 god-object split).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaToolCall {
    pub index: usize,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    pub function: Option<DeltaToolCallFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaToolCallFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub config: crate::ProviderConfig,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub content: String,
    /// Thinking/reasoning content (model-internal reasoning, not visible output).
    #[serde(default)]
    pub thinking: String,
    pub done: bool,
    pub model: Option<String>,
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_tool_calls: Option<Vec<DeltaToolCall>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_serialization() {
        let td = ToolDefinition {
            tool_type: "function".into(),
            function: ToolFunctionDef {
                name: "read".into(),
                description: "Read a file".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "filePath": { "type": "string", "description": "Path to file" }
                    },
                    "required": ["filePath"],
                }),
            },
        };
        let json = serde_json::to_string_pretty(&td).unwrap();
        assert!(json.contains("\"type\": \"function\""));
        assert!(json.contains("\"name\": \"read\""));
        assert!(json.contains("\"description\": \"Read a file\""));
        assert!(json.contains("\"filePath\""));
    }

    #[test]
    fn test_tool_call_roundtrip() {
        let tc = ToolCall {
            id: "call_abc123".into(),
            tool_type: "function".into(),
            function: ToolCallFunction {
                name: "read".into(),
                arguments: r#"{"filePath": "src/main.rs"}"#.into(),
            },
        };
        let json = serde_json::to_string(&tc).unwrap();
        let parsed: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "call_abc123");
        assert_eq!(parsed.function.name, "read");
        assert_eq!(parsed.function.arguments, r#"{"filePath": "src/main.rs"}"#);
    }

    #[test]
    fn test_chat_message_with_tool_calls() {
        let msg = ChatMessage {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".into(),
                tool_type: "function".into(),
                function: ToolCallFunction {
                    name: "bash".into(),
                    arguments: r#"{"command": "ls -la"}"#.into(),
                },
            }]),
            tool_call_id: None,
            name: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"tool_calls\""));
        assert!(json.contains("call_1"));
        assert!(json.contains("ls -la"));

        let parsed: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.role, "assistant");
        assert!(parsed.tool_calls.is_some());
        assert_eq!(parsed.tool_calls.unwrap().len(), 1);
    }

    #[test]
    fn test_chat_message_tool_result() {
        let msg = ChatMessage {
            role: "tool".into(),
            content: "command output here".into(),
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
            name: Some("bash".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"tool_call_id\""));
        assert!(json.contains("call_1"));
        assert!(json.contains("\"name\""));

        let parsed: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.role, "tool");
        assert_eq!(parsed.tool_call_id.unwrap(), "call_1");
        assert_eq!(parsed.name.unwrap(), "bash");
    }

    #[test]
    fn test_chat_response_with_tool_calls() {
        let resp = ChatResponse {
            content: String::new(),
            model: "gpt-4o".into(),
            usage: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_xyz".into(),
                tool_type: "function".into(),
                function: ToolCallFunction {
                    name: "edit".into(),
                    arguments:
                        r#"{"filePath": "test.txt", "oldString": "foo", "newString": "bar"}"#.into(),
                },
            }]),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"tool_calls\""));
        assert!(json.contains("\"call_xyz\""));

        let parsed: ChatResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.tool_calls.is_some());
        let calls = parsed.tool_calls.unwrap();
        assert_eq!(calls[0].function.name, "edit");
    }

    #[test]
    fn test_chat_response_no_tool_calls_omits_field() {
        let resp = ChatResponse {
            content: "Hello".into(),
            model: "gpt-4o".into(),
            usage: None,
            tool_calls: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("tool_calls"));
    }

    #[test]
    fn test_stream_chunk_with_delta_tool_calls() {
        let chunk = StreamChunk {
            content: String::new(),
            thinking: String::new(),
            done: false,
            model: Some("gpt-4o".into()),
            usage: None,
            delta_tool_calls: Some(vec![DeltaToolCall {
                index: 0,
                id: Some("call_1".into()),
                tool_type: Some("function".into()),
                function: Some(DeltaToolCallFunction {
                    name: Some("read".into()),
                    arguments: Some(r#"{"fileP"#.into()),
                }),
            }]),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"delta_tool_calls\""));

        let parsed: StreamChunk = serde_json::from_str(&json).unwrap();
        assert!(parsed.delta_tool_calls.is_some());
        let deltas = parsed.delta_tool_calls.unwrap();
        assert_eq!(deltas[0].index, 0);
        assert_eq!(
            deltas[0].function.as_ref().unwrap().name.as_deref(),
            Some("read")
        );
    }

    #[test]
    fn test_tool_definitions_json() {
        let tools = vec![ToolDefinition {
            tool_type: "function".into(),
            function: ToolFunctionDef {
                name: "test_tool".into(),
                description: "A test".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "input": { "type": "string" }
                    },
                    "required": ["input"],
                }),
            },
        }];
        let json = serde_json::to_string_pretty(&tools).unwrap();
        assert!(json.contains("\"name\": \"test_tool\""));
        assert!(json.contains("\"required\": ["));
    }
}
