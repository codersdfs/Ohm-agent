use crate::{MemoryLayer, MemoryStore};

pub struct ProjectMemory<'a> {
    store: &'a MemoryStore,
}

impl<'a> ProjectMemory<'a> {
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    pub fn store(&self, key: &str, value: &str) -> Result<String, String> {
        self.store.store(MemoryLayer::Project, key, value)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<crate::SearchResult, String> {
        self.store.search(query, Some("project"), limit)
    }

    pub fn remember(&self, key: &str) -> Result<Option<String>, String> {
        self.store.remember(key, Some("project"))
    }
}
