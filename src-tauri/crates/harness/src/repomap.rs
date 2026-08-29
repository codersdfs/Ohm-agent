//! Repo map — lightweight symbol index using tree-sitter.
//!
//! Walks a project directory, parses supported source files with tree-sitter
//! grammars, and extracts symbol definitions (functions, structs, classes,
//! imports, etc.) with file path + line range. Results are cached with an LRU
//! so repeated queries don't re-parse unchanged files.
//!
//! This is the Path B Phase 1 deliverable (P1-03). It builds on the existing
//! tree-sitter grammars already in `tree_sitter_metrics.rs` and adds repo-wide
//! indexing + symbol search.

use super::tree_sitter_metrics::get_ts_language;
use super::Language;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tree_sitter::{Node, Parser};

/// A symbol extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Relative file path from repo root
    pub file_path: String,
    /// 1-based line numbers
    pub start_line: u32,
    pub end_line: u32,
    /// Optional: full signature (e.g. `fn foo<T>(x: T) -> Result<T, Error>`)
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Class,
    Interface,
    TypeAlias,
    Const,
    Variable,
    Import,
    Call,
    Module,
    Unknown,
}

impl SymbolKind {
    fn from_ts_node_kind(kind: &str) -> Self {
        match kind {
            // Rust grammar (tree-sitter-rust)
            "function_item"
            | "function_definition"
            | "function_declaration"
            | "arrow_function"
            | "function" => Self::Function,
            "struct_item" | "struct_definition" | "struct" => Self::Struct,
            "enum" | "enum_definition" | "enum_item" => Self::Enum,
            "trait_item" | "trait_definition" | "trait" => Self::Trait,
            "impl_item" | "impl_block" | "impl" => Self::Impl,
            // TypeScript / JavaScript grammar
            "class_declaration" | "class" | "class_abstract" | "class_expression" => Self::Class,
            "interface_declaration" | "interface" => Self::Interface,
            "type_alias_declaration" | "type_alias" => Self::TypeAlias,
            // Rust
            "const" | "const_definition" | "const_token" | "const_item" => Self::Const,
            // TS/JS
            "variable_declarator" | "variable_declaration" => Self::Variable,
            // Both
            "import" | "import_declaration" | "import_statement" | "use_declaration"
            | "use_item" => Self::Import,
            "call_expression" | "call" => Self::Call,
            "mod_item" | "module" | "module_declaration" | "mod" => Self::Module,
            // Python
            "functiondef" | "async_funcdef" | "async_function_definition" => Self::Function,
            "classdef" | "class_definition" => Self::Class,
            _ => Self::Unknown,
        }
    }
}

/// Cached symbol entry for a parsed file.
#[derive(Debug, Clone)]
struct CachedFile {
    symbols: Vec<Symbol>,
    parsed_at: Instant,
    file_mtime: Duration,
}

/// In-memory LRU cache for parsed file symbols.
struct SymbolCache {
    entries: HashMap<String, CachedFile>,
    max_entries: usize,
}

impl SymbolCache {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    /// Returns cached symbols if file hasn't changed (mtime-based invalidation).
    fn get(&self, path: &str) -> Option<&Vec<Symbol>> {
        let entry = self.entries.get(path)?;
        let actual_mtime = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                Duration::from_secs(
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                )
            });
        if actual_mtime == Some(entry.file_mtime) {
            Some(&entry.symbols)
        } else {
            None
        }
    }

    fn set(&mut self, path: String, symbols: Vec<Symbol>, mtime: Duration) {
        if self.entries.len() >= self.max_entries {
            // Evict oldest by parsed_at
            if let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.parsed_at)
                .map(|(k, _)| k.clone())
            {
                self.entries.remove(&key);
            }
        }
        self.entries.insert(
            path,
            CachedFile {
                symbols,
                parsed_at: Instant::now(),
                file_mtime: mtime,
            },
        );
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Directories to skip during repo walks.
const IGNORE_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    ".next",
    ".nuxt",
];

/// File extension → language string mapping.
const SUPPORTED_EXTS: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("js", "javascript"),
    ("jsx", "javascript"),
    ("py", "python"),
];

