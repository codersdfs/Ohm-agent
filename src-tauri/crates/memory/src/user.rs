use crate::MemoryEntry;

pub struct UserMemory;

impl UserMemory {
    pub fn new() -> Self {
        Self
    }

    pub fn store(&self, _entry: MemoryEntry) -> Result<String, String> {
        Ok(uuid::Uuid::new_v4().to_string())
    }

    pub fn search(&self, _query: &str) -> Result<Vec<MemoryEntry>, String> {
        Ok(vec![])
    }
}
