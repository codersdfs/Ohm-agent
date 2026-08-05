//! Amazon Bedrock provider — OpenAI-compatible transport.

use super::{ChatRequest, ChatResponse, LlmProvider};
use async_trait::async_trait;
use std::sync::Arc;

use crate::{ProviderConfig, StreamChunk};

pub struct BedrockProvider {
    config: Arc<ProviderConfig>,
}

impl BedrockProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        Self {
            config: Arc::new(config.clone()),
        }
    }
}

#[async_trait]
impl LlmProvider for BedrockProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, String> {
        Err("Bedrock provider not yet implemented".into())
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
        _tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), String> {
        Err("Bedrock provider not yet implemented".into())
    }
}
