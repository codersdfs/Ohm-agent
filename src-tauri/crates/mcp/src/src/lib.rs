// Hermes Memory — Three-layer (session, project, user) with SQLite + FTS5 + embeddings
// Every interaction is stored and retrievable via semantic search.

pub mod session;
pub mod project;
pub mod user;
pub mod embed;

use serde::{Deserialize, Serialize};
use rusqlite::{params, Connection};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub layer: MemoryLayer,
    pub key: String,
    pub value: String,
    pub embedding: Option<Vec<f32>>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryLayer {
    Session,
    Project,
    User,
}

impl MemoryLayer {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Project => "project",
            Self::User => "user",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "session" => Self::Session,
            "project" => Self::Project,
            _ => Self::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entries: Vec<MemoryEntry>,
    pub relevance: Vec<f64>,
}

/// Unified memory store backed by SQLite with FTS5 full-text search
/// and n-gram based embedding similarity.
pub struct MemoryStore {
    conn: Connection,
    embedding: embed::EmbeddingEngine,
}

impl MemoryStore {
    /// Open or create the SQLite database at `db_path`.
    /// Initializes schema (memory table + FTS5 virtual table) if needed.
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open memory db: {}", e))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory (
                id TEXT PRIMARY KEY,
                layer TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                embedding BLOB,
                timestamp TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_layer ON memory(layer);
            CREATE INDEX IF NOT EXISTS idx_memory_key ON memory(key);
            CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
                key, value, content='memory', content_rowid='rowid'
            );
            CREATE TRIGGER IF NOT EXISTS memory_ai AFTER INSERT ON memory BEGIN
                INSERT INTO memory_fts(rowid, key, value) VALUES (new.rowid, new.key, new.value);
            END;
            CREATE TRIGGER IF NOT EXISTS memory_ad AFTER DELETE ON memory BEGIN
                INSERT INTO memory_fts(memory_fts, rowid, key, value) VALUES('delete', old.rowid, old.key, old.value);
            END;
            CREATE TRIGGER IF NOT EXISTS memory_au AFTER UPDATE ON memory BEGIN
                INSERT INTO memory_fts(memory_fts, rowid, key, value) VALUES('delete', old.rowid, old.key, old.value);
                INSERT INTO memory_fts(rowid, key, value) VALUES (new.rowid, new.key, new.value);
            END;"
        ).map_err(|e| format!("Failed to initialize memory schema: {}", e))?;

        Ok(Self {
            conn,
            embedding: embed::EmbeddingEngine::new(),
        })
    }

    /// Store a memory entry. Generates embedding and persists to SQLite.
    pub fn store(&self, layer: MemoryLayer, key: &str, value: &str) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let embedding = self.embedding.embed(value)
            .map(|v| {
                let bytes: Vec<u8> = v.iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect();
                Some(bytes)
            })
            .unwrap_or(None);

        self.conn.execute(
            "INSERT INTO memory (id, layer, key, value, embedding, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                layer.as_str(),
                key,
                value,
                embedding,
                timestamp,
            ],
        ).map_err(|e| format!("Failed to store memory: {}", e))?;

        log::info!("MemoryStore: stored {:?} key={}", layer, key);
        Ok(id)
    }

    /// Search memory using FTS5 full-text search.
    /// Results are combined with embedding similarity for ranking.
    pub fn search(&self, query: &str, layer: Option<&str>, limit: usize) -> Result<SearchResult, String> {
        let query_vec = self.embedding.embed(query).unwrap_or_default();
        let fts_query = query.split_whitespace()
            .map(|w| format!("\"{}\"", w))
            .collect::<Vec<_>>()
            .join(" OR ");

        let mut entries: Vec<MemoryEntry> = Vec::new();
        let mut relevances: Vec<f64> = Vec::new();

        // Try FTS5 search first
        let fts_found = match layer {
            Some(l) => self.search_fts(&fts_query, Some(l), limit, &query_vec, &mut entries, &mut relevances),
            None => self.search_fts(&fts_query, None, limit, &query_vec, &mut entries, &mut relevances),
        };

        // Fall back to full scan
        if !fts_found {
            entries.clear();
            relevances.clear();
            match layer {
                Some(l) => self.search_scan(Some(l), limit, &query_vec, &mut entries, &mut relevances),
                None => self.search_scan(None, limit, &query_vec, &mut entries, &mut relevances),
            }
        }

        Ok(SearchResult { entries, relevance: relevances })
    }

    fn search_fts(
        &self,
        fts_query: &str,
        layer: Option<&str>,
        limit: usize,
        query_vec: &[f32],
        entries: &mut Vec<MemoryEntry>,
        relevances: &mut Vec<f64>,
    ) -> bool {
        let sql = match layer {
            Some(_) => "SELECT m.id, m.layer, m.key, m.value, m.embedding, m.timestamp
                         FROM memory_fts f JOIN memory m ON f.rowid = m.rowid
                         WHERE m.layer = ?1 AND memory_fts MATCH ?2
                         ORDER BY rank LIMIT ?3",
            None => "SELECT m.id, m.layer, m.key, m.value, m.embedding, m.timestamp
                     FROM memory_fts f JOIN memory m ON f.rowid = m.rowid
                     WHERE memory_fts MATCH ?1
                     ORDER BY rank LIMIT ?2",
        };

        let mut stmt = match self.conn.prepare(sql) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let result = match layer {
            Some(l) => stmt.query_map(params![l, fts_query, limit as i64], Self::map_entry),
            None => stmt.query_map(params![fts_query, limit as i64], Self::map_entry),
        };

        match result {
            Ok(rows) => {
                let mut found = false;
                for row in rows.flatten() {
                    found = true;
                    let relevance = row.embedding.as_ref()
                        .and_then(|emb| self.embedding.similarity(query_vec, emb).ok())
                        .unwrap_or(0.0);
                    entries.push(row);
                    relevances.push(relevance);
                }
                found
            }
            Err(_) => false,
        }
    }

    fn search_scan(
        &self,
        layer: Option<&str>,
        limit: usize,
        query_vec: &[f32],
        entries: &mut Vec<MemoryEntry>,
        relevances: &mut Vec<f64>,
    ) {
        let sql = match layer {
            Some(_) => "SELECT id, layer, key, value, embedding, timestamp FROM memory WHERE layer = ?1 ORDER BY timestamp DESC LIMIT ?2",
            None => "SELECT id, layer, key, value, embedding, timestamp FROM memory ORDER BY timestamp DESC LIMIT ?2",
        };

        let mut stmt = match self.conn.prepare(sql) {
            Ok(s) => s,
            Err(_) => return,
        };

        let result = match layer {
            Some(l) => stmt.query_map(params![l, limit as i64], Self::map_entry),
            None => stmt.query_map(params![limit as i64], Self::map_entry),
        };

        if let Ok(rows) = result {
            for row in rows.flatten() {
                let relevance = row.embedding.as_ref()
                    .and_then(|emb| self.embedding.similarity(query_vec, emb).ok())
                    .unwrap_or(0.0);
                entries.push(row);
                relevances.push(relevance);
            }
        }

        // Sort by relevance descending
        let mut paired: Vec<(usize, f64)> = (0..entries.len()).map(|i| (i, relevances[i])).collect();
        paired.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let sorted_entries: Vec<MemoryEntry> = paired.iter().map(|&(i, _)| entries[i].clone()).collect();
        let sorted_relevances: Vec<f64> = paired.iter().map(|&(_, r)| r).collect();
        *entries = sorted_entries;
        *relevances = sorted_relevances;
    }

    /// Remember a value by exact key lookup.
    pub fn remember(&self, key: &str, layer: Option<&str>) -> Result<Option<String>, String> {
        let sql = match layer {
            Some(_) => "SELECT value FROM memory WHERE key = ?1 AND layer = ?2 ORDER BY timestamp DESC LIMIT 1",
            None => "SELECT value FROM memory WHERE key = ?1 ORDER BY timestamp DESC LIMIT 1",
        };

        let mut stmt = self.conn.prepare(sql)
            .map_err(|e| format!("Failed to prepare: {}", e))?;

        let result = match layer {
            Some(l) => stmt.query_row(params![key, l], |row| row.get::<_, String>(0)),
            None => stmt.query_row(params![key], |row| row.get::<_, String>(0)),
        };

        match result {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to remember: {}", e)),
        }
    }

    /// Get total memory entry count, optionally filtered by layer.
    pub fn count(&self, layer: Option<&str>) -> Result<usize, String> {
        let sql = match layer {
            Some(_) => "SELECT COUNT(*) FROM memory WHERE layer = ?1",
            None => "SELECT COUNT(*) FROM memory",
        };

        let mut stmt = self.conn.prepare(sql)
            .map_err(|e| format!("Failed to prepare: {}", e))?;

        match layer {
            Some(l) => stmt.query_row(params![l], |row| row.get::<_, usize>(0)),
            None => stmt.query_row([], |row| row.get::<_, usize>(0)),
        }.map_err(|e| format!("Failed to count: {}", e))
    }

    /// Delete a memory entry by ID.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.conn.execute("DELETE FROM memory WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete: {}", e))?;
        Ok(())
    }

    /// Clear all memory entries, optionally for a specific layer.
    pub fn clear(&self, layer: Option<&str>) -> Result<usize, String> {
        let count = match layer {
            Some(l) => {
                self.conn.execute("DELETE FROM memory WHERE layer = ?1", params![l])
                    .map_err(|e| format!("Failed to clear: {}", e))?
            }
            None => {
                self.conn.execute("DELETE FROM memory", [])
                    .map_err(|e| format!("Failed to clear: {}", e))?
            }
        };
        Ok(count)
    }

    fn map_entry(row: &rusqlite::Row) -> rusqlite::Result<MemoryEntry> {
        let embedding_blob: Option<Vec<u8>> = row.get(4)?;
        let embedding = embedding_blob.map(|blob| {
            blob.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect::<Vec<f32>>()
        });

        Ok(MemoryEntry {
            id: row.get(0)?,
            layer: MemoryLayer::from_str(&row.get::<_, String>(1)?),
            key: row.get(2)?,
            value: row.get(3)?,
            embedding,
            timestamp: row.get(5)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> MemoryStore {
        MemoryStore::new(":memory:").unwrap()
    }

    #[test]
    fn test_store_and_remember() {
        let store = test_store();
        let id = store.store(MemoryLayer::Session, "test_key", "hello world").unwrap();
        assert!(!id.is_empty());

        let result = store.remember("test_key", Some("session")).unwrap();
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn test_search_returns_results() {
        let store = test_store();
        store.store(MemoryLayer::Project, "api_key", "sk-abc123").unwrap();
        store.store(MemoryLayer::Project, "db_url", "postgres://localhost").unwrap();

        let results = store.search("api", Some("project"), 10).unwrap();
        assert!(!results.entries.is_empty(), "Should find at least one result");
    }

    #[test]
    fn test_count() {
        let store = test_store();
        store.store(MemoryLayer::Session, "a", "1").unwrap();
        store.store(MemoryLayer::Project, "b", "2").unwrap();
        store.store(MemoryLayer::User, "c", "3").unwrap();

        assert_eq!(store.count(None).unwrap(), 3);
        assert_eq!(store.count(Some("session")).unwrap(), 1);
    }

    #[test]
    fn test_delete() {
        let store = test_store();
        let id = store.store(MemoryLayer::Session, "x", "y").unwrap();
        store.delete(&id).unwrap();
        assert_eq!(store.count(None).unwrap(), 0);
    }

    #[test]
    fn test_clear_layer() {
        let store = test_store();
        store.store(MemoryLayer::Session, "a", "1").unwrap();
        store.store(MemoryLayer::Project, "b", "2").unwrap();
        store.clear(Some("session")).unwrap();
        assert_eq!(store.count(None).unwrap(), 1);
    }

    #[test]
    fn test_search_across_layers() {
        let store = test_store();
        store.store(MemoryLayer::Session, "config", "dark_mode").unwrap();
        store.store(MemoryLayer::User, "config", "light_mode").unwrap();

        let all = store.search("config", None, 10).unwrap();
        assert_eq!(all.entries.len(), 2);
    }
}
