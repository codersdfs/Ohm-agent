use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: Option<String>,
    pub provider: String,
}

impl ModelInfo {
    pub fn display_name(&self) -> String {
        match &self.name {
            Some(n) => format!("{} ({})", n, self.id),
            None => self.id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum ProviderKind {
    Anthropic,
    OpenAI,
    Google,
    Mistral,
    XAI,
    Cerebras,
    Azure,
    Bedrock,
    HuggingFace,
    Groq,
    Kimi,
    MiniMax,
    OpenRouter,
    Local,
    Custom,
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::Google => "google",
            Self::Mistral => "mistral",
            Self::XAI => "xai",
            Self::Cerebras => "cerebras",
            Self::Azure => "azure",
            Self::Bedrock => "bedrock",
            Self::HuggingFace => "huggingface",
            Self::Groq => "groq",
            Self::Kimi => "kimi",
            Self::MiniMax => "minimax",
            Self::OpenRouter => "openrouter",
            Self::Local => "local",
            Self::Custom => "custom",
        };
        write!(f, "{}", s)
    }
}

impl ProviderKind {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAI,
            "google" => Self::Google,
            "mistral" => Self::Mistral,
            "xai" => Self::XAI,
            "cerebras" => Self::Cerebras,
            "azure" => Self::Azure,
            "bedrock" => Self::Bedrock,
            "huggingface" => Self::HuggingFace,
            "groq" => Self::Groq,
            "kimi" => Self::Kimi,
            "minimax" => Self::MiniMax,
            "openrouter" => Self::OpenRouter,
            "local" | "ollama" => Self::Local,
            "custom" | "other" | "openai-compatible" => Self::Custom,
            _ => Self::OpenAI,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Anthropic,
            Self::OpenAI,
            Self::Google,
            Self::Mistral,
            Self::XAI,
            Self::Cerebras,
            Self::Azure,
            Self::Bedrock,
            Self::HuggingFace,
            Self::Groq,
            Self::Kimi,
            Self::MiniMax,
            Self::OpenRouter,
            Self::Local,
            Self::Custom,
        ]
    }

    pub fn default_base_url(&self) -> String {
        match self {
            Self::OpenAI => "https://api.openai.com/v1".into(),
            Self::Anthropic => "https://api.anthropic.com".into(),
            Self::Google => "https://generativelanguage.googleapis.com".into(),
            Self::Mistral => "https://api.mistral.ai".into(),
            Self::XAI => "https://api.x.ai/v1".into(),
            Self::Cerebras => "https://api.cerebras.ai/v1".into(),
            Self::Azure => "https://YOUR_RESOURCE.openai.azure.com/v1".into(),
            Self::Bedrock => "https://bedrock-runtime.YOUR_REGION.amazonaws.com".into(),
            Self::HuggingFace => "https://api-inference.huggingface.co/v1".into(),
            Self::Groq => "https://api.groq.com/openai/v1".into(),
            Self::Kimi => "https://api.moonshot.cn/v1".into(),
            Self::MiniMax => "https://api.minimax.chat/v1".into(),
            Self::OpenRouter => "https://openrouter.ai/api/v1".into(),
            Self::Local => "http://127.0.0.1:11434".into(),
            Self::Custom => "https://your-endpoint/v1".into(),
        }
    }

    pub fn is_openai_compatible(&self) -> bool {
        matches!(
            self,
            Self::OpenAI
                | Self::XAI
                | Self::Cerebras
                | Self::Groq
                | Self::Kimi
                | Self::MiniMax
                | Self::OpenRouter
                | Self::Azure
                | Self::Bedrock
                | Self::HuggingFace
                | Self::Mistral
                | Self::Custom
        )
    }

    pub fn supports_streaming(&self) -> bool {
        matches!(
            self,
            Self::OpenAI
                | Self::XAI
                | Self::Cerebras
                | Self::Groq
                | Self::Kimi
                | Self::MiniMax
                | Self::OpenRouter
                | Self::Azure
                | Self::Bedrock
                | Self::HuggingFace
                | Self::Mistral
                | Self::Local
                | Self::Custom
        )
    }

    pub fn context_window(&self) -> u64 {
        match self {
            Self::Anthropic => 200_000,
            Self::Google => 1_048_576,
            _ => 128_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::OpenAI,
            api_key: None,
            base_url: None,
            model: "llama3.1:8b".into(),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

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
    pub config: ProviderConfig,
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
    #[serde(default)]
    pub thinking: String,
    pub done: bool,
    pub model: Option<String>,
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_tool_calls: Option<Vec<DeltaToolCall>>,
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, String>;
    async fn chat_stream(
        &self,
        request: ChatRequest,
        tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), String>;
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
                    arguments: r#"{"filePath": "test.txt"}"#.into(),
                },
            }]),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"tool_calls\""));
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
    }
}
