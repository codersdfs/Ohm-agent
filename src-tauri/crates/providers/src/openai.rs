use crate::{ChatRequest, ChatResponse, LlmProvider};

pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAIProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, String> {
        Ok(ChatResponse {
            content: "OpenAI response".into(),
            model: "gpt-4".into(),
            usage: None,
        })
    }

    async fn chat_stream(&self, _request: ChatRequest) -> Result<String, String> {
        Ok("streamed".into())
    }
}
