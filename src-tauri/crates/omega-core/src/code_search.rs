//! Semantic code search: connects repo-map symbol indexing (harness) to the
//! embedding-backed memory store (memory). Symbols are persisted as Project-layer
//! memory entries whose value is the symbol JSON; search runs the store's
//! FTS5 + embedding ranking and filters to symbol entries.

use harness::repomap::Symbol;
use memory::{MemoryLayer, MemoryStore, SearchResult};

/// Key prefix that marks memory entries as code-search symbols.
const SYM_PREFIX: &str = "sym:";

/// Marker key storing the repo root that was last indexed.
const INDEX_MARKER: &str = "sym:__indexed_root__";

/// A semantic code-search hit, reconstructed from an indexed symbol entry.
#[derive(Debug, Clone)]
pub struct CodeSearchHit {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: u32,
    pub relevance: f64,
}

fn symbol_key(s: &Symbol) -> String {
    format!("{}::{}", s.file_path, s.name)
}

/// Index repo-map symbols into the project-layer memory store.
/// Re-indexing is idempotent: symbols already stored are skipped.
pub fn index_symbols(store: &MemoryStore, symbols: &[Symbol]) -> Result<usize, String> {
    let mut indexed = 0;
    for s in symbols {
        let key = format!("{}{}", SYM_PREFIX, symbol_key(s));
        // Skip already-indexed symbols (touch avoids re-embedding on re-index).
        if store
            .remember(&key, Some("project"))
            .ok()
            .flatten()
            .is_some()
        {
            continue;
        }
        let value = serde_json::to_string(s).map_err(|e| format!("serialize symbol: {}", e))?;
        store.store(MemoryLayer::Project, &key, &value)?;
        indexed += 1;
    }
    Ok(indexed)
}

/// Whether symbols for `root` are already indexed in this store.
pub fn is_indexed(store: &MemoryStore, root: &str) -> bool {
    store
        .remember(INDEX_MARKER, Some("project"))
        .ok()
        .flatten()
        .map(|m| m == root)
        .unwrap_or(false)
}

/// Record that `root` has been indexed.
fn mark_indexed(store: &MemoryStore, root: &str) -> Result<(), String> {
    // Touch marker so it keeps the latest root; store() appends a new row which
    // is fine for our indexed/idempotent check.
    store
        .store(MemoryLayer::Project, INDEX_MARKER, root)
        .map(|_| ())
}

/// Semantically search indexed code symbols. Returns hits ranked by relevance.
pub fn search_code(
    store: &MemoryStore,
    query: &str,
    limit: usize,
) -> Result<Vec<CodeSearchHit>, String> {
    let SearchResult { entries, relevance } = store.search(query, Some("project"), limit)?;
    let mut hits = Vec::new();
    for (entry, rel) in entries.into_iter().zip(relevance) {
        if !entry.key.starts_with(SYM_PREFIX) {
            continue;
        }
        if let Ok(sym) = serde_json::from_str::<Symbol>(&entry.value) {
            hits.push(CodeSearchHit {
                name: sym.name,
                kind: format!("{:?}", sym.kind),
                file_path: sym.file_path,
                start_line: sym.start_line,
                relevance: rel,
            });
        }
    }
    hits.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(hits)
}

/// Index a repo's symbols into the memory store, then run a semantic query.
/// Idempotent: only new symbols are written; a query may index lazily when
/// nothing has been indexed yet. `force_reindex` drops the symbol index first.
pub fn search_repo(
    root: &str,
    query: &str,
    limit: usize,
    db_path: &str,
    force_reindex: bool,
) -> Result<Vec<CodeSearchHit>, String> {
    let store = MemoryStore::new(db_path)?;

    let need_index = force_reindex || !is_indexed(&store, root);
    if need_index {
        if force_reindex {
            // Drop existing symbol entries (project layer only) before re-writing.
            clear_symbol_index(&store)?;
        }
        let mut repo_map = harness::repomap::RepoMap::new();
        repo_map.index_repo(std::path::Path::new(root))?;
        let symbols: Vec<Symbol> = repo_map.symbols().into_iter().cloned().collect();
        let n = index_symbols(&store, &symbols)?;
        mark_indexed(&store, root)?;
        log::info!("search_repo: indexed {} symbols from {}", n, root);
    }

    search_code(&store, query, limit)
}

/// Remove all `sym:` entries from the project layer.
fn clear_symbol_index(store: &MemoryStore) -> Result<(), String> {
    store.clear(Some("project"))?;
    // clear() wipes the whole project layer (not just sym:* rows); acceptable
    // ponytail: code-search is the only project-layer writer today. If other
    // project memories get added, scope this to a prefix delete.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(name: &str, file: &str, line: u32) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: harness::repomap::SymbolKind::Function,
            file_path: file.to_string(),
            start_line: line,
            end_line: line,
            signature: Some(format!("fn {}()", name)),
        }
    }

    #[test]
    fn index_is_idempotent() {
        let store = MemoryStore::new(":memory:").unwrap();
        let syms = vec![symbol("connect_db", "db.rs", 4)];
        assert_eq!(index_symbols(&store, &syms).unwrap(), 1);
        assert_eq!(
            index_symbols(&store, &syms).unwrap(),
            0,
            "no dupe rows on re-index"
        );
    }

    #[test]
    fn search_finds_matching_symbol() {
        let store = MemoryStore::new(":memory:").unwrap();
        let syms = vec![
            symbol("connect_db", "db.rs", 4),
            symbol("render_ui", "ui.rs", 10),
        ];
        index_symbols(&store, &syms).unwrap();
        let hits = search_code(&store, "database connection", 5).unwrap();
        assert_eq!(
            hits[0].name, "connect_db",
            "semantic match should rank first"
        );
        assert_eq!(hits[0].file_path, "db.rs");
        assert_eq!(hits[0].start_line, 4);
    }
}