/// Repo-wide symbol index.
pub struct RepoMap {
    /// Map of file_path → symbols (eager load for active repo)
    pub files: HashMap<String, Vec<Symbol>>,
    /// LRU cache for on-demand re-parsing of changed files
    cache: SymbolCache,
    /// Set of file extensions we can parse
    supported_extensions: Vec<&'static str>,
}

impl Default for RepoMap {
    fn default() -> Self {
        Self::new()
    }
}

impl RepoMap {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            cache: SymbolCache::new(100),
            supported_extensions: SUPPORTED_EXTS.iter().map(|(e, _)| *e).collect(),
        }
    }

    /// Walk `root` and index all supported source files.
    pub fn index_repo<P: AsRef<Path>>(&mut self, root: P) -> Result<usize, String> {
        let root = root.as_ref();
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| !Self::ignore_entry(e.path()))
        {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("skipping entry: {}", e);
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }

            let ext = entry.path().extension().and_then(|e| e.to_str());
            if ext.map_or(true, |e| !self.supported_extensions.contains(&e)) {
                continue;
            }

            let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
            let rel_str = rel.to_string_lossy().to_string();

            // Check cache (mtime-based invalidation)
            if let Some(cached) = self.cache.get(&rel_str) {
                self.files.insert(rel_str.clone(), cached.clone());
                continue;
            }

            match self.parse_file(entry.path()) {
                Ok(symbols) => {
                    let mtime = std::fs::metadata(entry.path())
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            Duration::from_secs(
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            )
                        })
                        .unwrap_or_default();

                    self.cache.set(rel_str.clone(), symbols.clone(), mtime);
                    self.files.insert(rel_str, symbols);
                }
                Err(e) => log::warn!("failed to parse {}: {}", rel_str, e),
            }
        }
        Ok(self.files.len())
    }

    /// Parse a single file and return its symbols.
    fn parse_file(&self, path: &Path) -> Result<Vec<Symbol>, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = match self.lang_for_ext(ext) {
            Some(l) => l,
            None => return Ok(vec![]),
        };

        let ts_lang = get_ts_language(&lang)
            .ok_or_else(|| format!("No tree-sitter grammar for {:?} (extension {})", lang, ext))?;

        let mut parser = Parser::new();
        if parser.set_language(ts_lang).is_err() {
            return Err("Failed to set tree-sitter language".into());
        }

        let tree = parser
            .parse(content.as_bytes(), None)
            .ok_or("Failed to parse with tree-sitter")?;

        let root = tree.root_node();
        let mut symbols = Vec::new();
        let rel_path = path.to_string_lossy().to_string();
        self.extract_symbols(&root, &content, &rel_path, &mut symbols);
        Ok(symbols)
    }

    fn lang_for_ext(&self, ext: &str) -> Option<Language> {
        let lang_str = SUPPORTED_EXTS
            .iter()
            .find(|(e, _)| *e == ext)
            .map(|(_, l)| *l)?;
        Some(Language::from_str(lang_str))
    }

    /// Recursively extract symbols from tree-sitter AST nodes.
    fn extract_symbols(&self, node: &Node, source: &str, file_path: &str, out: &mut Vec<Symbol>) {
        let kind = node.kind();
        let symbol_kind = SymbolKind::from_ts_node_kind(kind);

        // Only collect named nodes that represent definitions
        if symbol_kind != SymbolKind::Unknown && node.child_count() > 0 {
            if let Some(name) = Self::name_of(node, source) {
                if !name.is_empty() {
                    let start_line = node.start_position().row as u32 + 1;
                    let end_line = node.end_position().row as u32 + 1;
                    let signature = Self::signature_of(node, source);

                    out.push(Symbol {
                        name,
                        kind: symbol_kind,
                        file_path: file_path.to_string(),
                        start_line,
                        end_line,
                        signature,
                    });
                }
            }
        }

        // Recurse into children
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(i) {
                self.extract_symbols(&child, source, file_path, out);
            }
        }
    }

    fn ignore_entry(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|name| IGNORE_DIRS.contains(&name))
            .unwrap_or(false)
    }

    fn name_of(node: &Node, source: &str) -> Option<String> {
        // Try common field names for the identifier
        for field in &["name", "identifier", "left"] {
            if let Some(name_node) = node.child_by_field_name(field) {
                if let Ok(text) = name_node.utf8_text(source.as_bytes()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        None
    }

    fn signature_of(node: &Node, source: &str) -> Option<String> {
        node.utf8_text(source.as_bytes()).ok().and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }
            if trimmed.len() > 200 {
                // Find the nearest char boundary at or before byte 200
                let mut end = 200;
                while !trimmed.is_char_boundary(end) {
                    end -= 1;
                }
                Some(format!("{}...", &trimmed[..end]))
            } else {
                Some(trimmed.to_string())
            }
        })
    }

    /// Search symbols by name substring (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&Symbol> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<&Symbol> = self
            .files
            .values()
            .flatten()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .collect();
        // Sort by: exact match first, then shorter names (fewer false positives)
        results.sort_by(|a, b| {
            let a_exact = a.name.eq_ignore_ascii_case(&query_lower);
            let b_exact = b.name.eq_ignore_ascii_case(&query_lower);
            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.len().cmp(&b.name.len()),
            }
        });
        results
    }

    /// Get all symbols in a file.
    pub fn symbols_in_file(&self, file_path: &str) -> Option<&Vec<Symbol>> {
        self.files.get(file_path)
    }

    /// Get symbols of a specific kind.
    pub fn symbols_by_kind(&self, kind: SymbolKind) -> Vec<&Symbol> {
        self.files
            .values()
            .flatten()
            .filter(|s| s.kind == kind)
            .collect()
    }

    /// All indexed symbols across every file.
    pub fn symbols(&self) -> Vec<&Symbol> {
        self.files.values().flatten().collect()
    }

    /// Drop the cache and re-index.
    pub fn clear(&mut self) {
        self.files.clear();
        self.cache.clear();
    }

    /// Build a reference graph from the indexed symbols and return ranked
    /// `(Symbol, score)` pairs sorted descending by graph-centrality score.
    ///
    /// Nodes are symbols; edges go from a symbol that references another
    /// to the referenced symbol. Edge weights derive from symbol kind.
    /// Ranking uses a simplified iterative PageRank (damping 0.85, 50 max
    /// iterations, 1e-6 convergence threshold) over a custom adjacency list.
    pub fn build_graph(&self) -> Vec<(&Symbol, f64)> {
        let symbols: Vec<&Symbol> = self.symbols();
        if symbols.is_empty() {
            return vec![];
        }

        let n = symbols.len();

        let kind_weight = |s: &Symbol| -> f64 {
            match s.kind {
                SymbolKind::Trait
                | SymbolKind::Struct
                | SymbolKind::Enum
                | SymbolKind::TypeAlias => 3.0,
                SymbolKind::Function => 2.0,
                SymbolKind::Impl | SymbolKind::Class | SymbolKind::Interface => 2.5,
                SymbolKind::Module => 1.5,
                SymbolKind::Import => 0.5,
                SymbolKind::Variable | SymbolKind::Const => 0.8,
                SymbolKind::Call | SymbolKind::Unknown => 1.0,
            }
        };

        let mut adj: Vec<Vec<(usize, f64)>> = vec![vec![]; n];
        let mut base_weights: Vec<f64> = vec![0.0; n];

        for (i, s) in symbols.iter().enumerate() {
            base_weights[i] = kind_weight(s);
        }

        for (i, s) in symbols.iter().enumerate() {
            let sig = s.signature.as_deref().unwrap_or("");
            let text = format!("{} {} {}", s.name, sig, s.file_path);
            for (j, other) in symbols.iter().enumerate() {
                if i == j {
                    continue;
                }
                if text.contains(&other.name) && other.name.len() > 2 {
                    let weight = kind_weight(other) * 0.5;
                    adj[i].push((j, weight));
                    base_weights[i] += weight * 0.3;
                }
            }
        }

        let total_weight: f64 = base_weights.iter().sum::<f64>().max(1.0);
        for w in &mut base_weights {
            *w /= total_weight;
        }

        let d: f64 = 0.85;
        let epsilon: f64 = 1e-6;
        let max_iter: usize = 50;
        let mut ranks = vec![1.0 / n as f64; n];

        for _ in 0..max_iter {
            let mut new_ranks = vec![0.0; n];
            for j in 0..n {
                let mut sum = 0.0;
                for (src, outgoing) in adj.iter().enumerate() {
                    for (tgt, w) in outgoing {
                        if *tgt == j {
                            let out_weight: f64 =
                                adj[src].iter().map(|(_, w)| *w).sum::<f64>().max(1.0);
                            sum += ranks[src] * (*w / out_weight);
                        }
                    }
                }
                new_ranks[j] = (1.0 - d) / n as f64 + d * sum;
            }
            let diff: f64 = ranks
                .iter()
                .zip(new_ranks.iter())
                .map(|(a, b)| (a - b).abs())
                .sum();
            ranks = new_ranks;
            if diff < epsilon {
                break;
            }
        }

        let mut indexed: Vec<(usize, f64)> = (0..n).map(|i| (i, ranks[i])).collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut result: Vec<(&Symbol, f64)> = vec![];
        for (i, rank) in indexed {
            result.push((symbols[i], rank));
        }
        result
    }

    /// Render the repo-map within a token budget.
    ///
    /// Produces a tree-format string grouping symbols by file, ordered by
    /// graph rank. Symbols with lower rank are dropped first when the budget
    /// is tight. Uses chars/4 as token approximation for the repo-map section.
    pub fn render(&self, token_budget: u32) -> String {
        if token_budget == 0 {
            return String::new();
        }

        let ranked = self.build_graph();
        if ranked.is_empty() {
            return String::new();
        }

        let max_chars = (token_budget as usize) * 4;
        let mut out = String::new();
        out.push_str("# Repo map (ranked by graph centrality):\n");

        let mut file_groups: std::collections::HashMap<&str, Vec<(&Symbol, f64)>> =
            std::collections::HashMap::new();
        let mut file_order: Vec<&str> = vec![];

        for (symbol, rank) in &ranked {
            let entry = file_groups.entry(symbol.file_path.as_str()).or_default();
            if entry.is_empty() {
                file_order.push(symbol.file_path.as_str());
            }
            entry.push((symbol, *rank));
        }

        for file_path in &file_order {
            let symbols = file_groups.get(file_path).unwrap();
            let mut block = String::new();
            block.push_str(&format!("{}/:\n", file_path));

            for (symbol, rank) in symbols {
                let sig = symbol.signature.as_deref().unwrap_or(&symbol.name);
                let line = format!("  ├── {} ({:?}) [rank: {:.3}]\n", sig, symbol.kind, rank);
                if out.len() + block.len() + line.len() > max_chars {
                    if out.len() + block.len() <= max_chars {
                        out.push_str(&block);
                        out.push_str("  …[truncated]\n");
                    }
                    return out;
                }
                block.push_str(&line);
            }
            out.push_str(&block);
        }

        out
    }
}

