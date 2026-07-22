use crate::openai::OpenAIProvider;
use crate::{ChatRequest, ChatResponse, LlmProvider, StreamChunk};

pub struct GoogleProvider {
    inner: OpenAIProvider,
}

impl GoogleProvider {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        let url = base_url.unwrap_or_else(|| "https://generativelanguage.googleapis.com".into());
        let openai_url = format!("{}/v1beta/openai", url.trim_end_matches('/'));
        Self {
            inner: OpenAIProvider::new(api_key, openai_url),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for GoogleProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, String> {
        self.inner.chat(request).await
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
        tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), String> {
        self.inner.chat_stream(request, tx).await
    }
}
