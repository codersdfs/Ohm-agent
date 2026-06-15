use crate::{ChatRequest, ChatResponse, LlmProvider};

pub struct LocalProvider {
    base_url: String,
}

impl LocalProvider {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

#[async_trait::async_trait]
impl LlmProvider for LocalProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, String> {
        Ok(ChatResponse {
            content: "Local response".into(),
            model: "local".into(),
            usage: None,
        })
    }

    async fn chat_stream(&self, _request: ChatRequest) -> Result<String, String> {
        Ok("streamed".into())
    }
}
