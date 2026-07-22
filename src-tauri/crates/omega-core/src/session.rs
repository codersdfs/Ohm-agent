//! Conversation session persistence (JSONL).
//!
//! Sessions live under the project config dir:
//!   `<config_dir>/sessions/<session_id>.jsonl`
//! with a `last-session` marker for default resume.
//!
//! Writes are atomic: full history is serialized to a temp file, then renamed
//! over the active path. This avoids partial-line appends and duplicate records
//! after a mid-write crash.

use chrono::Utc;
use providers::ChatMessage;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Rotate when the session file exceeds this size.
pub const MAX_SESSION_BYTES: u64 = 2 * 1024 * 1024; // 2 MiB

/// Tool message content is truncated to this many chars on load.
pub const TOOL_CONTENT_TRUNCATE: usize = 2000;

const TRUNCATE_SUFFIX: &str = "\n…[truncated on load]";
const ROTATE_MARKER_ROLE: &str = "system";
const ROTATE_MARKER_PREFIX: &str = "[session-rotated]";

/// One JSONL record — ChatMessage fields plus timestamp.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<providers::ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub ts: String,
}

impl SessionRecord {
    pub fn from_message(msg: &ChatMessage) -> Self {
        Self {
            role: msg.role.clone(),
            content: msg.content.clone(),
            tool_calls: msg.tool_calls.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            name: msg.name.clone(),
            ts: Utc::now().to_rfc3339(),
        }
    }

    pub fn into_message(self) -> ChatMessage {
        ChatMessage {
            role: self.role,
            content: self.content,
            tool_calls: self.tool_calls,
            tool_call_id: self.tool_call_id,
            name: self.name,
        }
    }
}

/// Result of loading a session file.
#[derive(Debug, Default)]
pub struct SessionLoad {
    pub messages: Vec<ChatMessage>,
    pub warnings: Vec<String>,
    /// True when history was restored from an existing file.
    pub resumed: bool,
}

/// Open session handle with append tracking to avoid duplicate writes.
#[derive(Debug)]
pub struct SessionStore {
    pub id: String,
    pub path: PathBuf,
    /// How many messages from the start of the in-memory history are already on disk.
    persisted_count: usize,
}

impl SessionStore {
    /// Project config directory (`directories` crate), matching CLI config_dir.
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("com", "omega", "omega-agent")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn sessions_dir() -> PathBuf {
        Self::config_dir().join("sessions")
    }

    pub fn last_session_path() -> PathBuf {
        Self::sessions_dir().join("last-session")
    }

    pub fn path_for(id: &str) -> PathBuf {
        Self::sessions_dir().join(format!("{id}.jsonl"))
    }

    pub fn ensure_sessions_dir() -> Result<(), String> {
        fs::create_dir_all(Self::sessions_dir()).map_err(|e| format!("create sessions dir: {e}"))
    }

