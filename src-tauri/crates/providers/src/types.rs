//! Provider configuration types — enums and config structs.
//! Split out of `lib.rs` (P5 god-object split).

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
    /// OpenAI-compatible endpoint with a user-supplied base URL.
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
            "local" => Self::Local,
            "ollama" => Self::Local,
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

    /// Default context window size (input + output) for this provider's models.
    /// Used for the context-length indicator.
    pub fn context_window(&self) -> u64 {
        match self {
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
            | Self::Custom => 128_000,
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
