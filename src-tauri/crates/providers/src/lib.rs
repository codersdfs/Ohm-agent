//! Provider abstraction — 14 LLM providers via a unified `LlmProvider` trait.
//!
//! Re-exports shared types from [`types`] and per-provider transports from
//! [`anthropic`], [`openai`], [`bedrock`], and [`local`]. Model discovery lives
//! in [`models`]; provider selection routing in [`router`].

pub mod anthropic;
pub mod bedrock;
pub mod local;
pub mod models;
pub mod openai;
pub mod protocol;
pub mod router;
pub mod types;

pub use models::fetch_models;
pub use protocol::*;
pub use router::*;
pub use types::*;

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, String>;
    async fn chat_stream(
        &self,
        request: ChatRequest,
        tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), String>;
}

pub fn create_provider(config: &ProviderConfig) -> Result<Box<dyn LlmProvider>, String> {
    let api_key = config.api_key.clone().unwrap_or_default();
    let base_url = config.base_url.clone();

    match config.kind {
        ProviderKind::OpenAI
        | ProviderKind::XAI
        | ProviderKind::Cerebras
        | ProviderKind::Groq
        | ProviderKind::Kimi
        | ProviderKind::MiniMax
        | ProviderKind::OpenRouter
        | ProviderKind::Azure
        | ProviderKind::HuggingFace
        | ProviderKind::Mistral
        | ProviderKind::Custom => {
            let url = base_url.clone().unwrap_or_else(|| match config.kind {
                ProviderKind::OpenAI => "https://api.openai.com/v1".into(),
                ProviderKind::XAI => "https://api.x.ai/v1".into(),
                ProviderKind::Cerebras => "https://api.cerebras.ai/v1".into(),
                ProviderKind::Groq => "https://api.groq.com/openai/v1".into(),
                ProviderKind::Kimi => "https://api.moonshot.cn/v1".into(),
                ProviderKind::MiniMax => "https://api.minimax.chat/v1".into(),
                ProviderKind::OpenRouter => "https://openrouter.ai/api/v1".into(),
                ProviderKind::Azure => "https://YOUR_RESOURCE.openai.azure.com/v1".into(),
                ProviderKind::HuggingFace => "https://api-inference.huggingface.co/v1".into(),
                ProviderKind::Mistral => "https://api.mistral.ai/v1".into(),
                ProviderKind::Custom => "https://your-endpoint/v1".into(),
                _ => unreachable!(),
            });
            Ok(Box::new(openai::OpenAIProvider::new(api_key, url)))
        }
        ProviderKind::Anthropic => Ok(Box::new(anthropic::AnthropicProvider::new(
            api_key, base_url,
        ))),
        ProviderKind::Google => Ok(Box::new(openai::OpenAIProvider::new(
            api_key,
            base_url.unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta/openai".into()),
        ))),
        ProviderKind::Bedrock => Ok(Box::new(bedrock::BedrockProvider::new(config))),
        ProviderKind::Local => {
            let mut url = base_url.unwrap_or_else(|| "http://127.0.0.1:11434".into());
            if !url.ends_with("/v1") {
                url = format!("{}/v1", url.trim_end_matches('/'));
            }
            Ok(Box::new(local::LocalProvider::new(url)))
        }
    }
}
