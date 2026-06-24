mod providers_backend;

use crate::message::Message;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

pub use providers_backend::ProviderBackend;

#[async_trait]
pub trait ChatBackend: Send + Sync {
    async fn stream_chat(
        &self,
        history: &[Message],
        tx: UnboundedSender<String>,
        cycle_idx: usize,
    ) -> Result<(), String>;
}

pub struct MockBackend {
    responses: Vec<String>,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self {
            responses: vec![
                "Rust's ownership system guarantees memory safety without a garbage collector. \
                 Every value has exactly one owner, and the borrow checker ensures no data \
                 races or dangling references at compile time."
                    .into(),
                "Async Rust uses the Future trait to represent deferred computation. When you \
                 call an async function, it returns a Future that can be polled by the runtime. \
                 Tokio is the most popular async executor in the ecosystem."
                    .into(),
                "Pattern matching in Rust is powerful and exhaustive. The match expression \
                 must cover every possible case, which eliminates an entire class of bugs. \
                 You can destructure enums, tuples, and structs directly in the arms."
                    .into(),
                "The Result<T, E> type is Rust's answer to error handling. Instead of \
                 exceptions, Rust uses the ? operator to propagate errors up the call chain. \
                 This makes error paths explicit and impossible to accidentally ignore."
                    .into(),
                "Cargo is Rust's build system and package manager. It handles compilation, \
                 dependency resolution, testing, benchmarks, and publishing to crates.io. \
                 Workspaces let you manage multiple related packages in one repository."
                    .into(),
                "Iterators in Rust are lazy and zero-cost abstractions. Methods like map, \
                 filter, and collect compose efficiently because the compiler inlines them \
                 into tight loops. They often outperform hand-written for loops."
                    .into(),
            ],
        }
    }
}

#[async_trait]
impl ChatBackend for MockBackend {
    async fn stream_chat(
        &self,
        history: &[Message],
        tx: UnboundedSender<String>,
        cycle_idx: usize,
    ) -> Result<(), String> {
        let response = &self.responses[cycle_idx % self.responses.len()];

        let user_input = history
            .iter()
            .rev()
            .find(|m| matches!(m.sender, crate::message::MessageSender::User))
            .map(|m| m.content.as_str())
            .unwrap_or("");

        let _ = user_input;

        let words: Vec<&str> = response.split_whitespace().collect();
        let mut current = String::new();
        let mut idx = 0;

        while idx < words.len() {
            let chunk_size = 1 + (idx % 3);
            let mut chunk_words = Vec::new();
            for w in words.iter().skip(idx).take(chunk_size) {
                chunk_words.push(*w);
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&chunk_words.join(" "));
            idx += chunk_words.len();

            if tx.send(current.clone()).is_err() {
                return Err("channel closed".into());
            }

            let delay = 30 + (idx % 5) * 15;
            tokio::time::sleep(tokio::time::Duration::from_millis(delay as u64)).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn mock_streams_chunks() {
        let backend = MockBackend::default();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let history = vec![Message::user("test")];

        let handle = tokio::spawn(async move { backend.stream_chat(&history, tx, 0).await });

        let mut chunks = Vec::new();
        while let Some(chunk) = rx.recv().await {
            chunks.push(chunk);
        }

        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert!(!chunks.is_empty());
    }

    #[test]
    fn mock_cycles_responses() {
        let backend = MockBackend::default();
        assert_eq!(backend.responses.len(), 6);
    }
}
