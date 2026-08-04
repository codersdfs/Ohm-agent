//! Repo map — symbol index with graph ranking and token-budgeted rendering.
//!
//! Walks a project directory, parses supported source files with tree-sitter,
//! and extracts symbol definitions (functions, structs, classes, imports,
//! calls, etc.) with file path + line range. Results are cached with an
//! mtime-invalidated LRU so repeated queries don't re-parse unchanged files.
//!
//! On top of the index, builds a symbol reference graph (edges from callers /
//! importers to the definitions they reference, intra-workspace only) and ranks
//! it with a simplified PageRank power iteration. `render(token_budget)`
//! produces an Aider-style file-tree summary of the top-ranked symbols, fit to
//! a token budget.
//!
//! This is the P1 "Ranked Context Engineering" repo-map deliverable, built
//! fresh on branch `p1`.

use super::tree_sitter_metrics::get_ts_language;
use super::Language;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
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
            "function_item" | "function_definition" | "function_declaration" | "arrow_function" | "function" => {
                Self::Function
            }
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
            "import" | "import_declaration" | "import_statement" | "use_declaration" | "use_item" => {
                Self::Import
            }
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
    ".git", ".svn", ".hg", "node_modules", "target", ".venv", "venv", "__pycache__", "dist",
    "build", ".next", ".nuxt", ".codegraph", ".omo",
];

/// File extension → language string mapping. Trimmed to grammars actually
/// wired in `tree_sitter_metrics::get_ts_language` (research finding: the
/// original extension list was ahead of grammar support).
const SUPPORTED_EXTS: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("js", "javascript"),
    ("jsx", "javascript"),
    ("py", "python"),
];

/// A reference edge in the symbol graph: `from` (caller/import symbol) refers
/// to `to` (the definition it names).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolRef {
    pub from: SymbolId,
    pub to: SymbolId,
}

/// Stable identity for a symbol inside the graph.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SymbolId {
    pub file_index: u32,
    pub symbol_index: u32,
}

/// The symbol reference graph — nodes are symbols, edges are intra-workspace
/// references (caller/import → definition).
#[derive(Debug, Clone, Default)]
pub struct SymbolGraph {
    pub nodes: Vec<Symbol>,
    pub edges: Vec<SymbolRef>,
}

impl SymbolGraph {
    /// Simplified PageRank via power iteration with a damping factor of 0.85.
    /// Returns a score per node index (aligned with `self.nodes`).
    pub fn pagerank(&self, iterations: u32) -> Vec<f64> {
        let n = self.nodes.len();
        if n == 0 {
            return vec![];
        }
        // Build adjacency: out-edges per node.
        let mut out: Vec<Vec<usize>> = vec![vec![]; n];
        let mut incoming: Vec<Vec<usize>> = vec![vec![]; n];
        for e in &self.edges {
            let from = e.from.symbol_index as usize;
            let to = e.to.symbol_index as usize;
            if from < n && to < n {
                out[from].push(to);
                incoming[to].push(from);
            }
        }

        let damping: f64 = 0.85;
        let base = (1.0 - damping) / n as f64;
        let mut rank: Vec<f64> = vec![1.0 / n as f64; n];

        for _ in 0..iterations.max(1) {
            let mut next: Vec<f64> = vec![base; n];
            for (from, targets) in out.iter().enumerate() {
                if targets.is_empty() {
                    // Dangling node: distribute uniformly to avoid rank sink.
                    let share = rank[from] / n as f64;
                    for r in next.iter_mut() {
                        *r += share * damping;
                    }
                    continue;
                }
                let share = rank[from] * damping / targets.len() as f64;
                for t in targets {
                    next[*t] += share;
                }
            }
            rank = next;
        }

        let _ = incoming; // retained for future edge-weighting refinements
        rank
    }
}