    pub fn read_last_session_id() -> Option<String> {
        let path = Self::last_session_path();
        fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn write_last_session_id(id: &str) -> Result<(), String> {
        Self::ensure_sessions_dir()?;
        fs::write(Self::last_session_path(), id).map_err(|e| format!("write last-session: {e}"))
    }

    /// Create a brand-new session id and empty file; mark as last.
    pub fn create_new() -> Result<Self, String> {
        Self::ensure_sessions_dir()?;
        let id = uuid::Uuid::new_v4().to_string();
        let path = Self::path_for(&id);
        File::create(&path).map_err(|e| format!("create session file: {e}"))?;
        Self::write_last_session_id(&id)?;
        Ok(Self {
            id,
            path,
            persisted_count: 0,
        })
    }

    /// Open an existing session (or create empty file if missing).
    pub fn open(id: &str) -> Result<Self, String> {
        if id.trim().is_empty() {
            return Err("session id must not be empty".into());
        }
        // Reject path separators / traversal
        if id.contains('/') || id.contains('\\') || id.contains("..") {
            return Err("invalid session id".into());
        }
        Self::ensure_sessions_dir()?;
        let path = Self::path_for(id);
        if !path.exists() {
            File::create(&path).map_err(|e| format!("create session file: {e}"))?;
        }
        Self::write_last_session_id(id)?;
        let mut store = Self {
            id: id.to_string(),
            path,
            persisted_count: 0,
        };
        // Count existing records so no-op persists stay cheap.
        let loaded = store.load_messages()?;
        store.persisted_count = loaded.messages.len();
        Ok(store)
    }

    /// Resolve CLI flags:
    /// - `--new-session` → always new
    /// - `--session <id>` → open that id
    /// - default → last session if marker exists, else new
    pub fn resolve(
        session_id: Option<String>,
        new_session: bool,
    ) -> Result<(Self, SessionLoad), String> {
        if new_session {
            let store = Self::create_new()?;
            return Ok((
                store,
                SessionLoad {
                    messages: Vec::new(),
                    warnings: Vec::new(),
                    resumed: false,
                },
            ));
        }

        if let Some(id) = session_id {
            let mut store = Self::open(&id)?;
            let mut load = store.load_messages()?;
            load.resumed = !load.messages.is_empty();
            store.persisted_count = load.messages.len();
            return Ok((store, load));
        }

        if let Some(last) = Self::read_last_session_id() {
            let path = Self::path_for(&last);
            if path.exists() {
                let mut store = Self::open(&last)?;
                let mut load = store.load_messages()?;
                load.resumed = !load.messages.is_empty();
                store.persisted_count = load.messages.len();
                return Ok((store, load));
            }
        }

        let store = Self::create_new()?;
        Ok((
            store,
            SessionLoad {
                messages: Vec::new(),
                warnings: Vec::new(),
                resumed: false,
            },
        ))
    }

    /// Load messages from this session's JSONL file.
    /// Corrupt lines are skipped (warning recorded). Tool contents truncated.
    pub fn load_messages(&self) -> Result<SessionLoad, String> {
        load_session_file(&self.path)
    }

    /// Durably clear the active session file and reset the in-memory cursor.
    /// Keeps the same session id and last-session marker.
    pub fn clear(&mut self) -> Result<(), String> {
        self.atomic_write_body("", 0)?;
        self.persisted_count = 0;
        // Ensure marker still points at this (now empty) session.
        Self::write_last_session_id(&self.id)?;
        Ok(())
    }

    /// Persist the full conversation snapshot atomically.
    ///
    /// No-ops when `messages.len() == persisted_count` (already on disk).
    /// Always rewrites the whole file via temp+rename so a crash cannot leave
    /// partial lines or duplicate appends. Rotates to `.jsonl.1` when the
    /// payload would exceed [`MAX_SESSION_BYTES`].
    pub fn persist_messages(&mut self, messages: &[ChatMessage]) -> Result<(), String> {
        if messages.len() == self.persisted_count && self.path.exists() {
            // Already durable; still allow clear→empty and grow paths through.
            if messages.is_empty() {
                return Ok(());
            }
            // Cheap path: nothing new.
            return Ok(());
        }

        // History shortened (e.g. /clear then new turns, or external reset).
        // Always rewrite from the provided snapshot.
        self.write_snapshot(messages)
    }

    /// Backward-compatible alias used by earlier integration points.
    pub fn append_messages(&mut self, messages: &[ChatMessage]) -> Result<(), String> {
        self.persist_messages(messages)
    }

    fn write_snapshot(&mut self, messages: &[ChatMessage]) -> Result<(), String> {
        let body = serialize_messages(messages, false)?;
        let body_bytes = body.len() as u64;

        if body_bytes > MAX_SESSION_BYTES || self.file_len() > MAX_SESSION_BYTES {
            // Archive current active file (if any), then write a compacted snapshot.
            self.rotate_current_to_backup()?;
            let compact = serialize_messages(messages, true)?;
            let marker = rotate_marker_line(&self.path)?;
            let full = format!("{marker}{compact}");
            self.atomic_write_body(&full, messages.len())?;
            return Ok(());
        }

        self.atomic_write_body(&body, messages.len())
    }

    fn file_len(&self) -> u64 {
        fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0)
    }

