pub mod openai;
pub mod anthropic;
pub mod google;
pub mod local;

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
            "local" => Self::Local,
            "ollama" => Self::Local,
            _ => Self::OpenAI,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Anthropic, Self::OpenAI, Self::Google, Self::Mistral,
            Self::XAI, Self::Cerebras, Self::Azure, Self::Bedrock,
            Self::HuggingFace, Self::Groq, Self::Kimi, Self::MiniMax,
            Self::OpenRouter, Self::Local,
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
        }
    }

    pub fn is_openai_compatible(&self) -> bool {
        matches!(self, Self::OpenAI | Self::XAI | Self::Cerebras | Self::Groq
            | Self::Kimi | Self::MiniMax | Self::OpenRouter | Self::Azure
            | Self::Bedrock | Self::HuggingFace | Self::Mistral)
    }

    pub fn supports_streaming(&self) -> bool {
        matches!(self, Self::OpenAI | Self::XAI | Self::Cerebras | Self::Groq
            | Self::Kimi | Self::MiniMax | Self::OpenRouter | Self::Azure
            | Self::Bedrock | Self::HuggingFace | Self::Mistral | Self::Local)
    }

    /// Default context window size (input + output) for this provider's models.
    /// Used for the context-length indicator.
    pub fn context_window(&self) -> u64 {
        match self {
            Self::OpenAI | Self::XAI | Self::Cerebras | Self::Groq
            | Self::Kimi | Self::MiniMax | Self::OpenRouter | Self::Azure
            | Self::Bedrock | Self::HuggingFace | Self::Mistral | Self::Local => 128_000,
            Self::Anthropic => 200_000,
            Self::Google => 1_048_576,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Thinking/reasoning content (model-internal reasoning, not visible output).
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
    async fn chat_stream(&self, request: ChatRequest, tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>) -> Result<(), String>;
}

pub fn create_provider(config: &ProviderConfig) -> Result<Box<dyn LlmProvider>, String> {
    let api_key = config.api_key.clone().unwrap_or_default();
    let base_url = config.base_url.clone();

    match config.kind {
        ProviderKind::OpenAI | ProviderKind::XAI | ProviderKind::Cerebras
        | ProviderKind::Groq | ProviderKind::Kimi | ProviderKind::MiniMax
        | ProviderKind::OpenRouter | ProviderKind::Azure | ProviderKind::Bedrock
        | ProviderKind::HuggingFace | ProviderKind::Mistral => {
            let url = base_url.clone().unwrap_or_else(|| match config.kind {
                ProviderKind::OpenAI => "https://api.openai.com/v1".into(),
                ProviderKind::XAI => "https://api.x.ai/v1".into(),
                ProviderKind::Cerebras => "https://api.cerebras.ai/v1".into(),
                ProviderKind::Groq => "https://api.groq.com/openai/v1".into(),
                ProviderKind::Kimi => "https://api.moonshot.cn/v1".into(),
                ProviderKind::MiniMax => "https://api.minimax.chat/v1".into(),
                ProviderKind::OpenRouter => "https://openrouter.ai/api/v1".into(),
                ProviderKind::Azure => "https://YOUR_RESOURCE.openai.azure.com/v1".into(),
                ProviderKind::Bedrock => "https://bedrock-runtime.YOUR_REGION.amazonaws.com".into(),
                ProviderKind::HuggingFace => "https://api-inference.huggingface.co/v1".into(),
                ProviderKind::Mistral => "https://api.mistral.ai/v1".into(),
                _ => unreachable!(),
            });
            Ok(Box::new(openai::OpenAIProvider::new(api_key, url)))
        }
        ProviderKind::Anthropic => {
            Ok(Box::new(anthropic::AnthropicProvider::new(api_key, base_url)))
        }
        ProviderKind::Google => {
            Ok(Box::new(google::GoogleProvider::new(api_key, base_url)))
        }
        ProviderKind::Local => {
            let mut url = base_url.unwrap_or_else(|| "http://127.0.0.1:11434".into());
            if !url.ends_with("/v1") {
                url = format!("{}/v1", url.trim_end_matches('/'));
            }
            Ok(Box::new(local::LocalProvider::new(url)))
        }
    }
}

// ─── Model fetching ─────────────────────────────────────────────────────────

pub async fn fetch_models(config: &ProviderConfig) -> Result<Vec<ModelInfo>, String> {
    let base_url = config.base_url.clone().unwrap_or_else(|| config.kind.default_base_url());

    if config.kind.is_openai_compatible() {
        fetch_openai_compatible_models(&base_url, config.api_key.as_deref()).await
    } else {
        match config.kind {
            ProviderKind::Local => {
                match fetch_local_models(&base_url).await {
                    Ok(models) if !models.is_empty() => Ok(models),
                    _ => fetch_openai_compatible_models(&format!("{}/v1", base_url.trim_end_matches('/')), None).await,
                }
            }
            ProviderKind::Google => {
                fetch_google_models(&base_url, config.api_key.as_deref()).await
            }
            ProviderKind::Anthropic => {
                fetch_anthropic_models(&base_url, config.api_key.as_deref()).await
            }
            _ => fetch_openai_compatible_models(&base_url, config.api_key.as_deref()).await,
        }
    }
}

async fn fetch_openai_compatible_models(base_url: &str, api_key: Option<&str>) -> Result<Vec<ModelInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let urls = vec![
        format!("{}/models", base_url.trim_end_matches('/')),
        format!("{}/v1/models", base_url.trim_end_matches('/')),
    ];

    for url in urls {
        let mut req = client.get(&url);
        if let Some(key) = api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
                let provider_name = extract_provider_name(&url);

                if let Some(models) = data.get("data").and_then(|d| d.as_array()) {
                    let mut result: Vec<ModelInfo> = models.iter().filter_map(|m| {
                        let id = m.get("id").and_then(|v| v.as_str())?.to_string();
                        let name = m.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                        Some(ModelInfo { id, name, provider: provider_name.clone() })
                    }).collect();
                    if !result.is_empty() {
                        result.sort_by(|a, b| a.id.cmp(&b.id));
                        return Ok(dedup_models(result));
                    }
                }
            }
            _ => continue,
        }
    }

    Err("No models endpoint responded".into())
}