// Suppress unused import warning — PathBuf is used by walkdir's API indirectly
#[allow(unused_imports)]
use PathBuf as _PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_kind_from_ts() {
        assert_eq!(
            SymbolKind::from_ts_node_kind("function_definition"),
            SymbolKind::Function
        );
        assert_eq!(
            SymbolKind::from_ts_node_kind("struct_definition"),
            SymbolKind::Struct
        );
        assert_eq!(
            SymbolKind::from_ts_node_kind("import_declaration"),
            SymbolKind::Import
        );
        assert_eq!(
            SymbolKind::from_ts_node_kind("unknown_kind"),
            SymbolKind::Unknown
        );
    }

    #[test]
    fn index_rust_file_finds_functions() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("test.rs");
        std::fs::write(&file, "fn foo() {}\nstruct Bar;\nfn baz() {}").unwrap();

        let symbols = map.parse_file(&file).unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"), "missing foo: {:?}", names);
        assert!(names.contains(&"Bar"), "missing Bar: {:?}", names);
        assert!(names.contains(&"baz"), "missing baz: {:?}", names);
    }

    #[test]
    fn search_finds_symbol_by_name() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("test.rs");
        std::fs::write(&file, "fn my_function() {}\nstruct MyStruct;").unwrap();

        let symbols = map.parse_file(&file).unwrap();
        map.files.insert("test.rs".to_string(), symbols);

        let results = map.search("my_function");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "my_function");
        assert_eq!(results[0].kind, SymbolKind::Function);
    }

    #[test]
    fn search_case_insensitive() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("test.rs");
        std::fs::write(&file, "fn MyFunction() {}").unwrap();

        let symbols = map.parse_file(&file).unwrap();
        map.files.insert("test.rs".to_string(), symbols);

        let results = map.search("myfunction");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "MyFunction");
    }

    #[test]
    fn parse_python_file() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("test.py");
        std::fs::write(
            &file,
            "def my_func():\n    pass\nclass MyClass:\n    pass\n",
        )
        .unwrap();

        let symbols = map.parse_file(&file).unwrap();
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"my_func"), "missing my_func: {:?}", names);
        assert!(names.contains(&"MyClass"), "missing MyClass: {:?}", names);
    }

    #[test]
    fn index_repo_walks_directory() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();

        std::fs::write(temp.path().join("a.rs"), "fn alpha() {}").unwrap();
        std::fs::write(temp.path().join("b.py"), "def beta(): pass").unwrap();
        // Non-source file — should be skipped
        std::fs::write(temp.path().join("README.md"), "# test").unwrap();

        let count = map.index_repo(temp.path()).unwrap();
        assert!(
            count >= 2,
            "expected at least 2 indexed files, got {}",
            count
        );

        let all_names: Vec<String> = map
            .files
            .values()
            .flatten()
            .map(|s| s.name.clone())
            .collect();
        assert!(
            all_names.contains(&"alpha".to_string()),
            "missing alpha: {:?}",
            all_names
        );
        assert!(
            all_names.contains(&"beta".to_string()),
            "missing beta: {:?}",
            all_names
        );
    }

    #[test]
    fn index_repo_skips_ignored_dirs() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();

        std::fs::write(temp.path().join("keep.rs"), "fn keep() {}").unwrap();
        std::fs::create_dir_all(temp.path().join("node_modules")).unwrap();
        std::fs::write(
            temp.path().join("node_modules").join("skip.rs"),
            "fn skip() {}",
        )
        .unwrap();

        let count = map.index_repo(temp.path()).unwrap();
        assert_eq!(
            count, 1,
            "should only index 1 file (skip node_modules), got {}",
            count
        );

        let names: Vec<&str> = map
            .files
            .values()
            .flatten()
            .map(|s| s.name.as_str())
            .collect();
        assert!(names.contains(&"keep"), "missing keep function");
        assert!(
            !names.contains(&"skip"),
            "skip function should not be indexed"
        );
    }
}
#[test]
fn build_graph_empty_repo() {
    let map = RepoMap::new();
    let ranked = map.build_graph();
    assert!(ranked.is_empty());
}

