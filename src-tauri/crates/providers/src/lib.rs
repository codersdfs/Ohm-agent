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
    fn as_any(&self) -> &dyn std::any::Any;
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
            base_url.unwrap_or_else(|| {
                "https://generativelanguage.googleapis.com/v1beta/openai".into()
            }),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn config_for(kind: ProviderKind) -> ProviderConfig {
        ProviderConfig {
            kind,
            api_key: Some("test-key".into()),
            base_url: None,
            model: "test-model".into(),
            max_tokens: 1024,
            temperature: 0.5,
            max_concurrent_tools: 3,
        }
    }

    // ---- create_provider routing ----

    #[test]
    fn create_provider_openai_routes_to_openai() {
        let p = create_provider(&config_for(ProviderKind::OpenAI)).unwrap();
        assert!(p
            .as_any()
            .downcast_ref::<openai::OpenAIProvider>()
            .is_some());
    }

    #[test]
    fn create_provider_anthropic_routes_to_anthropic() {
        let p = create_provider(&config_for(ProviderKind::Anthropic)).unwrap();
        assert!(p
            .as_any()
            .downcast_ref::<anthropic::AnthropicProvider>()
            .is_some());
    }

    #[test]
    fn create_provider_local_routes_to_local() {
        let p = create_provider(&config_for(ProviderKind::Local)).unwrap();
        assert!(p.as_any().downcast_ref::<local::LocalProvider>().is_some());
    }

    #[test]
    fn create_provider_bedrock_routes_to_bedrock() {
        let p = create_provider(&config_for(ProviderKind::Bedrock)).unwrap();
        assert!(p
            .as_any()
            .downcast_ref::<bedrock::BedrockProvider>()
            .is_some());
    }

    #[test]
    fn create_provider_google_routes_to_openai_compatible() {
        // Google should route through OpenAIProvider (not a separate GoogleProvider)
        let p = create_provider(&config_for(ProviderKind::Google)).unwrap();
        assert!(p
            .as_any()
            .downcast_ref::<openai::OpenAIProvider>()
            .is_some());
    }

    #[test]
    fn create_provider_bedrock_not_openai() {
        // Bedrock must NOT route through OpenAIProvider
        let p = create_provider(&config_for(ProviderKind::Bedrock)).unwrap();
        assert!(p
            .as_any()
            .downcast_ref::<openai::OpenAIProvider>()
            .is_none());
    }

    // ---- from_name ----

    #[test]
    fn from_name_all_variants() {
        for kind in ProviderKind::all() {
            let name = format!("{}", kind);
            let parsed = ProviderKind::from_name(&name).unwrap();
            assert_eq!(
                format!("{}", parsed),
                name,
                "round-trip failed for {}",
                name
            );
        }
    }

    #[test]
    fn from_name_aliases() {
        assert_eq!(
            ProviderKind::from_name("ollama").unwrap(),
            ProviderKind::Local
        );
        assert_eq!(
            ProviderKind::from_name("openai-compatible").unwrap(),
            ProviderKind::Custom
        );
        assert_eq!(
            ProviderKind::from_name("other").unwrap(),
            ProviderKind::Custom
        );
    }

    #[test]
    fn from_name_unknown_returns_err() {
        assert!(ProviderKind::from_name("bogus").is_err());
        assert!(ProviderKind::from_name("nonexistent").is_err());
        assert!(ProviderKind::from_name("").is_err());
    }

    #[test]
    fn from_str_trait_uses_from_name() {
        // std::str::FromStr delegates to from_name
        assert_eq!(
            "openai".parse::<ProviderKind>().unwrap(),
            ProviderKind::OpenAI
        );
        assert!("bogus".parse::<ProviderKind>().is_err());
    }

    // ---- context_window ----

    #[test]
    fn context_window_google_is_200k() {
        assert_eq!(ProviderKind::Google.context_window(), 200_000);
    }

    #[test]
    fn context_window_bedrock() {
        assert_eq!(ProviderKind::Bedrock.context_window(), 128_000);
    }

    // ---- is_openai_compatible ----

    #[test]
    fn bedrock_not_openai_compatible() {
        assert!(!ProviderKind::Bedrock.is_openai_compatible());
    }

    #[test]
    fn google_not_openai_compatible() {
        // Google routes through OpenAIProvider but isn't listed in is_openai_compatible
        assert!(!ProviderKind::Google.is_openai_compatible());
    }
}
