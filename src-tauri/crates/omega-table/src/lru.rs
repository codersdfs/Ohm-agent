use std::collections::HashMap;
use std::time::{Duration, Instant};

const DEFAULT_CAPACITY: usize = 100;
const DEFAULT_TTL: Duration = Duration::from_secs(300);

struct CacheEntry {
    data: Vec<u8>,
    loaded_at: Instant,
    access_count: u64,
}

pub struct TableCache {
    entries: HashMap<String, CacheEntry>,
    capacity: usize,
    ttl: Duration,
}

impl TableCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            capacity: DEFAULT_CAPACITY,
            ttl: DEFAULT_TTL,
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&Vec<u8>> {
        if let Some(entry) = self.entries.get(key) {
            if entry.loaded_at.elapsed() > self.ttl {
                self.entries.remove(key);
                return None;
            }
        }
        let entry = self.entries.get_mut(key)?;
        entry.access_count += 1;
        Some(&entry.data)
    }

    pub fn set(&mut self, key: String, data: Vec<u8>) {
        if self.entries.len() >= self.capacity {
            if let Some(evict_key) = self.evict_one() {
                self.entries.remove(&evict_key);
            }
        }
        self.entries.insert(key, CacheEntry {
            data,
            loaded_at: Instant::now(),
            access_count: 0,
        });
    }

    fn evict_one(&self) -> Option<String> {
        self.entries.iter()
            .min_by_key(|(_, e)| e.access_count)
            .map(|(k, _)| k.clone())
    }
}
