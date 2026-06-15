use crate::{ChatRequest, ChatResponse, LlmProvider};

pub struct GoogleProvider {
    api_key: String,
}

impl GoogleProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait::async_trait]
impl LlmProvider for GoogleProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, String> {
        Ok(ChatResponse {
            content: "Google response".into(),
            model: "gemini-2".into(),
            usage: None,
        })
    }

    async fn chat_stream(&self, _request: ChatRequest) -> Result<String, String> {
        Ok("streamed".into())
    }
}