    fn rotate_current_to_backup(&self) -> Result<(), String> {
        if !self.path.exists() {
            return Ok(());
        }
        let backup = PathBuf::from(format!("{}.1", self.path.display()));
        let _ = fs::remove_file(&backup);
        // Best-effort: if rename fails because nothing is there, ignore.
        match fs::rename(&self.path, &backup) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("rotate session: {e}")),
        }
    }

    /// Write `body` to `<path>.tmp` then rename over `path`. Only updates
    /// `persisted_count` after a successful rename.
    fn atomic_write_body(&mut self, body: &str, count: usize) -> Result<(), String> {
        let tmp = self.tmp_path();
        {
            let mut file = File::create(&tmp).map_err(|e| format!("create session temp: {e}"))?;
            file.write_all(body.as_bytes())
                .map_err(|e| format!("write session temp: {e}"))?;
            file.flush()
                .map_err(|e| format!("flush session temp: {e}"))?;
        }
        // On Windows, rename over existing destination can fail; remove first.
        if self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
        fs::rename(&tmp, &self.path).map_err(|e| {
            // Leave tmp in place for debugging; do not bump persisted_count.
            format!("atomic replace session: {e}")
        })?;
        self.persisted_count = count;
        Ok(())
    }

    fn tmp_path(&self) -> PathBuf {
        let mut tmp = self.path.clone();
        let name = self
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("session.jsonl");
        tmp.set_file_name(format!("{name}.tmp"));
        tmp
    }
}

fn rotate_marker_line(path: &Path) -> Result<String, String> {
    let marker = SessionRecord {
        role: ROTATE_MARKER_ROLE.into(),
        content: format!(
            "{ROTATE_MARKER_PREFIX} older history archived to {}.1",
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("session.jsonl")
        ),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        ts: Utc::now().to_rfc3339(),
    };
    Ok(format!(
        "{}\n",
        serde_json::to_string(&marker).map_err(|e| e.to_string())?
    ))
}

fn serialize_messages(messages: &[ChatMessage], compact_tools: bool) -> Result<String, String> {
    let mut body = String::new();
    for msg in messages {
        let mut owned;
        let msg_ref = if compact_tools && msg.role == "tool" {
            owned = msg.clone();
            owned.content = truncate_tool_content(&owned.content);
            &owned
        } else {
            msg
        };
        let rec = SessionRecord::from_message(msg_ref);
        body.push_str(&serde_json::to_string(&rec).map_err(|e| e.to_string())?);
        body.push('\n');
    }
    Ok(body)
}

/// Truncate tool content for load-time compaction.
pub fn truncate_tool_content(content: &str) -> String {
    let count = content.chars().count();
    if count <= TOOL_CONTENT_TRUNCATE {
        return content.to_string();
    }
    let truncated: String = content.chars().take(TOOL_CONTENT_TRUNCATE).collect();
    format!("{truncated}{TRUNCATE_SUFFIX}")
}

/// Load a session JSONL file from an arbitrary path (used by tests + SessionStore).
pub fn load_session_file(path: &Path) -> Result<SessionLoad, String> {
    if !path.exists() {
        return Ok(SessionLoad::default());
    }
    let file = File::open(path).map_err(|e| format!("open session: {e}"))?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut warnings = Vec::new();

    for (line_no, line_res) in reader.lines().enumerate() {
        let line = match line_res {
            Ok(l) => l,
            Err(e) => {
                warnings.push(format!("line {}: read error: {e}", line_no + 1));
                continue;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let rec: SessionRecord = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                warnings.push(format!("line {}: corrupt JSON skipped: {e}", line_no + 1));
                log::warn!(
                    "session {}: line {}: corrupt JSON skipped: {e}",
                    path.display(),
                    line_no + 1
                );
                continue;
            }
        };

        // Skip internal rotation markers (not LLM history).
        if rec.role == ROTATE_MARKER_ROLE && rec.content.starts_with(ROTATE_MARKER_PREFIX) {
            continue;
        }

        let mut msg = rec.into_message();
        if msg.role == "tool" {
            msg.content = truncate_tool_content(&msg.content);
        }
        messages.push(msg);
    }

    Ok(SessionLoad {
        messages,
        warnings,
        resumed: false,
    })
}