/// Repo-wide symbol index with graph ranking.
pub struct RepoMap {
    /// Map of file_path → symbols (eager load for active repo)
    pub files: HashMap<String, Vec<Symbol>>,
    /// LRU cache for on-demand re-parsing of changed files
    cache: SymbolCache,
    /// Set of file extensions we can parse
    supported_extensions: Vec<&'static str>,
    /// File path → index into the symbol graph node list
    file_index: HashMap<String, u32>,
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
            file_index: HashMap::new(),
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
                Ok(mut symbols) => {
                    // parse_file records the absolute path; the index is keyed
                    // by the repo-relative path — normalize so graph lookups match.
                    for s in symbols.iter_mut() {
                        s.file_path = rel_str.clone();
                    }
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
        self.rebuild_file_index();
        Ok(self.files.len())
    }

    /// Parse a single file and return its symbols.
    pub fn parse_file(&self, path: &Path) -> Result<Vec<Symbol>, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = match self.lang_for_ext(ext) {
            Some(l) => l,
            None => return Ok(vec![]),
        };

        let ts_lang = get_ts_language(&lang).ok_or_else(|| {
            format!(
                "No tree-sitter grammar for {:?} (extension {})",
                lang, ext
            )
        })?;

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
        let lang = match lang_str {
            "rust" => Language::Rust,
            "typescript" => Language::TypeScript,
            "javascript" => Language::JavaScript,
            "python" => Language::Python,
            _ => return None,
        };
        Some(lang)
    }

    /// Recursively extract symbols from tree-sitter AST nodes.
    fn extract_symbols(
        &self,
        node: &Node,
        source: &str,
        file_path: &str,
        out: &mut Vec<Symbol>,
    ) {
        let kind = node.kind();
        let symbol_kind = SymbolKind::from_ts_node_kind(kind);

        // Collect named definition nodes and call expressions.
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
        // Call expressions name the callee: resolve the `function` field and
        // take its last identifier segment (handles `crate::busy()` → `busy`).
        if node.kind() == "call_expression" || node.kind() == "call" {
            if let Some(fn_node) = node.child_by_field_name("function") {
                if let Ok(text) = fn_node.utf8_text(source.as_bytes()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let last_segment = trimmed
                            .rsplit("::")
                            .next()
                            .unwrap_or(trimmed)
                            .trim();
                        if !last_segment.is_empty() {
                            return Some(last_segment.to_string());
                        }
                    }
                }
            }
        }
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
        node.utf8_text(source.as_bytes())
            .ok()
            .and_then(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return None;
                }
                if trimmed.len() > 200 {
                    Some(format!("{}...", &trimmed[..200]))
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
        self.file_index.clear();
    }

    fn rebuild_file_index(&mut self) {
        self.file_index.clear();
        for (i, path) in self.files.keys().enumerate() {
            self.file_index.insert(path.clone(), i as u32);
        }
    }

    /// Build the symbol reference graph. Nodes are every definition symbol;
    /// edges connect a caller/import symbol to the definition it names when
    /// that definition is in-workspace (same name, different file).
    pub fn build_graph(&self) -> SymbolGraph {
        let mut graph = SymbolGraph::default();
        let mut node_of: HashMap<(String, String, SymbolKind), usize> = HashMap::new();
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();

        // Collect definition nodes first (skip Call/Import/Unknown as nodes).
        let mut file_order: Vec<&String> = self.files.keys().collect();
        file_order.sort();
        for file in &file_order {
            if let Some(symbols) = self.files.get(*file) {
                for s in symbols {
                    let is_def = !matches!(
                        s.kind,
                        SymbolKind::Call | SymbolKind::Import | SymbolKind::Unknown
                    );
                    if !is_def {
                        continue;
                    }
                    let idx = graph.nodes.len();
                    graph.nodes.push(s.clone());
                    node_of.insert((s.name.clone(), s.file_path.clone(), s.kind), idx);
                    by_name
                        .entry(s.name.clone())
                        .or_insert_with(Vec::new)
                        .push(idx);
                }
            }
        }

        // Add edges: Call/Import symbol in file F references a definition named
        // X elsewhere → edge (caller file F's symbol → definition).
        for file in &file_order {
            if let Some(symbols) = self.files.get(*file) {
                for s in symbols {
                    if s.kind != SymbolKind::Call && s.kind != SymbolKind::Import {
                        continue;
                    }
                    // Caller symbol: locate a representative node for its file.
                    let caller_id = match node_of.get(&(
                        s.name.clone(),
                        s.file_path.clone(),
                        SymbolKind::Function,
                    )) {
                        Some(&idx) => idx,
                        None => {
                            // Fall back to first definition in the same file.
                            match self.files.get(&s.file_path) {
                                Some(defs) => match defs
                                    .iter()
                                    .position(|d| !matches!(d.kind, SymbolKind::Call))
                                {
                                    Some(pos) => {
                                        let key = (
                                            defs[pos].name.clone(),
                                            defs[pos].file_path.clone(),
                                            defs[pos].kind,
                                        );
                                        match node_of.get(&key) {
                                            Some(&i) => i,
                                            None => continue,
                                        }
                                    }
                                    None => continue,
                                },
                                None => continue,
                            }
                        }
                    };

                    if let Some(targets) = by_name.get(&s.name) {
                        for &to in targets {
                            let to_node = &graph.nodes[to];
                            // Intra-workspace only: skip same-file self-reference.
                            if to_node.file_path == s.file_path {
                                continue;
                            }
                            let from = graph.nodes[caller_id].clone();
                            let to_sym = to_node.clone();
                            let from_id = SymbolId {
                                file_index: *self
                                    .file_index
                                    .get(&from.file_path)
                                    .unwrap_or(&0),
                                symbol_index: caller_id as u32,
                            };
                            let to_id = SymbolId {
                                file_index: *self
                                    .file_index
                                    .get(&to_sym.file_path)
                                    .unwrap_or(&0),
                                symbol_index: to as u32,
                            };
                            graph.edges.push(SymbolRef {
                                from: from_id,
                                to: to_id,
                            });
                        }
                    }
                }
            }
        }

        graph
    }

    /// Rank symbols by simplified PageRank over the reference graph.
    /// Returns a map symbol_index → score (aligned with `build_graph().nodes`).
    pub fn graph_rank(&self, iterations: u32) -> HashMap<usize, f64> {
        let graph = self.build_graph();
        let ranks = graph.pagerank(iterations);
        let mut map = HashMap::new();
        for (i, score) in ranks.iter().enumerate() {
            if *score > 0.0 {
                map.insert(i, *score);
            }
        }
        map
    }

    /// Render the repo map within a token budget, ranked by PageRank.
    ///
    /// Produces an Aider-style file tree: files grouped by directory, ranked
    /// symbols under their file, top-K fit to `token_budget` (chars/4
    /// estimate, consistent with the crate's zero-dependency baseline).
    /// Rank annotations are omitted from the LLM-visible output (selection
    /// only), matching Aider's final render.
    pub fn render(&self, token_budget: u32) -> String {
        if self.files.is_empty() {
            return String::new();
        }

        let graph = self.build_graph();
        let ranks = graph.pagerank(20);

        // Map node index → (file, symbol) for ranked selection.
        let mut ranked: Vec<(f64, &Symbol)> = graph
            .nodes
            .iter()
            .zip(ranks.iter())
            .map(|(s, r)| (*r, s))
            .collect();
        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Greedy fit: take ranked symbols until the budget is consumed.
        let mut used = 0usize; // chars
        let mut by_file: HashMap<String, Vec<&Symbol>> = HashMap::new();
        let budget_chars = (token_budget as usize) * 4;
        let header = format!("# Repo map ({} symbols)\n", graph.nodes.len());
        used += header.len();

        for (_score, sym) in ranked {
            if sym.kind == SymbolKind::Call || sym.kind == SymbolKind::Unknown {
                continue;
            }
            let line = format!("    {} {}\n", symbol_prefix(sym.kind), sym.name);
            let cost = 4 + line.len() + sym.file_path.len();
            if used + cost > budget_chars && !by_file.is_empty() {
                break;
            }
            used += cost;
            by_file.entry(sym.file_path.clone()).or_default().push(sym);
        }

        if by_file.is_empty() {
            return String::new();
        }

        // Group by directory for a compact tree.
        let mut dirs: HashMap<String, Vec<&String>> = HashMap::new();
        let mut file_paths: Vec<&String> = by_file.keys().collect();
        file_paths.sort();
        for fp in &file_paths {
            let dir = match fp.rfind('/') {
                Some(i) => &fp[..i],
                None => ".",
            };
            dirs.entry(dir.to_string())
                .or_insert_with(Vec::new)
                .push(*fp);
        }

        let mut out = header;
        let mut dir_keys: Vec<&String> = dirs.keys().collect();
        dir_keys.sort();
        for dir in dir_keys {
            out.push_str(&format!("{}/\n", dir));
            let mut files = dirs[dir].clone();
            files.sort();
            for fp in files {
                out.push_str(&format!("  {}\n", fp));
                let mut syms = by_file[fp].clone();
                syms.sort_by(|a, b| a.start_line.cmp(&b.start_line));
                for s in syms {
                    out.push_str(&format!(
                        "    {} {} [L{}]\n",
                        symbol_prefix(s.kind),
                        s.name,
                        s.start_line
                    ));
                }
            }
        }

        out
    }
}

