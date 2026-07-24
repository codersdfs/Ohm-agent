use crate::rules::RuleEntry;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureEvent {
    pub signature: String,
    pub raw: String,
    pub kind: String,
    pub fix_recipe: Option<String>,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total_patterns: usize,
    pub promoted: usize,
    pub recurrence_rate_before: f64,
    pub recurrence_rate_after: f64,
}

pub struct NegativeKnowledgeStore {
    conn: Connection,
}

impl NegativeKnowledgeStore {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open negative knowledge db: {}", e))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS failures (
                signature TEXT PRIMARY KEY,
                raw TEXT NOT NULL,
                kind TEXT NOT NULL,
                fix_recipe TEXT,
                count INTEGER NOT NULL DEFAULT 1,
                promoted INTEGER NOT NULL DEFAULT 0,
                first_seen TEXT NOT NULL,
                last_seen TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_failures_kind ON failures(kind);
            CREATE INDEX IF NOT EXISTS idx_failures_promoted ON failures(promoted);",
        )
        .map_err(|e| format!("Failed to init negative knowledge schema: {}", e))?;

        Ok(Self { conn })
    }

    /// Record a failure event. Returns true if the pattern was promoted (count >= 3).
    pub fn record_failure(&self, event: &FailureEvent) -> Result<bool, String> {
        let now = chrono::Utc::now().to_rfc3339();
        let signature = compute_signature(&event.signature);

        let result = self.conn.execute(
            "INSERT INTO failures (signature, raw, kind, fix_recipe, count, promoted, first_seen, last_seen)
             VALUES (?1, ?2, ?3, ?4, 1, 0, ?5, ?5)",
            params![signature, event.raw, event.kind, event.fix_recipe, now],
        );

        let promoted = match result {
            Ok(_) => false,
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                self.conn.execute(
                    "UPDATE failures SET count = count + 1, last_seen = ?1 WHERE signature = ?2",
                    params![now, signature],
                ).map_err(|e| format!("Failed to increment count: {}", e))?;

                let count: i64 = self.conn.query_row(
                    "SELECT count FROM failures WHERE signature = ?1",
                    params![signature],
                    |row| row.get(0),
                ).map_err(|e| format!("Failed to get count: {}", e))?;

                if count >= 3 {
                    self.conn.execute(
                        "UPDATE failures SET promoted = 1 WHERE signature = ?1",
                        params![signature],
                    ).map_err(|e| format!("Failed to promote: {}", e))?;
                    true
                } else {
                    false
                }
            }
            Err(e) => return Err(format!("Failed to record failure: {}", e)),
        };

        Ok(promoted)
    }

    /// Get all promoted rules.
    pub fn get_promoted_rules(&self) -> Result<Vec<RuleEntry>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT raw, kind, fix_recipe, count FROM failures WHERE promoted = 1",
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let mut rules = vec![];
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        }).map_err(|e| format!("Failed to query promoted rules: {}", e))?;

        for row in rows.flatten() {
            rules.push(RuleEntry {
                pattern: row.0.clone(),
                severity: "error".to_string(),
                message: row.0.clone(),
                tool_hint: row.2,
                frequency: row.3 as u32,
                promoted: true,
            });
        }

        Ok(rules)
    }

    /// Get statistics about the negative knowledge store.
    pub fn get_stats(&self) -> Result<Stats, String> {
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM failures",
            [],
            |row| row.get(0),
        ).map_err(|e| format!("Failed to count patterns: {}", e))?;

        let promoted: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM failures WHERE promoted = 1",
            [],
            |row| row.get(0),
        ).map_err(|e| format!("Failed to count promoted: {}", e))?;

        let avg_count: f64 = self.conn.query_row(
            "SELECT AVG(count) FROM failures",
            [],
            |row| row.get::<_, f64>(0),
        ).unwrap_or(0.0);

        Ok(Stats {
            total_patterns: total as usize,
            promoted: promoted as usize,
            recurrence_rate_before: avg_count,
            recurrence_rate_after: if promoted > 0 { 0.0 } else { avg_count },
        })
    }

    /// Inject promoted rules into the rules database.
    pub fn inject_into_rules_db(&self, db: &mut crate::rules::RulesDatabase, lang: &crate::Language) -> Result<u32, String> {
        let rules = self.get_promoted_rules()?;
        let mut injected = 0u32;

        for rule in rules {
            db.promote_or_increment(lang, "golden", &rule.pattern, &rule.message, "error");
            injected += 1;
        }

        Ok(injected)
    }
}

