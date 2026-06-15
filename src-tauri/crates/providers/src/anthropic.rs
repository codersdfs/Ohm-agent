use crate::{ChatRequest, ChatResponse, LlmProvider};

pub struct AnthropicProvider {
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, String> {
        Ok(ChatResponse {
            content: "Anthropic response".into(),
            model: "claude-3-5-sonnet".into(),
            usage: None,
        })
    }

    async fn chat_stream(&self, _request: ChatRequest) -> Result<String, String> {
        Ok("streamed".into())
    }
}
