use crate::{MemoryLayer, MemoryStore};

pub struct SessionMemory<'a> {
    store: &'a MemoryStore,
}

impl<'a> SessionMemory<'a> {
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    pub fn store(&self, key: &str, value: &str) -> Result<String, String> {
        self.store.store(MemoryLayer::Session, key, value)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<crate::SearchResult, String> {
        self.store.search(query, Some("session"), limit)
    }

    pub fn remember(&self, key: &str) -> Result<Option<String>, String> {
        self.store.remember(key, Some("session"))
    }
}