/// Convert ChatMessage history into reasonable TUI transcript entries.
/// System prompts are omitted from the UI; tool messages become notices.
pub fn messages_to_transcript_entries(
    messages: &[ChatMessage],
) -> Vec<crate::tui::transcript::TranscriptEntry> {
    use crate::tui::transcript::{ToolCallState, ToolCallStatus, TranscriptEntry};

    let mut entries = Vec::new();
    for msg in messages {
        match msg.role.as_str() {
            "system" => {
                // Hide default system prompts; show only short notices if any.
            }
            "user" => {
                entries.push(TranscriptEntry::User {
                    content: msg.content.clone(),
                });
            }
            "assistant" => {
                if let Some(ref tcs) = msg.tool_calls {
                    for tc in tcs {
                        let mut state = ToolCallState::new(
                            tc.function.name.clone(),
                            tc.function.arguments.clone(),
                        );
                        state.status = ToolCallStatus::Completed;
                        entries.push(TranscriptEntry::ToolCallBox { state });
                    }
                }
                if !msg.content.is_empty() {
                    entries.push(TranscriptEntry::Assistant {
                        content: msg.content.clone(),
                        rendered: None,
                        is_streaming: false,
                        thinking: String::new(),
                    });
                }
            }
            "tool" => {
                let name = msg.name.as_deref().unwrap_or("tool");
                let preview: String = msg.content.chars().take(200).collect();
                let text = if msg.content.chars().count() > 200 {
                    format!("{name} → {preview}…")
                } else {
                    format!("{name} → {preview}")
                };
                // Attach to last matching tool box when possible.
                let mut attached = false;
                for entry in entries.iter_mut().rev() {
                    if let TranscriptEntry::ToolCallBox { state } = entry {
                        if state.tool_name == name && state.result.is_none() {
                            state.result = Some(preview.clone());
                            state.result_preview = Some(preview.clone());
                            state.status = ToolCallStatus::Completed;
                            attached = true;
                            break;
                        }
                    }
                }
                if !attached {
                    entries.push(TranscriptEntry::Notice {
                        text,
                        is_error: false,
                    });
                }
            }
            other => {
                entries.push(TranscriptEntry::Notice {
                    text: format!(
                        "[{other}] {}",
                        msg.content.chars().take(120).collect::<String>()
                    ),
                    is_error: false,
                });
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "omega-session-test-{}-{}-{}",
            std::process::id(),
            n,
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_messages() -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: "system".into(),
                content: "You are helpful.".into(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ChatMessage {
                role: "user".into(),
                content: "Hello".into(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ChatMessage {
                role: "assistant".into(),
                content: "Hi there!".into(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ]
    }

    fn store_at(dir: &Path, id: &str) -> SessionStore {
        let path = dir.join(format!("{id}.jsonl"));
        File::create(&path).unwrap();
        SessionStore {
            id: id.to_string(),
            path,
            persisted_count: 0,
        }
    }

    #[test]
    fn roundtrip_three_messages() {
        let dir = unique_dir();
        let mut store = store_at(&dir, "rt");
        let msgs = sample_messages();
        store.persist_messages(&msgs).unwrap();

        // Second persist of the same list must not duplicate.
        store.persist_messages(&msgs).unwrap();

        let loaded = store.load_messages().unwrap();
        assert_eq!(loaded.messages.len(), 3);
        assert_eq!(loaded.messages[0].role, "system");
        assert_eq!(loaded.messages[1].content, "Hello");
        assert_eq!(loaded.messages[2].content, "Hi there!");
        assert!(loaded.warnings.is_empty());

        // File should have exactly 3 lines of JSON.
        let raw = fs::read_to_string(&store.path).unwrap();
        let lines: Vec<_> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn durable_clear_empties_file_and_resets_count() {
        let dir = unique_dir();
        let mut store = store_at(&dir, "clr");
        store.persist_messages(&sample_messages()).unwrap();
        assert!(!fs::read_to_string(&store.path).unwrap().is_empty());
        assert_eq!(store.persisted_count, 3);

        store.clear().unwrap();
        assert_eq!(store.persisted_count, 0);
        let raw = fs::read_to_string(&store.path).unwrap();
        assert!(
            raw.trim().is_empty(),
            "expected empty session file, got {raw:?}"
        );
        let loaded = store.load_messages().unwrap();
        assert!(loaded.messages.is_empty());

        // New messages after clear rewrite cleanly (no ghosts).
        let again = vec![sample_messages()[1].clone()];
        store.persist_messages(&again).unwrap();
        let loaded = store.load_messages().unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "Hello");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_rewrite_no_duplicates_on_repeated_persist() {
        let dir = unique_dir();
        let mut store = store_at(&dir, "atom");
        let mut msgs = sample_messages();
        store.persist_messages(&msgs).unwrap();

        // Grow history one message at a time; each persist is a full rewrite.
        msgs.push(ChatMessage {
            role: "user".into(),
            content: "Again".into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
        store.persist_messages(&msgs).unwrap();
        store.persist_messages(&msgs).unwrap(); // identical snapshot

        let loaded = store.load_messages().unwrap();
        assert_eq!(loaded.messages.len(), 4);
        let raw = fs::read_to_string(&store.path).unwrap();
        let lines: Vec<_> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(
            loaded
                .messages
                .iter()
                .filter(|m| m.content == "Again")
                .count(),
            1
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_line_skipped_rest_loaded() {
        let dir = unique_dir();
        let path = dir.join("corrupt.jsonl");
        let good1 = SessionRecord::from_message(&sample_messages()[1]);
        let good2 = SessionRecord::from_message(&sample_messages()[2]);
        let body = format!(
            "{}\nTHIS IS NOT JSON\n{}\n",
            serde_json::to_string(&good1).unwrap(),
            serde_json::to_string(&good2).unwrap()
        );
        fs::write(&path, body).unwrap();

        let loaded = load_session_file(&path).unwrap();
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content, "Hello");
        assert_eq!(loaded.messages[1].content, "Hi there!");
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].contains("corrupt"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tool_content_truncated_on_load() {
        let dir = unique_dir();
        let path = dir.join("tool.jsonl");
        let big: String = "x".repeat(5000);
        let rec = SessionRecord {
            role: "tool".into(),
            content: big.clone(),
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
            name: Some("read".into()),
            ts: Utc::now().to_rfc3339(),
        };
        fs::write(&path, format!("{}\n", serde_json::to_string(&rec).unwrap())).unwrap();

        let loaded = load_session_file(&path).unwrap();
        assert_eq!(loaded.messages.len(), 1);
        let content = &loaded.messages[0].content;
        assert!(content.ends_with(TRUNCATE_SUFFIX));
        let without_suffix = content.trim_end_matches(TRUNCATE_SUFFIX);
        assert_eq!(without_suffix.chars().count(), TOOL_CONTENT_TRUNCATE);
        // Original full content is not present
        assert!(content.chars().count() < big.chars().count());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_creates_backup_when_over_limit() {
        let dir = unique_dir();
        let mut store = store_at(&dir, "rot");

        // Build a snapshot larger than MAX_SESSION_BYTES so write_snapshot rotates.
        let pad: String = "p".repeat(64 * 1024);
        let mut huge = Vec::new();
        let mut size = 0u64;
        while size <= MAX_SESSION_BYTES {
            huge.push(ChatMessage {
                role: "user".into(),
                content: pad.clone(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
            size += pad.len() as u64 + 32;
        }

        // First persist should rotate (body > cap) and write compacted form.
        // Compaction only shortens tool roles; user pads still huge, so we
        // force the rotate path by pre-filling the active file over the limit.
        {
            let pad_body = serialize_messages(&huge[..3], false).unwrap();
            // Write a small file first, then a huge snapshot.
            store.atomic_write_body(&pad_body, 3).unwrap();
        }
        // Manually inflate file past limit so next persist rotates.
        {
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .append(true)
                .open(&store.path)
                .unwrap();
            let junk = "x".repeat((MAX_SESSION_BYTES as usize) + 64);
            // Invalid JSON line — load will skip; size still counts for rotation.
            writeln!(f, "{junk}").unwrap();
        }
        assert!(store.file_len() > MAX_SESSION_BYTES);

        let full = sample_messages();
        store.persisted_count = 0;
        store.persist_messages(&full).unwrap();

        let backup = PathBuf::from(format!("{}.1", store.path.display()));
        assert!(backup.exists(), "expected rotated backup .jsonl.1");
        assert!(store.path.exists());

        let loaded = store.load_messages().unwrap();
        assert_eq!(loaded.messages.len(), 3);
        assert_eq!(loaded.messages[1].content, "Hello");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_api_keys_in_records() {
        // SessionRecord only holds ChatMessage fields + ts — never provider config.
        let rec = SessionRecord::from_message(&sample_messages()[0]);
        let json = serde_json::to_string(&rec).unwrap();
        assert!(!json.contains("api_key"));
        assert!(!json.contains("Authorization"));
    }
}