#[test]
fn build_graph_returns_sorted_ranks() {
    let mut map = RepoMap::new();
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("a.rs"),
        "fn main() { foo(); bar(); }\nfn foo() {}\nfn bar() {}",
    )
    .unwrap();
    let symbols = map.parse_file(temp.path().join("a.rs").as_path()).unwrap();
    map.files.insert("a.rs".to_string(), symbols);

    let ranked = map.build_graph();
    assert!(!ranked.is_empty());
    // Verify descending order.
    for w in ranked.windows(2) {
        assert!(
            w[0].1 >= w[1].1,
            "ranks not descending: {} < {}",
            w[0].1,
            w[1].1
        );
    }
}

#[test]
fn render_respects_budget() {
    let mut map = RepoMap::new();
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("a.rs"),
        "fn alpha() {}\nfn beta() {}\nstruct Gamma;",
    )
    .unwrap();
    let symbols = map.parse_file(temp.path().join("a.rs").as_path()).unwrap();
    map.files.insert("a.rs".to_string(), symbols);

    // With zero budget → empty.
    assert_eq!(map.render(0), "");

    // With small budget → should fit.
    let rendered = map.render(512);
    assert!(rendered.contains("a.rs"));
    assert!(rendered.contains("alpha"));
    assert!(rendered.contains("beta"));
    assert!(rendered.contains("Gamma"));
}

#[test]
fn render_groups_by_file() {
    let mut map = RepoMap::new();
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("a.rs"), "fn foo() {}\nstruct Bar;").unwrap();
    std::fs::write(temp.path().join("b.rs"), "fn baz() {}").unwrap();
    let syms_a = map.parse_file(temp.path().join("a.rs").as_path()).unwrap();
    let syms_b = map.parse_file(temp.path().join("b.rs").as_path()).unwrap();
    map.files.insert("a.rs".to_string(), syms_a);
    map.files.insert("b.rs".to_string(), syms_b);

    let rendered = map.render(2048);
    assert!(rendered.contains("a.rs/"));
    assert!(rendered.contains("b.rs/"));
}