/// Normalize a message by stripping paths, line numbers, and temporary IDs.
pub fn normalize_message(msg: &str) -> String {
    let mut result = msg.to_string();

    // Strip file paths (src/main.rs:42:)
    let path_re = regex::Regex::new(r#"[a-zA-Z0-9_/.-]+\.(rs|ts|tsx|js|py|go|java|cs):\d+:"#).unwrap();
    result = path_re.replace_all(&result, "").to_string();

    // Strip line numbers (standalone numbers near colons)
    let line_re = regex::Regex::new(r#":\d+:"#).unwrap();
    result = line_re.replace_all(&result, ":").to_string();

    // Strip hex IDs and UUIDs
    let hex_re = regex::Regex::new(r#"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"#).unwrap();
    result = hex_re.replace_all(&result, "").to_string();

    let hex_short_re = regex::Regex::new(r#"[0-9a-f]{16,}"#).unwrap();
    result = hex_short_re.replace_all(&result, "").to_string();

    // Strip request IDs
    let req_re = regex::Regex::new(r#"request [a-zA-Z0-9_-]+"#).unwrap();
    result = req_re.replace_all(&result, "request").to_string();

    result.trim().to_string()
}

/// Compute a deterministic signature from a normalized message.
pub fn compute_signature(normalized: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    normalized.hash(&mut hasher);
    format!("sig_{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_paths_and_line_numbers() {
        let msg = "src/main.rs:42: error[E0308]: mismatched types, expected `i32` found `&str`";
        let normalized = normalize_message(msg);
        assert!(!normalized.contains("src/main.rs"));
        assert!(!normalized.contains("42"));
        assert!(normalized.contains("mismatched types"));
    }

    #[test]
    fn normalize_strips_temp_ids() {
        let msg = "error in request abc123def456: connection refused";
        let normalized = normalize_message(msg);
        assert!(!normalized.contains("abc123def456"));
    }

    #[test]
    fn record_failure_promotes_at_count_3() {
        let store = NegativeKnowledgeStore::new(":memory:").unwrap();
        let event = FailureEvent {
            signature: "test_sig".into(),
            raw: "test error".into(),
            kind: "compile".into(),
            fix_recipe: Some("check types".into()),
            count: 0,
        };

        let promoted1 = store.record_failure(&event).unwrap();
        assert!(!promoted1);

        let promoted2 = store.record_failure(&event).unwrap();
        assert!(!promoted2);

        let promoted3 = store.record_failure(&event).unwrap();
        assert!(promoted3);

        let rules = store.get_promoted_rules().unwrap();
        assert!(rules.iter().any(|r| r.pattern.contains("test error")), "Should find promoted rule matching 'test error'");
    }

    #[test]
    fn get_stats_returns_counts() {
        let store = NegativeKnowledgeStore::new(":memory:").unwrap();
        let event = FailureEvent {
            signature: "test_sig".into(),
            raw: "test error".into(),
            kind: "compile".into(),
            fix_recipe: None,
            count: 0,
        };

        store.record_failure(&event).unwrap();
        store.record_failure(&event).unwrap();
        store.record_failure(&event).unwrap();

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.total_patterns, 1);
        assert_eq!(stats.promoted, 1);
    }

    #[test]
    fn signature_is_deterministic() {
        let msg1 = "src/lib.rs:10: error: unused variable `x`";
        let msg2 = "src/lib.rs:20: error: unused variable `x`";
        let sig1 = compute_signature(&normalize_message(msg1));
        let sig2 = compute_signature(&normalize_message(msg2));
        assert_eq!(sig1, sig2, "Same normalized message should produce same signature");
    }
}