fn symbol_prefix(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "fn",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Impl => "impl",
        SymbolKind::Class => "class",
        SymbolKind::Interface => "interface",
        SymbolKind::TypeAlias => "type",
        SymbolKind::Const => "const",
        SymbolKind::Variable => "let",
        SymbolKind::Import => "use",
        SymbolKind::Call => "call",
        SymbolKind::Module => "mod",
        SymbolKind::Unknown => "?",
    }
}

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
        std::fs::write(&file, "def my_func():\n    pass\nclass MyClass:\n    pass\n").unwrap();

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
        assert!(count >= 2, "expected at least 2 indexed files, got {}", count);

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
        std::fs::write(temp.path().join("node_modules").join("skip.rs"), "fn skip() {}").unwrap();

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

    #[test]
    fn build_graph_connects_callers_to_definitions() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("lib.rs"),
            "pub fn helper() {}\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("main.rs"),
            "mod lib;\nfn main() { lib::helper(); }\n",
        )
        .unwrap();

        map.index_repo(temp.path()).unwrap();
        let graph = map.build_graph();

        // helper() should be referenced by main.rs's call symbol.
        let helper_nodes: Vec<&Symbol> = graph
            .nodes
            .iter()
            .filter(|s| s.name == "helper")
            .collect();
        assert_eq!(helper_nodes.len(), 1, "helper def should be a node");

        let has_edge = graph.edges.iter().any(|e| {
            let to = &graph.nodes[e.to.symbol_index as usize];
            to.name == "helper"
        });
        assert!(has_edge, "expected an edge into helper, got {:?}", graph.edges);
    }

    #[test]
    fn pagerank_ranks_referenced_symbols_higher() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();
        // busy() referenced by two callers, idle() referenced by none
        std::fs::write(
            temp.path().join("a.rs"),
            "pub fn busy() {}\npub fn idle() {}\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("b.rs"),
            "fn caller1() { crate::busy(); }\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("c.rs"),
            "fn caller2() { crate::busy(); }\n",
        )
        .unwrap();

        map.index_repo(temp.path()).unwrap();
        let graph = map.build_graph();
        let ranks = graph.pagerank(20);

        let busy_idx = graph
            .nodes
            .iter()
            .position(|s| s.name == "busy")
            .unwrap();
        let idle_idx = graph.nodes.iter().position(|s| s.name == "idle").unwrap();
        assert!(
            ranks[busy_idx] > ranks[idle_idx],
            "busy should outrank idle: busy={} idle={}",
            ranks[busy_idx],
            ranks[idle_idx]
        );
    }

    #[test]
    fn render_respects_budget_and_includes_top_symbols() {
        let mut map = RepoMap::new();
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("mod.rs"),
            "pub fn important() {}\npub fn minor() {}\npub fn unused() {}\n",
        )
        .unwrap();

        map.index_repo(temp.path()).unwrap();
        let rendered = map.render(200);
        assert!(rendered.contains("important"), "top symbol should render");
        assert!(
            rendered.len() <= 200 * 4 + 512,
            "render exceeded budget slack: {} chars",
            rendered.len()
        );
    }
}
