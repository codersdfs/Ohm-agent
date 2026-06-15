use crate::{ChatRequest, ChatResponse, LlmProvider};

pub struct MistralProvider {
    api_key: String,
}

impl MistralProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait::async_trait]
impl LlmProvider for MistralProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, String> {
        Ok(ChatResponse {
            content: "Mistral response".into(),
            model: "mistral-large".into(),
            usage: None,
        })
    }

    async fn chat_stream(&self, _request: ChatRequest) -> Result<String, String> {
        Ok("streamed".into())
    }
}
