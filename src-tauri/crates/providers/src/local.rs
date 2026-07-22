use crate::openai::OpenAIProvider;
use crate::{ChatRequest, ChatResponse, LlmProvider, StreamChunk};

pub struct LocalProvider {
    inner: OpenAIProvider,
}

impl LocalProvider {
    pub fn new(base_url: String) -> Self {
        Self {
            inner: OpenAIProvider::new(String::new(), base_url),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for LocalProvider {
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