async fn fetch_local_models(base_url: &str) -> Result<Vec<ModelInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let resp = client.get(&url).send().await.map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API error {}: {}", resp.status(), resp.text().await.unwrap_or_default()));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    let mut result = Vec::new();

    if let Some(models) = data.get("models").and_then(|d| d.as_array()) {
        for m in models {
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let model_name = m.get("model").and_then(|v| v.as_str());
            result.push(ModelInfo {
                id: name.to_string(),
                name: model_name.map(|s| s.to_string()),
                provider: "local".into(),
            });
        }
    }

    if result.is_empty() {
        return Err("No models found in Ollama response".into());
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

async fn fetch_google_models(base_url: &str, api_key: Option<&str>) -> Result<Vec<ModelInfo>, String> {
    let key = api_key.ok_or_else(|| "API key required for Google provider".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/v1beta/models?key={}", base_url.trim_end_matches('/'), key);
    let resp = client.get(&url).send().await.map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API error {}: {}", resp.status(), resp.text().await.unwrap_or_default()));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    let mut result = Vec::new();

    if let Some(models) = data.get("models").and_then(|d| d.as_array()) {
        for m in models {
            if let Some(id) = m.get("name").and_then(|v| v.as_str()) {
                let name = m.get("displayName").and_then(|v| v.as_str())
                    .or_else(|| m.get("description").and_then(|v| v.as_str()));
                result.push(ModelInfo {
                    id: id.to_string(),
                    name: name.map(|s| s.to_string()),
                    provider: "google".into(),
                });
            }
        }
    }

    if result.is_empty() {
        return Err("No models found in Google response".into());
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

async fn fetch_anthropic_models(base_url: &str, api_key: Option<&str>) -> Result<Vec<ModelInfo>, String> {
    let key = api_key.ok_or_else(|| "API key required for Anthropic provider".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let resp = client.get(&url)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API error {}: {}", resp.status(), resp.text().await.unwrap_or_default()));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    let mut result = Vec::new();

    if let Some(models) = data.get("data").and_then(|d| d.as_array()) {
        for m in models {
            if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
                let name = m.get("name").and_then(|v| v.as_str())
                    .or_else(|| m.get("display_name").and_then(|v| v.as_str()));
                result.push(ModelInfo {
                    id: id.to_string(),
                    name: name.map(|s| s.to_string()),
                    provider: "anthropic".into(),
                });
            }
        }
    }

    if result.is_empty() {
        return Err("No models found in Anthropic response".into());
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

fn dedup_models(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    models.into_iter().filter(|m| seen.insert(m.id.clone())).collect()
}

fn extract_provider_name(url: &str) -> String {
    if url.contains("openai") { "openai".into() }
    else if url.contains("anthropic") { "anthropic".into() }
    else if url.contains("x.ai") || url.contains("xai") { "xai".into() }
    else if url.contains("cerebras") { "cerebras".into() }
    else if url.contains("groq") { "groq".into() }
    else if url.contains("moonshot") || url.contains("kimi") { "kimi".into() }
    else if url.contains("minimax") { "minimax".into() }
    else if url.contains("openrouter") { "openrouter".into() }
    else if url.contains("azure") { "azure".into() }
    else if url.contains("bedrock") { "bedrock".into() }
    else if url.contains("huggingface") { "huggingface".into() }
    else if url.contains("mistral") { "mistral".into() }
    else if url.contains("google") || url.contains("generativelanguage") { "google".into() }
    else { "unknown".into() }
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
                    arguments: r#"{"filePath": "test.txt", "oldString": "foo", "newString": "bar"}"#.into(),
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
        assert_eq!(deltas[0].function.as_ref().unwrap().name.as_deref(), Some("read"));
    }

    #[test]
    fn test_tool_definitions_json() {
        let tools = vec![
            ToolDefinition {
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
            },
        ];
        let json = serde_json::to_string_pretty(&tools).unwrap();
        assert!(json.contains("\"name\": \"test_tool\""));
        assert!(json.contains("\"required\": ["));
    }
}
