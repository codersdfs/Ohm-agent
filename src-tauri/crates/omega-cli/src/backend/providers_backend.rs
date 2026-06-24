use super::ChatBackend;
use crate::message::{Message, MessageSender};
use async_trait::async_trait;
use providers::{ChatMessage, ChatRequest, LlmProvider, StreamChunk, ProviderConfig};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct ProviderBackend {
    provider: Arc<dyn LlmProvider>,
    config: ProviderConfig,
}

impl ProviderBackend {
    pub fn new(provider: Arc<dyn LlmProvider>, config: ProviderConfig) -> Self {
        Self { provider, config }
    }

    pub fn config(&self) -> &ProviderConfig {
        &self.config
    }
}

fn messages_to_chat(history: &[Message]) -> Vec<ChatMessage> {
    history
        .iter()
        .map(|m| {
            let role = match m.sender {
                MessageSender::User => "user",
                MessageSender::Assistant => "assistant",
                MessageSender::System => "system",
                MessageSender::Tool => "user",
            };
            ChatMessage {
                role: role.to_string(),
                content: m.content.clone(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }
        })
        .collect()
}

#[async_trait]
impl ChatBackend for ProviderBackend {
    async fn stream_chat(
        &self,
        history: &[Message],
        tx: UnboundedSender<String>,
        _cycle_idx: usize,
    ) -> Result<(), String> {
        let chat_messages = messages_to_chat(history);

        let request = ChatRequest {
            messages: chat_messages,
            config: self.config.clone(),
            stream: true,
            tools: None,
        };

        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel::<StreamChunk>();
        let tx_clone = tx.clone();

        let forward_handle = tokio::spawn(async move {
            while let Some(chunk) = chunk_rx.recv().await {
                if !chunk.content.is_empty() {
                    if tx_clone.send(chunk.content).is_err() {
                        break;
                    }
                }
            }
        });

        let result = self.provider.chat_stream(request, chunk_tx).await;
        let _ = forward_handle.await;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    #[test]
    fn messages_to_chat_conversion() {
        let messages = vec![
            Message::user("hello"),
            Message::assistant("hi there", false),
            Message::system("welcome"),
        ];
        let chat = messages_to_chat(&messages);
        assert_eq!(chat.len(), 3);
        assert_eq!(chat[0].role, "user");
        assert_eq!(chat[0].content, "hello");
        assert_eq!(chat[1].role, "assistant");
        assert_eq!(chat[2].role, "system");
    }
}
