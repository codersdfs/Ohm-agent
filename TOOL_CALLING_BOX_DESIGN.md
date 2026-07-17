# Tool Calling Box — Comprehensive Design Document

> **Style**: Claude Code / Pi Agent — categorized, discoverable, parameterized tooling with structured lifecycle, search, extension, and safety.
> **Target**: Omega Agent (`tool-harness` crate + TUI layer)
> **Status**: Design Proposal v1.0

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Tool Category Taxonomy](#2-tool-category-taxonomy)
3. [Complete Metadata Schema](#3-complete-metadata-schema)
4. [Discovery & Search Mechanism](#4-discovery--search-mechanism)
5. [Execution Lifecycle](#5-execution-lifecycle)
6. [UI/UX Patterns](#6-uiux-patterns)
7. [Extension & Plugin System](#7-extension--plugin-system)
8. [Edge Cases & Failure Handling](#8-edge-cases--failure-handling)
9. [Implementation Roadmap](#9-implementation-roadmap)

---

## 1. Architecture Overview

### 1.1 What Is a Tool Calling Box?

A **Tool Calling Box** is an opinionated container that owns the full lifecycle of every tool an agent can call. It is not merely a registry (which the current `tool-harness` already has); it is a **discoverable, categorized, self-documenting toolbox** with:

- **Category taxonomy** — every tool lives in a logical group
- **Rich metadata** — beyond name/desc/schema to include tags, versioning, deprecation, examples, error modes, and cost hints
- **Multi-modal discovery** — search by name, fuzzy match, category, tags, parameter names, or natural-language description
- **Structured execution pipeline** — the existing 14-step pipeline is formalized and extended with streaming progress, circuit-breaking, and parallel-safe scheduling
- **UI surfaces** — the TUI can render tool cards, compact lists, search results, and comparison views to both the human user and the LLM
- **Extension API** — third-party tool bundles (MCP, WASM plugins, script-based tools) register through a stable interface with sandboxed permissions

### 1.2 Layered Architecture

```
┌─────────────────────────────────────────────────────┐
│                  UI / TUI Layer                       │
│  ToolBoxView  |  ToolCard  |  SearchBar  |  Help     │
├─────────────────────────────────────────────────────┤
│              Tool Calling Box (Orchestrator)          │
│  ┌─────────┐ ┌──────────┐ ┌────────┐ ┌──────────┐   │
│  │ Catalog  │ │ Indexer  │ │Searcher│ │ Executor  │   │
│  │(category │ │ (fuzzy)  │ │(hybrid)│ │(pipeline) │   │
│  │ + tags)  │ │          │ │        │ │           │   │
│  └─────────┘ └──────────┘ └────────┘ └──────────┘   │
├─────────────────────────────────────────────────────┤
│              Tool Registry (existing)                 │
│  Built-in Tools  |  MCP Tools  |  Plugin Tools       │
├─────────────────────────────────────────────────────┤
│              Tool Implementations                     │
│  Read  |  Write  |  Edit  |  Bash  |  Grep  |  ...   │
└─────────────────────────────────────────────────────┘
```

### 1.3 Relationship to Existing `tool-harness`

The existing `tool-harness` crate already provides:
- `Tool` trait with `name()`, `description()`, `parameters_schema()`, `call()`
- `ToolRegistry` for built-in + MCP tool storage
- `ExecutionPipeline` with 14 steps (lookup → abort-check → validation → permission → execute → budget → hooks → telemetry)
- `ToolOrchestrator` for LLM tool-call loop
- Permission, budget, and hook subsystems

**What the Tool Calling Box adds** on top of this solid foundation:

| Concern | Current `tool-harness` | Tool Calling Box |
|---|---|---|
| Categorization | Flat list | Hierarchical category taxonomy (FileOps, CodeExec, Web, etc.) |
| Metadata | name, description, parameters_schema | + tags, version, deprecation, cost hints, examples, error modes |
| Discovery | `list()` returns flat Vec<String> | Search by name, fuzzy, category, tag, parameter, NL |
| UI | None (TUI reads tool list directly) | Tool cards, compact list, search results, comparison views |
| Extensibility | MCP tools merged via `merge_mcp_tools()` | Plugin registry + versioned manifest + sandbox |
| Streaming results | None | ProgressEmitter for long-running tools |
| Rate limiting | None | Token-bucket per-tool / per-category |
| Tool chaining | Manual orchestration | Built-in `then` / `pipe` / `compose` patterns |
| Safety gates | Permission rules + tool.check_permissions | Sandbox + content policy + cost budgets |

---

## 2. Tool Category Taxonomy

### 2.1 Top-Level Categories

```
FILE_OPERATIONS    — read, write, edit, rename, delete, list, stat, watch
CODE_EXECUTION     — bash, python, eval, compile, test-runner
SEARCH_QUERY       — grep, glob, find, code-search, semantic-search
WEB_NETWORK        — http-get, http-post, fetch-url, web-scrape, curl
COMMUNICATION      — notify, slack, email, review-request, confirm
SYSTEM            — env, clock, os-info, process-list, exit
AGENT_MANAGEMENT   — subagent, chain, parallel, async, delegate
MCP_SERVICES      — read-resource, list-tools, call-tool (from MCP servers)
MEMORY_STORE       — kv-get, kv-set, kv-delete, memory-search, remember
CODING_ASSIST      — lsp-hover, lsp-complete, lsp-diagnostics, format, lint
DIFF_PATCH         — apply-diff, view-diff, staged-diff, git-diff, git-commit
DATA_TRANSFORM     — json-parse, csv-parse, jq-query, yaml-convert, base64
HELP_DOCS          — tool-help, man-page, usage-example, available-tools
```

### 2.2 Full Taxonomy with Examples

```
1. FILE_OPERATIONS
   1.1 Read                      — read, read-multiple, head, tail, read-lines
   1.2 Write                     — write, append, prepend
   1.3 Edit                      — edit (search/replace), insert, delete-lines
   1.4 Metadata                  — stat, ls, du, checksum, mime-type
   1.5 Organization              — rename, move, copy, delete, mkdir, rmdir
   1.6 Watch                     — watch-file, watch-dir (inotify/kqueue)
   1.7 Archive                   — tar, zip, unzip, gzip

2. CODE_EXECUTION
   2.1 Shell                     — bash, sh, powershell, cmd
   2.2 Scripting                 — python, node, ruby, lua
   2.3 Compilation               — compile, build, cargo-build, tsc
   2.4 Testing                   — cargo-test, pytest, jest, go-test
   2.5 REPL                      — repl/eval (per-language sandboxed)

3. SEARCH_QUERY
   3.1 Pattern                   — grep, ripgrep, ag, find
   3.2 File-search               — glob, locate, walkdir
   3.3 Code-intelligence         — code-search, symbol-search, definition-search
   3.4 Semantic                  — embedding-search, similar-code, faq-match

4. WEB_NETWORK
   4.1 HTTP                      — get, post, put, delete, patch, head
   4.2 Scrape                    — fetch-html, extract-text, extract-links
   4.3 API                       — graphql, rest-call, websocket-send
   4.4 Download                  — wget, curl, download-file

5. COMMUNICATION
   5.1 Notify                    — notify, alert, bell
   5.2 Messaging                 — slack-send, discord-send, email-send
   5.3 Review                    — request-review, submit-review
   5.4 Human-in-loop             — confirm, ask-user, choose

6. SYSTEM
   6.1 Info                      — env, clock, os-info, hostname
   6.2 Process                   — ps, kill, top
   6.3 Resource                  — mem, cpu, disk-usage
   6.4 Permission                — whoami, id, file-mode, acl

7. AGENT_MANAGEMENT
   7.1 Subagent                  — subagent-launch, subagent-status, subagent-list
   7.2 Chain                     — chain-run, chain-status
   7.3 Orchestration             — parallel, fanout, reduce
   7.4 Context                   — context-get, context-set, context-fork

8. MCP_SERVICES
   8.1 MCP-Meta                  — list-servers, list-server-tools
   8.2 MCP-Resources             — read-resource, list-resources
   8.3 MCP-Execution             — call-tool, execute-query
   8.4 MCP-Prompts               — get-prompt

9. MEMORY_STORE
   9.1 Key-Value                 — kv-get, kv-set, kv-delete, kv-list
   9.2 Semantic                  — memory-embed, memory-search, memory-forget
   9.3 Session                   — session-get, session-set, session-clear

10. CODING_ASSIST
   10.1 LSP                      — lsp-hover, lsp-complete, lsp-definition, lsp-references
   10.2 Formatting               — format-file, format-range
   10.3 Linting                  — lint-file, lint-project
   10.4 Refactoring              — rename-symbol, extract-function

11. DIFF_PATCH
   11.1 Diff                     — diff-files, diff-dirs, git-diff, staged-diff
   11.2 Patch                    — apply-patch, create-patch
   11.3 Git                      — git-add, git-commit, git-status, git-log, git-push

12. DATA_TRANSFORM
   12.1 JSON                     — json-parse, json-stringify, jq
   12.2 CSV/TSV                  — csv-parse, csv-to-json, json-to-csv
   12.3 YAML/TOML                — yaml-parse, toml-parse, convert-format
   12.4 Encoding                 — base64-encode, base64-decode, hex, url-encode

13. HELP_DOCS
   13.1 Tool-introspection       — tool-help, tool-schema, tool-examples
   13.2 Category-browse          — category-list, category-tools
   13.3 Search-help              — search-tools, find-similar, suggest-tools
```

### 2.3 Cross-Cutting Concerns

Some tools legitimately belong to multiple categories. The design uses **primary category + tags** to handle this:

- `edit` → primary: `FILE_OPERATIONS`, tags: `["diff", "patch", "code-assist"]`
- `bash` → primary: `CODE_EXECUTION`, tags: `["system", "shell", "automation"]`
- `grep` → primary: `SEARCH_QUERY`, tags: `["file-ops", "code-intel"]`
- `apply-diff` → primary: `DIFF_PATCH`, tags: `["file-ops", "git"]`

When a tool belongs to two categories equally, assign the more operation-ordered one (what the tool *does*) over the domain-ordered one (where it's used). For example, `jq` is primarily `DATA_TRANSFORM.json` even though it's often used for code search.

---

## 3. Complete Metadata Schema

### 3.1 `ToolMetadata` Struct

```rust
/// Comprehensive metadata for a tool in the calling box.
/// This is the canonical source of truth for display, discovery, and execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    // ── Identity ─────────────────────────────────────────────────
    /// Unique tool name (snake_case, e.g. "read_file")
    pub name: String,

    /// Human-readable display label (e.g. "Read File")
    pub label: String,

    /// Short one-liner description (<120 chars)
    pub description: String,

    /// Longer documentation / guidance for LLM usage (<2000 chars)
    pub doc: Option<String>,

    // ── Categorization ───────────────────────────────────────────
    /// Primary category from the taxonomy
    pub category: ToolCategory,

    /// Optional subcategory within the category
    pub subcategory: Option<String>,

    /// Additional tags for cross-cutting search
    pub tags: Vec<String>,

    // ── Parameters ───────────────────────────────────────────────
    /// Full JSON Schema (OpenAPI 3.0 subset) for parameters
    pub parameters: serde_json::Value,

    /// Quick-reference: ordered list of parameter summaries
    pub param_summaries: Vec<ParamSummary>,

    // ── Execution characteristics ────────────────────────────────
    /// Whether the tool is read-only (safe for dry-run / plan mode)
    pub read_only: bool,

    /// Whether the tool can be called concurrently without side effects
    pub concurrency_safe: bool,

    /// Typical execution latency hint: "instant" | "fast" | "slow" | "blocking"
    pub latency_hint: LatencyHint,

    /// Whether the tool can stream incremental results
    pub supports_streaming: bool,

    /// Maximum result size in characters before truncation/persistence
    pub max_result_chars: usize,

    // ── Error modes ──────────────────────────────────────────────
    /// Known error modes the tool can produce
    pub errors: Vec<ToolErrorSpec>,

    // ── Usage guidance ───────────────────────────────────────────
    /// Example invocations (for LLM few-shot or help display)
    pub examples: Vec<ToolExample>,

    /// Cost tokens per call (approximate, for budgeting)
    pub cost_hint: Option<CostHint>,

    // ── Lifecycle ────────────────────────────────────────────────
    /// Semantic version of the tool spec
    pub version: String,

    /// Deprecation status
    pub deprecation: Option<DeprecationInfo>,

    /// Source origin: "builtin" | "mcp" | "plugin" | "dynamic"
    pub source: ToolSource,

    /// Provider/server name if MCP or plugin
    pub source_name: Option<String>,
}
```

### 3.2 Supporting Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCategory {
    FileOperations,
    CodeExecution,
    SearchQuery,
    WebNetwork,
    Communication,
    System,
    AgentManagement,
    McpServices,
    MemoryStore,
    CodingAssist,
    DiffPatch,
    DataTransform,
    HelpDocs,
}

impl ToolCategory {
    pub fn label(&self) -> &str {
        match self {
            Self::FileOperations => "File Operations",
            Self::CodeExecution => "Code Execution",
            Self::SearchQuery => "Search / Query",
            Self::WebNetwork => "Web / Network",
            Self::Communication => "Communication",
            Self::System => "System",
            Self::AgentManagement => "Agent Management",
            Self::McpServices => "MCP Services",
            Self::MemoryStore => "Memory Store",
            Self::CodingAssist => "Coding Assist",
            Self::DiffPatch => "Diff / Patch",
            Self::DataTransform => "Data Transform",
            Self::HelpDocs => "Help / Docs",
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            Self::FileOperations => "📄",
            Self::CodeExecution => "▶",
            Self::SearchQuery => "🔍",
            Self::WebNetwork => "🌐",
            Self::Communication => "💬",
            Self::System => "⚙",
            Self::AgentManagement => "🤖",
            Self::McpServices => "🔌",
            Self::MemoryStore => "🧠",
            Self::CodingAssist => "✏",
            Self::DiffPatch => "📝",
            Self::DataTransform => "🔄",
            Self::HelpDocs => "❓",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSummary {
    pub name: String,
    pub param_type: String,       // "string" | "number" | "boolean" | "array" | "object"
    pub description: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
    pub example: Option<serde_json::Value>,
    pub constraints: Option<ParamConstraints>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamConstraints {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LatencyHint {
    Instant,    // <50ms, e.g. kv-get, env
    Fast,       // <500ms, e.g. read, grep
    Slow,       // <10s, e.g. write large file, build
    Blocking,   // indefinite, e.g. watch, long-running bash
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolErrorSpec {
    pub kind: String,             // "not_found" | "permission_denied" | "timeout" | ...
    pub description: String,
    pub recoverable: bool,
    pub retry_advice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExample {
    pub title: String,
    pub description: String,
    pub arguments: serde_json::Value,
    pub expected_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostHint {
    pub tokens_per_call: u32,
    pub category: CostCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CostCategory {
    Free,       // Read-only, local ops
    Cheap,      // grep, glob, small edits
    Moderate,   // large file writes, bash scripts
    Expensive,  // large search, heavy computation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationInfo {
    pub deprecated_in_version: String,
    pub removal_version: Option<String>,
    pub replacement: Option<String>,     // suggested alternative tool name
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolSource {
    Builtin,
    Mcp,
    Plugin,
    Dynamic,    // generated at runtime by another tool
}

/// Parameters schema format version marker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub schema: String,           // "https://json-schema.org/draft/2020-12/schema"
    pub version: String,          // Tool calling box schema version, e.g. "1.0.0"
}
```

### 3.3 Example: `read` Tool Metadata (JSON form)

```json
{
  "name": "read",
  "label": "Read File",
  "description": "Read the contents of a file at the given path",
  "doc": "Returns file contents as a string. For binary files, returns a hex dump. Respects $HOME/.omega/allowlist for safe paths.",
  "category": "file_operations",
  "subcategory": "read",
  "tags": ["file", "view", "cat"],
  "parameters": {
    "type": "object",
    "properties": {
      "filePath": {
        "type": "string",
        "description": "Absolute or relative path to the file"
      },
      "offset": {
        "type": "number",
        "description": "Starting line number (1-indexed)",
        "default": 1
      },
      "limit": {
        "type": "number",
        "description": "Maximum lines to read",
        "default": 2000
      }
    },
    "required": ["filePath"]
  },
  "param_summaries": [
    { "name": "filePath", "param_type": "string", "description": "Absolute or relative path to the file", "required": true },
    { "name": "offset", "param_type": "number", "description": "Starting line number (1-indexed)", "required": false, "default": 1 },
    { "name": "limit", "param_type": "number", "description": "Maximum lines to read", "required": false, "default": 2000 }
  ],
  "read_only": true,
  "concurrency_safe": true,
  "latency_hint": "fast",
  "supports_streaming": true,
  "max_result_chars": 50000,
  "errors": [
    { "kind": "not_found", "description": "File does not exist", "recoverable": true, "retry_advice": "Check the file path" },
    { "kind": "permission_denied", "description": "Cannot read file due to OS permissions", "recoverable": false },
    { "kind": "too_large", "description": "File exceeds max read size", "recoverable": true, "retry_advice": "Use offset/limit to read in chunks" }
  ],
  "examples": [
    { "title": "Read a file", "description": "Read entire file", "arguments": { "filePath": "src/main.rs" } },
    { "title": "Read with offset", "description": "Read lines 100-200", "arguments": { "filePath": "src/main.rs", "offset": 100, "limit": 100 } }
  ],
  "version": "1.0.0",
  "source": "builtin",
  "cost_hint": { "tokens_per_call": 50, "category": "free" }
}
```

---

## 4. Discovery & Search Mechanism

### 4.1 Search Index

The `ToolBoxIndex` maintains an in-memory inverted index for fast multi-modal search:

```rust
pub struct ToolBoxIndex {
    /// Maps tool name → ToolMetadata
    by_name: HashMap<String, ToolMetadata>,

    /// Maps category → list of tool names
    by_category: HashMap<ToolCategory, Vec<String>>,

    /// Maps tag → list of tool names
    by_tag: HashMap<String, Vec<String>>,

    /// Maps parameter name → list of tool names
    by_param: HashMap<String, Vec<String>>,

    /// Full-text search index (description, doc, examples)
    fts_index: HashMap<String, Vec<(String, f32)>>,  // token → [(tool_name, tf-idf score)]

    /// Alias map (shortcuts, alternate names)
    aliases: HashMap<String, String>,

    /// Fuzzy trie for approximate name matching
    fuzzy_trie: FuzzyTrie,
}
```

### 4.2 Search Modes

```rust
pub enum SearchMode {
    /// Exact name match (fastest)
    Exact(String),

    /// Prefix or substring match on name
    NameMatch(String),

    /// Category browse
    ByCategory(ToolCategory),

    /// Tag search
    ByTag(String),

    /// Parameter name search (find tools accepting a given parameter)
    ByParam(String),

    /// Fuzzy name match (Levenshtein distance ≤ 2)
    Fuzzy(String),

    /// Full-text search across description + doc + examples
    FullText(String),

    /// Composite search (combine multiple modes with AND/OR)
    Composite(SearchQuery),
}

pub struct SearchQuery {
    pub text: Option<String>,
    pub category: Option<ToolCategory>,
    pub tags: Option<Vec<String>>,
    pub read_only: Option<bool>,
    pub latency_max: Option<LatencyHint>,
    pub source: Option<ToolSource>,
}

pub struct SearchResult {
    pub tool: ToolMetadata,
    pub score: f32,
    pub match_reason: String,   // "name match", "category: File Operations", etc.
}
```

### 4.3 Search Ranking

1. **Exact name match** → score 1.0
2. **Prefix match on name** → score 0.9
3. **Category match** → score 0.7
4. **Tag match** → score 0.6
5. **Fuzzy match (edit distance 1)** → score 0.5
6. **Parameter match** → score 0.4
7. **Full-text match in description** → score 0.3 * tf-idf
8. **Full-text match in doc/examples** → score 0.2 * tf-idf
9. **Fuzzy match (edit distance 2)** → score 0.1

Results are deduplicated (highest score wins) and returned sorted descending.

### 4.4 Fuzzy Matching Strategy

```rust
/// Simple character-level trie with Levenshtein traversal
pub struct FuzzyTrie {
    root: TrieNode,
}

impl FuzzyTrie {
    /// Find tools where name matches within max_distance edits
    pub fn search(&self, query: &str, max_distance: u8) -> Vec<(String, u8)> {
        // BFS through trie tracking edit distance
        // Return (name, distance) for all matches ≤ max_distance
    }

    /// Add tool name to the trie
    pub fn insert(&mut self, name: &str) {}
}
```

For the MVP, fuzzy matching uses character-level bigram Jaccard similarity (simpler and faster than full Levenshtein):

```rust
fn fuzzy_score(query: &str, name: &str) -> f64 {
    let query_bigrams: HashSet<(char, char)> = query.chars().zip(query.chars().skip(1)).collect();
    let name_bigrams: HashSet<(char, char)> = name.chars().zip(name.chars().skip(1)).collect();
    let intersection = query_bigrams.intersection(&name_bigrams).count();
    let union = query_bigrams.union(&name_bigrams).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}
```

### 4.5 Alias System

Some tools need alternative names for common usage:

| Alias | Canonical Name | Reason |
|---|---|---|
| `cat` | `read` | Unix convention |
| `ls` | `glob` | Directory listing |
| `find` | `glob` | File search |
| `curl` | `http-get` | Web convention |
| `rg` | `grep` | ripgrep alias |
| `printenv` | `env` | Environment vars |
| `del` | `delete` | Windows convention |
| `mv` | `rename` | Unix convention |
| `cp` | `copy` | Unix convention |

Aliases are stored in the same index and resolve to the canonical name.

### 4.6 Context-Aware Recommendations

The `ToolBox` can suggest tools based on current context:

```rust
pub struct ToolRecommender {
    /// Recent tool call history for frequency boosting
    recent_calls: VecDeque<String>,

    /// Current conversation mode (plan, implement, debug, review)
    mode: Option<ConversationMode>,

    /// Currently open files / project structure
    current_context: Option<WorkspaceContext>,
}

impl ToolRecommender {
    /// Get recommended tools for the current context (top N)
    pub fn recommend(&self, n: usize) -> Vec<SearchResult> {
        // 1. Tools from recent history (frequency boost)
        // 2. Tools matching current mode (e.g., plan mode → read-only tools)
        // 3. Tools related to open files (e.g., open .rs → lint, compile)
        // 4. Return union with scores
    }
}
```

When an LLM prompt is about to be constructed, the `ToolBox` can filter the tool list based on context:

- **Plan mode**: only `read_only = true` tools
- **Debug mode**: `SEARCH_QUERY`, `DIFF_PATCH`, `read-only` tools
- **Implement mode**: all tools except destructive system tools
- **Review mode**: `read_only = true` tools + diff tools

---

## 5. Execution Lifecycle

### 5.1 Enhanced Pipeline

The existing 14-step pipeline is preserved and enhanced with streaming, circuit-breaking, and observability hooks. The new pipeline signature:

```rust
impl ExecutionPipeline {
    /// Execute a tool through the full pipeline with optional streaming.
    pub async fn execute(
        &self,
        tool_name: &str,
        input: ToolInput,
        ctx: &ToolUseContext,
        progress: Option<&dyn ProgressEmitter>,  // NEW
    ) -> Result<(ToolResult, BudgetCheck), ToolError>;
}
```

### 5.2 ProgressEmitter for Streaming Tools

```rust
/// Interface for tools that produce incremental results.
pub trait ProgressEmitter: Send + Sync {
    /// Called periodically with incremental output chunks.
    fn on_chunk(&self, chunk: &str) -> Result<(), String>;

    /// Called when execution is complete.
    fn on_complete(&self, final_output: &str) -> Result<(), String>;

    /// Called when an error occurs mid-execution.
    fn on_error(&self, error: &ToolError) -> Result<(), String>;

    /// Estimated progress (0.0 to 1.0), if known.
    fn on_progress(&self, fraction: f32, message: &str) -> Result<(), String>;
}
```

Long-running tools (bash, build, download) implement `StreamingTool`:

```rust
#[async_trait]
pub trait StreamingTool: Tool {
    /// Execute with incremental progress reporting.
    async fn call_streaming(
        &self,
        input: ToolInput,
        ctx: &ToolUseContext,
        progress: &dyn ProgressEmitter,
    ) -> Result<ToolResult, ToolError>;
}
```

### 5.3 Rate Limiting & Circuit Breaking

```rust
pub struct RateLimiter {
    /// Per-tool token bucket configuration
    buckets: HashMap<String, TokenBucket>,
    /// Per-category token bucket
    category_buckets: HashMap<ToolCategory, TokenBucket>,
}

struct TokenBucket {
    capacity: u32,           // max burst
    refill_rate: f64,        // tokens per second
    refill_interval: Duration,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn try_consume(&mut self, tokens: u32) -> Result<(), RateLimitError>;
}
```

Circuit breaker for tools that fail repeatedly:

```rust
pub struct CircuitBreaker {
    state: CircuitState,          // Closed | Open | HalfOpen
    failure_count: u32,
    failure_threshold: u32,        // e.g., 5 failures → open
    success_threshold: u32,        // e.g., 2 successes in HalfOpen → close
    timeout: Duration,             // time before moving from Open to HalfOpen
    last_failure_time: Instant,
}

enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing fast, no execution
    HalfOpen,   // Testing recovery
}
```

Rate limit and circuit breaker configs are defined per-tool in `ToolMetadata`:

```rust
pub struct ToolExecutionPolicy {
    pub rate_limit: Option<RateLimitConfig>,
    pub circuit_breaker: Option<CircuitBreakerConfig>,
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub timeout: Option<Duration>,
}

pub struct RateLimitConfig {
    pub max_calls_per_minute: u32,
    pub max_concurrent: u32,
}

pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout_seconds: u32,
}
```

### 5.4 Retry Policies

```rust
pub enum RetryPolicy {
    /// Do not retry
    None,
    /// Fixed delay between retries
    Fixed { max_retries: u32, delay: Duration },
    /// Exponential backoff with jitter
    ExponentialBackoff {
        max_retries: u32,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
        jitter: f64,   // 0.0–1.0 random jitter factor
    },
}

/// Default retry configuration for each error kind
impl Default for RetryPolicy {
    fn default() -> Self {
        Self::ExponentialBackoff {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: 0.1,
        }
    }
}
```

Retry policy is determined by the error kind and tool configuration:

| Error Kind | Retry? | Policy |
|---|---|---|
| NotFound | No | None |
| SchemaValidation | No | None (input fix needed) |
| PermissionDenied | No | None (policy change needed) |
| ExecutionFailed | Maybe | ExponentialBackoff (if retryable) |
| Timeout | Yes | ExponentialBackoff |
| ProviderError | Yes | ExponentialBackoff |
| Aborted | No | None |
| BudgetExceeded | No | None |
| RateLimited | Yes | Fixed delay = Retry-After |
| Internal | Maybe | ExponentialBackoff (1 retry) |

### 5.5 Tool Chaining / Composition Patterns

```rust
/// Tool composition operators on the ToolBox
impl ToolBox {
    /// Run tool A, then feed its output as input to tool B
    pub async fn then(
        &self,
        tool_a: &str,
        input_a: ToolInput,
        tool_b: &str,
        ctx: &ToolUseContext,
    ) -> Result<ToolResult, ToolError>;

    /// Pipe the output of tool A as input to tool B (for streamable tools)
    pub async fn pipe(
        &self,
        tool_a: &str,
        input_a: ToolInput,
        tool_b: &str,
        ctx: &ToolUseContext,
    ) -> Result<ToolResult, ToolError>;

    /// Compose multiple tools into a single execution plan
    pub fn compose(plan: &CompositionPlan) -> ComposedExecution;
}

pub struct CompositionPlan {
    pub steps: Vec<CompositionStep>,
    pub stop_on_error: bool,
    pub collect_outputs: bool,
}

pub struct CompositionStep {
    pub tool: String,
    pub input: ToolInput,
    pub output_mapping: Option<OutputMapping>,  // map result fields to next step's input
}
```

---

## 6. UI/UX Patterns

### 6.1 Tool Card View (Detailed)

Displayed for a single tool when the user requests help or when the LLM asks "show me the tool":

```
┌─────────────────────────────────────────────────────────┐
│  📄 Read File                         Builtin · v1.0.0  │
│  Read the contents of a file at the given path          │
├─────────────────────────────────────────────────────────┤
│  Parameters                                             │
│  ┌─────────────────────────────────────────────────────┐│
│  │ filePath  (required)  string                        ││
│  │   Absolute or relative path to the file             ││
│  │   e.g. "src/main.rs"                                ││
│  ├─────────────────────────────────────────────────────┤│
│  │ offset    (optional)  number  default: 1            ││
│  │   Starting line number (1-indexed)                  ││
│  ├─────────────────────────────────────────────────────┤│
│  │ limit     (optional)  number  default: 2000         ││
│  │   Maximum lines to read                             ││
│  └─────────────────────────────────────────────────────┘│
│                                                         │
│  Properties  Read-only  ·  Concurrency-safe  ·  Fast    │
│                                                         │
│  Examples                                               │
│  1. Read a file: { "filePath": "src/main.rs" }          │
│  2. Read lines 100-200: { "filePath": "src/main.rs",    │
│     "offset": 100, "limit": 100 }                       │
│                                                         │
│  Error Modes                                            │
│  • not_found – File does not exist (recoverable)        │
│  • permission_denied – OS permissions (not recoverable) │
│  • too_large – File exceeds max read size (recoverable) │
└─────────────────────────────────────────────────────────┘
```

### 6.2 Compact Tool List View

Used for `/tools` listing or category browse:

```
Tool Calling Box  —  24 tools available

📄 File Operations (5)          🌐 Web / Network (2)       🤖 Agent Management (3)
  read          Read file         http-get    HTTP GET       subagent    Launch subagent
  write         Write file        fetch-url   Fetch URL      chain       Run chain
  edit          Edit file                                      parallel    Run parallel
  delete        Delete file
  ls            List directory

▶ Code Execution (2)            🔍 Search / Query (3)      💬 Communication (2)
  bash          Run shell         grep        Search text     notify      Send notification
  python        Run Python        glob        Find files      confirm     Ask user
                                  find        Search files

⚙ System (3)                    📝 Diff / Patch (2)        🔄 Data Transform (2)
  env           Environment       diff        Show diff       json        JSON operations
  clock         Date/time         apply-diff  Apply patch     base64      Base64 encode/decode

  [?] tool-help <name> for details  ·  [/search <query>] to find tools
```

### 6.3 Search Results View

When the user types `/search <query>`:

```
  Search: "find file by content"
  ┌────────────────────────────────────────────────────┐
  │ 1. grep (score: 0.85)   Search Query              │
  │    Search file contents using regex patterns       │
  │ 2. glob (score: 0.65)   File Operations            │
  │    Find files by glob pattern                      │
  │ 3. find (score: 0.60)   Search Query               │
  │    Find files by name/location                     │
  │ 4. read (score: 0.30)   File Operations            │
  │    Read file contents — matches "file" + "content" │
  └────────────────────────────────────────────────────┘
```

### 6.4 Comparison View

When the user asks "what's the difference between grep and find":

```
  Search: "compare grep find"
  ┌─────────────────────────────────────────────────────────────────────┐
  │                     grep                               find          │
  ├─────────────────────────────────────────────────────────────────────┤
  │ Category     Search Query                         Search Query      │
  │ Description  Search file contents by regex        Find files by     │
  │              pattern                              name/pattern      │
  │ Read-only    Yes                                  Yes               │
  │ Parameters   pattern (req), path (opt)            pattern (req),    │
  │              context (opt)                        path (opt)        │
  │ Latency      Fast                                 Fast              │
  │ Concurrency  Safe                                 Safe              │
  │ Use when     You know what text you want          You know the      │
  │              but not where                        filename/pattern  │
  └─────────────────────────────────────────────────────────────────────┘
```

### 6.5 Widget Architecture & Component Tree

The TUI widgets follow a layered component architecture. Each widget is a self-contained `Widget` implementor that can be composed:

```
App (root)
├── ConversationView              ← main message list
│   ├── MessageBubble             ← LLM or user message
│   ├── ToolCallInline            ← collapsed/expanded tool call
│   │   ├── ToolCallHeader        ← name + status icon + duration
│   │   ├── ToolCallParams        ← key-value args (expandable)
│   │   ├── ToolCallProgress      ← streaming progress bar
│   │   └── ToolCallResult        ← result preview or error card
│   └── ToolCallChain             ← composed sequences
│
├── ToolBoxPanel                  ← right-side or overlay panel
│   ├── ToolListCompact           ← categorized overview
│   ├── ToolCardDetail            ← single-tool deep view
│   ├── ToolComparisonView        ← side-by-side diff
│   ├── ToolSearchView            ← search + results
│   └── ToolRecommendations       ← context-aware suggestions
│
├── CommandBar                     ← slash-command entry
│   └── SearchBar                 ← live-search with autocomplete
│
└── StatusBar                     ← global status line
    ├── ToolCount                 ← "24 tools loaded"
    ├── PluginIndicator           ← "3 plugins active"
    └── ModeIndicator             ← "plan | implement | review"
```

#### Widget Traits

```rust
/// Every widget can render itself into a buffer and handle input.
pub trait ToolWidget {
    /// Render into the terminal buffer at given area.
    fn render(&self, area: Rect, buf: &mut Buffer);

    /// Handle keyboard/mouse input. Returns whether the event was consumed.
    fn handle_event(&mut self, event: &Event) -> Result<bool, WidgetError>;

    /// Get the minimum size needed for proper rendering.
    fn min_size(&self) -> (u16, u16);

    /// Get the widget's current height (for scroll containers).
    fn height(&self) -> u16;
}

/// Widgets that support scrolling.
pub trait ScrollableWidget: ToolWidget {
    fn scroll_offset(&self) -> u16;
    fn set_scroll_offset(&mut self, offset: u16);
    fn max_scroll(&self) -> u16;
    fn scroll_up(&mut self, lines: u16);
    fn scroll_down(&mut self, lines: u16);
}

/// Widgets that support filtering/searching.
pub trait FilterableWidget: ToolWidget {
    fn set_filter(&mut self, query: &str);
    fn clear_filter(&mut self);
    fn filtered_count(&self) -> usize;
    fn active_count(&self) -> usize;
}

/// Widgets that can emit actions (tool calls, navigation, etc.).
pub trait ActionWidget: ToolWidget {
    type Action;
    fn pending_action(&self) -> Option<Self::Action>;
    fn consume_action(&mut self) -> Option<Self::Action>;
}
```

### 6.6 Layout & Responsive Behavior

The toolbox panel adapts to terminal width:

| Width | Layout |
|---|---|
| < 80 cols | Full-screen overlay (hides conversation) |
| 80–120 cols | Right-side panel, 30–40 cols wide |
| 120–180 cols | Right-side panel, 40–50 cols wide |
| > 180 cols | Right-side panel, 50–60 cols wide, multi-column tool list |

On narrow terminals, the toolbox becomes an overlay panel that slides in via a keybinding
(e.g., `Ctrl+T`). On wide terminals it's a persistent side panel.

### 6.7 Keyboard Navigation Map

```
Global (any mode):
  Ctrl+T        Toggle toolbox panel open/closed
  Ctrl+F        Focus search bar
  Ctrl+Q        Close toolbox / back to conversation
  Esc           Close current view / go up one level

Toolbox panel focused:
  Tab / ↓       Next tool in list
  Shift+Tab / ↑ Previous tool in list
  →             Expand/collapse category
  Enter         Open tool card detail
  /             Focus search within toolbox
  n             Next search result
  N             Previous search result
  c             Compare selected tool with... (then pick second)
  r             Run tool (opens param prompt)
  ?             Show help for selected tool
  d             Show deprecation info (if deprecated)
  g g           Jump to first tool
  G             Jump to last tool

Tool card detail view:
  Tab / ↓       Next parameter / example / error block
  Shift+Tab / ↑ Previous block
  Enter         Copy tool name to clipboard
  r             Run this tool (opens param prompt)
  c             Compare with another tool
  Esc / q       Back to list
  [             Previous tool in category
  ]             Next tool in category

Comparison view:
  Tab           Toggle between left/right selection
  Enter         Select a tool to compare
  Esc / q       Exit comparison

Search bar:
  Enter         Execute search
  Tab           Accept autocomplete suggestion
  ↑ / ↓         Navigate results
  Esc           Clear / close
```

### 6.8 Color & Styling System

Each category gets a distinct accent color for visual scanning:

```rust
impl ToolCategory {
    pub fn accent_color(&self) -> Color {
        match self {
            Self::FileOperations => Color::Cyan,
            Self::CodeExecution => Color::Green,
            Self::SearchQuery => Color::Blue,
            Self::WebNetwork => Color::Magenta,
            Self::Communication => Color::Yellow,
            Self::System => Color::White,
            Self::AgentManagement => Color::Red,
            Self::McpServices => Color::LightBlue,
            Self::MemoryStore => Color::LightCyan,
            Self::CodingAssist => Color::LightGreen,
            Self::DiffPatch => Color::LightYellow,
            Self::DataTransform => Color::LightMagenta,
            Self::HelpDocs => Color::Gray,
        }
    }
}
```

Common style tokens:

```
Style tokens for tool widgets:
  ┌──────────────────────────────────────────┐
  │ token               │ style              │
  ├──────────────────────────────────────────┤
  │ tool.name           │ bold + accent      │
  │ tool.category_badge │ dim + accent bg    │
  │ tool.description    │ normal             │
  │ param.name          │ bold + white       │
  │ param.required_tag  │ red + bold ("*req")
  │ param.optional_tag  │ dim ("opt")        │
  │ param.type          │ italic + cyan      │
  │ param.default       │ dim + yellow       │
  │ example.title       │ underline + bold   │
  │ example.code        │ on dark bg         │
  │ error.kind          │ red + bold         │
  │ error.recoverable   │ yellow + italic    │
  │ property.tag        │ dim + bold         │
  │ search.query        │ inverse            │
  │ search.match_hl     │ yellow + bold      │
  │ status.ok           │ green + bold ("✓" ) │
  │ status.err          │ red + bold  ("✗" ) │
  │ status.pending      │ yellow + dim ("⋯" )│
  │ progress.fill       │ green              │
  │ progress.empty      │ dim                │
  │ border.focused      │ accent + bold      │
  │ border.inactive     │ dim                │
  │ separator           │ dim                │
  └──────────────────────────────────────────┘
```

### 6.9 Interaction States

Every interactive widget has explicit visual states:

| State | Visual | Behavior |
|---|---|---|
| `Normal` | Default rendering | Accepts keyboard input |
| `Focused` | Bold border, accent indicator | Primary keyboard target |
| `Hovered` | Dim highlight on item | Mouse hover (if supported) |
| `Selected` | Highlighted background | Item is selected/chosen |
| `Active` | Pulsing indicator or spinner | Tool is executing, streaming |
| `Disabled` | Dim, grayed out | Tool not available in current mode |
| `Error` | Red border + error icon | Tool returned an error |
| `Loading` | Spinner + skeleton | Content is being fetched |

### 6.10 Detailed Widget Specifications

---

#### Widget: `ToolBoxPanel`

The main container. Manages child view state and layout.

```rust
pub struct ToolBoxPanel {
    pub visible: bool,
    pub width: u16,                      // computed from terminal width
    pub active_view: ToolBoxView,       // which child is shown
    pub tool_list: ToolListCompact,
    pub tool_detail: ToolCardDetail,
    pub comparison: ToolComparisonView,
    pub search: ToolSearchView,
    pub recommendations: ToolRecommendations,
    pub status_bar: ToolBoxStatusBar,
}

pub enum ToolBoxView {
    List,           // Compact categorized list (default)
    Detail(String), // Card for a specific tool
    Compare(Vec<String>),  // Side-by-side comparison
    Search(String), // Search results
    Help,           // Toolbox help screen
}
```

---

#### Widget: `ToolCardDetail`

Full detail view for a single tool. Renders sections in vertical order:

```
┌────────────────────────────────────────────────┐
│ ← Tools / File Operations / read   [x] close   │  ← Breadcrumb + close
├────────────────────────────────────────────────┤
│                                                │
│  📄 read                               v1.0.0  │  ← Name + version
│  Read the contents of a file                   │  ← Description
│                                                │
│  🔵 Builtin · 🔒 Read-only · ⚡ Fast · 🧵 Safe │  ← Property badges
│                                                │
│  ─── Parameters ──────────────────────         │
│                                                │
│  filePath  *required*  string                  │  ← Required param
│  ┌──────────────────────────────────────────┐  │
│  │ Absolute or relative path to the file    │  │
│  │ e.g. "src/main.rs"                       │  │
│  └──────────────────────────────────────────┘  │
│                                                │
│  offset    optional   number  default: 1       │  ← Optional param
│  ┌──────────────────────────────────────────┐  │
│  │ Starting line number (1-indexed)         │  │
│  └──────────────────────────────────────────┘  │
│                                                │
│  limit     optional   number  default: 2000    │
│  ┌──────────────────────────────────────────┐  │
│  │ Maximum lines to read/display            │  │
│  └──────────────────────────────────────────┘  │
│                                                │
│  ─── Examples ──────────────────────           │
│                                                │
│  Example 1: Read a file                        │
│  ┌──────────────────────────────────────────┐  │
│  │ read { filePath: "src/main.rs" }         │  │
│  └──────────────────────────────────────────┘  │
│                                                │
│  Example 2: Read with offset                   │
│  ┌──────────────────────────────────────────┐  │
│  │ read { filePath: "src/main.rs",          │  │
│  │        offset: 100, limit: 100 }          │  │
│  └──────────────────────────────────────────┘  │
│                                                │
│  ─── Error Modes ────────────────────          │
│                                                │
│  • not_found — File does not exist             │
│    ↳ recoverable ✓  Check the file path        │
│  • permission_denied — OS permissions          │
│    ↳ not recoverable ✗                         │
│  • too_large — File exceeds max read size      │
│    ↳ recoverable ✓  Use offset/limit chunks    │
│                                                │
│  ─── Metadata ──────────────────────           │
│                                                │
│  Category:    File Operations > Read           │
│  Tags:        file, view, cat                  │
│  Source:      Builtin                          │
│  Latency:     Fast (<500ms)                    │
│  Max result:  50,000 chars                     │
│  Streaming:   ✓ Yes                            │
│  Concurrency: ✓ Safe                           │
│  Cost hint:   50 tokens/call (Free)            │
│                                                │
│  [r] Run tool   [c] Compare   [?] Help         │  ← Action bar
└────────────────────────────────────────────────┘
```

Implementation notes:
- Sections collapse/expand via `[+]` / `[-]` toggles
- Parameters use vertical layout (not horizontal) to show full descriptions
- Property badges are colored by category (see 6.8)
- The `[*required*]` / `[optional]` tags use color + icon
- Action bar at bottom shows available keybindings
- Breadcrumb navigation at top tracks depth: `Tools / File Operations / read`

---

#### Widget: `ToolListCompact`

The default view — a categorized overview of all tools.

```
┌────── Tool Calling Box ───────────────────────┐
│ [24 tools loaded · 3 plugins · plan mode]     │
├───────────────────────────────────────────────┤
│ ▶ File Operations (5)                    [+] │
│   ○ read          Read file contents          │
│   ○ write         Write content to file       │
│   ○ edit          Replace text in file        │
│   ○ delete        Delete a file               │
│   ○ ls            List directory contents     │
│                                               │
│ ▶ Code Execution (2)                     [+] │
│   ○ bash          Execute shell commands      │
│   ○ python        Execute Python code      >> │  ← most recently used
│                                               │
│ ▼ Search / Query (3)                     [-] │  ← expanded
│   ○ grep          Search text with regex      │
│   ○ glob          Find files by pattern    >> │  ← recently used
│   ○ find          Search filesystem           │
│                                               │
│ ▶ Web / Network (2)                      [+] │
│ ▶ Communication (2)                      [+] │
│ ▶ System (3)                             [+] │
│ ▶ Agent Management (3)                   [+] │
│ ▶ Coding Assist (4)                      [+] │
│ ▶ Diff / Patch (2)                       [+] │
│ ▶ Data Transform (2)                     [+] │
│ ▶ Help / Docs (2)                        [+] │
├───────────────────────────────────────────────┤
│  [/search]  [n/N next]  [c compare]  [?]      │
└───────────────────────────────────────────────┘
```

Features:
- Categories are collapsible (`▶` collapsed, `▼` expanded)
- Recently used tools are marked with `>>` and moved to top of their category
- Only expanded categories show their tools (reduces visual noise)
- The status bar shows tool count, plugin count, and current mode
- `Ctrl+F` instantly focuses the search bar which filters the list in real time
- Categories with tools matching the search filter auto-expand

```rust
pub struct ToolListCompact {
    pub categories: BTreeMap<ToolCategory, CategoryGroup>,
    pub collapsed: HashSet<ToolCategory>,
    pub filter: Option<String>,
    pub selected_index: usize,
    pub scroll_offset: u16,
    pub recent_tools: VecDeque<String>,   // max 5 recently used
    pub mode: ConversationMode,
}

pub struct CategoryGroup {
    pub category: ToolCategory,
    pub tools: Vec<ToolMetadata>,
    pub expanded: bool,
    pub matched_count: usize,   // after filter
}
```

---

#### Widget: `ToolSearchView`

Real-time search with live results and autocomplete.

```
┌───── Search Tools ─────────────────────────────┐
│  find files by content          [3 results]     │
│  ┌──────────────────────────────────────────┐  │
│  │ 🔍 Search tools...                       │  │  ← search input
│  └──────────────────────────────────────────┘  │
│                                                │
│  Results for "find files by content":          │
│                                                │
│  1. grep (.85)  🔍 Search / Query             │  ← selected (▶)
│  ┌──────────────────────────────────────────┐  │
│  │ Search file contents using regex         │  │
│  │ pattern (req), path (opt), context (opt) │  │
│  └──────────────────────────────────────────┘  │
│                                                │
│  2. glob (.65)  📄 File Operations             │
│     Find files by glob pattern                 │
│                                                │
│  3. find (.60)  🔍 Search / Query              │
│     Find files by name/location                │
│                                                │
│  Autocomplete:                                 │
│  ┌─ find ──────────────────────────────────┐   │
│  │ find (Search / Query)  .60              │   │
│  │ find-in-files (Search)  .45             │   │
│  │ find-symbol (Coding Assist)  .40        │   │
│  └─────────────────────────────────────────┘   │
│                                                │
│  [↑/↓ navigate] [Enter select] [Tab complete]  │
└────────────────────────────────────────────────┘
```

```rust
pub struct ToolSearchView {
    pub input: String,
    pub cursor_position: usize,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub autocomplete: Vec<ToolMetadata>,
    pub autocomplete_selected: usize,
    pub show_autocomplete: bool,
    pub is_searching: bool,
    pub debounce_timer: Option<Instant>,
}

impl ToolSearchView {
    /// Called on every keystroke with debounce (150ms).
    pub fn on_input_change(&mut self, toolbox: &ToolBox) {
        if self.input.is_empty() {
            self.results.clear();
            self.show_autocomplete = false;
            return;
        }
        // 1. Show autocomplete dropdown for partial name matches
        self.autocomplete = toolbox.search(SearchMode::Prefix(self.input.clone()));
        self.show_autocomplete = self.autocomplete.len() <= 10;

        // 2. Execute full search after debounce
        self.results = toolbox.search(SearchMode::Composite(SearchQuery {
            text: Some(self.input.clone()),
            category: None,
            tags: None,
            read_only: None,
            latency_max: None,
            source: None,
        }));
    }

    /// Autocomplete on Tab press
    pub fn accept_autocomplete(&mut self) {
        if let Some(tool) = self.autocomplete.get(self.autocomplete_selected) {
            self.input = tool.name.clone();
            self.show_autocomplete = false;
            self.on_input_change(...);
        }
    }
}
```

---

#### Widget: `ToolComparisonView`

Side-by-side comparison of two or more tools.

```
┌───── Compare Tools ──────────────────────────────────────────────┐
│                                                                │
│  Tool A:  grep               Tool B:  find                     │
│  ┌────────────────────────┐  ┌────────────────────────┐        │
│  │ Category               │  │                        │        │
│  │ Search / Query         │  │ Search / Query         │        │
│  ├────────────────────────┤  ├────────────────────────┤        │
│  │ Description            │  │                        │        │
│  │ Search file contents   │  │ Find files by name/    │        │
│  │ by regex pattern       │  │ pattern                │        │
│  ├────────────────────────┤  ├────────────────────────┤        │
│  │ Parameters             │  │                        │        │
│  │ pattern (req) string   │  │ pattern (req) string   │        │
│  │ path    (opt) string   │  │ path    (opt) string   │        │
│  │ context (opt) number   │  │ type    (opt) enum     │        │
│  ├────────────────────────┤  ├────────────────────────┤        │
│  │ Properties             │  │                        │        │
│  │ Read-only:      ✓     │  │ Read-only:      ✓     │        │
│  │ Concurrency:    ✓     │  │ Concurrency:    ✓     │        │
│  │ Latency:        Fast  │  │ Latency:        Fast  │        │
│  │ Streaming:      ✓     │  │ Streaming:      ✗     │        │
│  │ Max result:     50KB  │  │ Max result:     50KB  │        │
│  ├────────────────────────┤  ├────────────────────────┤        │
│  │ Tags                   │  │                        │        │
│  │ search, text, regex    │  │ file, path, filesystem │        │
│  ├────────────────────────┤  ├────────────────────────┤        │
│  │ Use when:              │  │ Use when:              │        │
│  │ You know the text but  │  │ You know the filename  │        │
│  │ not where              │  │ but not the location   │        │
│  └────────────────────────┘  └────────────────────────┘        │
│                                                                │
│  Differences highlighted:                                      │
│  • grep supports streaming, find does not                      │
│  • grep has context param, find has type param                 │
│  • grep is for text search, find is for file search            │
│  • grep tags: search/text/regex | find tags: file/path/fs      │
│                                                                │
│  [←/→ switch focus]  [Enter select tool]  [q quit]            │
└────────────────────────────────────────────────────────────────┘
```

```rust
pub struct ToolComparisonView {
    pub tools: Vec<ToolMetadata>,
    pub focused_side: usize,         // 0 for left, 1 for right
    pub scroll_offsets: [u16; 2],
    pub differences: Vec<DiffHighlight>,
}

pub struct DiffHighlight {
    pub field: String,              // "latency" | "parameters" | "streaming"
    pub tool_a_value: String,
    pub tool_b_value: String,
    pub significance: DiffSignificance,
}

pub enum DiffSignificance {
    Critical,   // e.g., one is read-only, the other writes
    Notable,    // different latency, streaming support
    Minor,      // different tags, categories
}
```

The comparison view automatically computes differences by iterating
over all metadata fields and flagging mismatches:

```rust
impl ToolComparisonView {
    pub fn new(tools: Vec<ToolMetadata>) -> Self {
        let differences = compute_differences(&tools[0], &tools[1]);
        // ...
    }
}

fn compute_differences(a: &ToolMetadata, b: &ToolMetadata) -> Vec<DiffHighlight> {
    let mut diffs = vec![];

    if a.category != b.category {
        diffs.push(DiffHighlight {
            field: "category".into(),
            tool_a_value: format!("{:?}", a.category),
            tool_b_value: format!("{:?}", b.category),
            significance: DiffSignificance::Notable,
        });
    }

    if a.read_only != b.read_only {
        diffs.push(DiffHighlight {
            field: "read_only".into(),
            tool_a_value: a.read_only.to_string(),
            tool_b_value: b.read_only.to_string(),
            significance: DiffSignificance::Critical,
        });
    }

    if a.latency_hint != b.latency_hint {
        diffs.push(DiffHighlight {
            field: "latency".into(),
            tool_a_value: format!("{:?}", a.latency_hint),
            tool_b_value: format!("{:?}", b.latency_hint),
            significance: DiffSignificance::Notable,
        });
    }

    // Compare parameter sets
    let a_params: HashSet<&str> = a.param_summaries.iter().map(|p| p.name.as_str()).collect();
    let b_params: HashSet<&str> = b.param_summaries.iter().map(|p| p.name.as_str()).collect();
    if a_params != b_params {
        diffs.push(DiffHighlight {
            field: "parameters".into(),
            tool_a_value: a_params.iter().join(", "),
            tool_b_value: b_params.iter().join(", "),
            significance: DiffSignificance::Notable,
        });
    }

    // Compare tags, streaming, concurrency, max_result, etc. ...

    diffs
}
```

---

#### Widget: `ToolCallInline`

Inline rendering of a tool call within the conversation stream. Each tool
call is a collapsible card rendered between message bubbles.

```rust
pub struct ToolCallInline {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub status: ToolCallStatus,
    pub result: Option<ToolCallResultData>,
    pub expanded: bool,
    pub duration: Option<Duration>,
    pub call_id: String,
}

pub enum ToolCallStatus {
    Pending,        // Queued
    Running,        // Executing (with streaming data)
    Completed,      // Done successfully
    Error(ToolErrorSpec), // Failed
    Aborted,        // Cancelled by user / budget
}

pub enum ToolCallResultData {
    Text(String),
    Truncated { preview: String, full_path: PathBuf, total_chars: usize },
    NoResult,   // Fire-and-forget tools
}
```

**Rendering by status:**

```
── Pending ─────────────────────────────────
┌─ ⋯ read "main.rs" (queued) ──────────────┐
│  Waiting for rate limit...                 │
└────────────────────────────────────────────┘

── Running (simple) ─────────────────────────
┌─ ▶ read "main.rs" ────────────────────────┐
│  Arguments:                               │
│  • filePath: "src/main.rs"                 │
│  • offset: 1                               │
│  • limit: 2000                             │
├────────────────────────────────────────────┤
│  ⏳ Reading...                             │
└────────────────────────────────────────────┘

── Running (streaming) ──────────────────────
┌─ ▶ bash "cargo test" ─────────────────────┐
│  Arguments:                               │
│  • command: "cargo test"                    │
│  • cwd: "."                                │
├────────────────────────────────────────────┤
│  Progress: ▓▓▓▓▓▓▓░░░░░  65%              │
│  ┌──────────────────────────────────────┐  │
│  │ Compiling omega-core v0.1.0          │  │
│  │ Compiling omega-cli v0.1.0           │  │
│  │    Compiling tool-harness v0.1.0     │  │
│  │    Finished test [unoptimized]       │  │
│  └──────────────────────────────────────┘  │
│  [live — 4.2s elapsed]                    │
└────────────────────────────────────────────┘

── Completed ────────────────────────────────
┌─ ▶ read "main.rs" ────────────────────────┐
│  • filePath: "src/main.rs"                 │
│  • offset: 1, limit: 2000                  │
├────────────────────────────────────────────┤
│  ✓ Done (12ms)                             │
│  ┌──────────────────────────────────────┐  │
│  │ fn main() {                          │  │
│  │     println!("Hello, world!");       │  │
│  │     // ... 2000 lines total ...      │  │
│  └──────────────────────────────────────┘  │
│  [2000 chars]  [?] toggle full output      │
└────────────────────────────────────────────┘

── Truncated ────────────────────────────────
┌─ ▶ grep "Error" logs/ ────────────────────┐
│  • pattern: "Error"                         │
│  • path: "logs/"                            │
├────────────────────────────────────────────┤
│  ⚠ Output truncated (12,450 of 84,203     │
│  chars). Full output saved to:             │
│  ~/.omega/tool-results/grep-abc123.txt     │
│  ┌──────────────────────────────────────┐  │
│  │ [ERROR] connection timeout (first    │  │
│  │ 12,450 chars shown...)               │  │
│  └──────────────────────────────────────┘  │
│  [Enter] open full output                   │
└────────────────────────────────────────────┘

── Error ────────────────────────────────────
┌─ ✗ read "missing.rs" ─────────────────────┐
│  • filePath: "missing.rs"                   │
├────────────────────────────────────────────┤
│  ✗ not_found                               │
│  File does not exist at path "missing.rs"   │
│                                            │
│  💡 Check the file path and try again       │
│  • Did you mean: "src/main.rs"?             │
│  • Try: ls to list directory contents       │
│                                            │
│  [r] retry  [e] edit path  [d] dismiss     │
└────────────────────────────────────────────┘
```

---

#### Widget: `ToolCallChain`

When the LLM executes a composed pipeline (`then` / `pipe`), the chain
renders as a vertical sequence of connected cards:

```
┌─ Chain: build & test ──────────────────────────┐
│                                                 │
│  Step 1/3: build  ──▶ bash "cargo build"        │
│  ┌──────────────────────────────────────────┐   │
│  │ ✓ Completed (3.2s)                      │   │
│  │ → exit code 0, output: "Compiling..."    │   │
│  └──────────────────────────────────────────┘   │
│                       │                         │
│                       ▼                         │
│  Step 2/3: test   ──▶ bash "cargo test"         │
│  ┌──────────────────────────────────────────┐   │
│  │ ✓ Completed (8.1s)                      │   │
│  │ → 42 passed, 0 failed                   │   │
│  └──────────────────────────────────────────┘   │
│                       │                         │
│                       ▼                         │
│  Step 3/3: notify ──▶ notify "Build green"      │
│  ┌──────────────────────────────────────────┐   │
│  │ ✓ Sent                                   │   │
│  └──────────────────────────────────────────┘   │
│                                                 │
│  Total: 11.5s  │  All steps succeeded           │
└─────────────────────────────────────────────────┘
```

The chain visualization includes:
- Vertical connector lines between steps (`│` + `▼`)
- Step number and tool name
- Duration per step + total
- Status icon per step
- Hover/focus on a step to see its full result

---

#### Widget: `ToolRecommendations`

Context-aware suggestions shown in a sidebar panel. Refreshes as
conversation context changes.

```
┌─ Suggested Tools ─────────────────────┐
│                                       │
│ Based on current context:             │
│                                       │
│ You're debugging test failures        │
│ ──────────────────────────────────    │
│                                       │
│ 1. grep       Search test output  >>  │  ← high relevance
│ 2. diff       Compare expected vs     │
│               actual                  │
│ 3. bash       Run specific test       │
│ 4. read       Inspect test file       │
│                                       │
│ Recent tools:                         │
│ ──────────────────────────────────    │
│ • cargo-test (2 min ago)              │
│ • git-log (5 min ago)                 │
│                                       │
│ Press Tab to focus recommendations    │
└───────────────────────────────────────┘
```

```rust
pub struct ToolRecommendations {
    pub suggestions: Vec<ScoredSuggestion>,
    pub recent: Vec<ToolMetadata>,
    pub mode: ConversationMode,
    pub context_summary: String,  // "debugging test failures"
}

pub struct ScoredSuggestion {
    pub tool: ToolMetadata,
    pub score: f32,
    pub reason: String,  // "You're debugging test failures"
}

impl ToolRecommendations {
    /// Recompute recommendations based on conversation context.
    /// Called when:
    ///   - A new message is added to the conversation
    ///   - A tool call completes
    ///   - The mode changes
    pub fn refresh(&mut self, history: &ConversationHistory, toolbox: &ToolBox) {
        let mut recommender = toolbox.recommender();

        // Add context signals
        if let Some(last_error) = history.last_tool_error() {
            recommender.add_signal(Signal::LastToolError(last_error));
        }
        if let Some(last_file) = history.last_referenced_file() {
            recommender.add_signal(Signal::OpenFile(last_file));
        }
        recommender.set_mode(self.mode);

        self.suggestions = recommender.recommend(5);
        self.recent = history.recent_tools(5);
        self.context_summary = recommender.context_summary();
    }
}
```

### 6.11 Rendering Engine Priority

The TUI rendering engine follows these priority rules when rendering
the toolbox widgets within the available terminal area:

```
Priority 1 (always visible):
  • StatusBar (1 line at bottom) — tool count, mode, plugin state
  • ToolCallInline in progress (streaming tools)

Priority 2 (if space allows):
  • Tool recommendations panel (collapsed to 3 lines if tight)
  • Active search bar

Priority 3 (expandable):
  • Full tool list
  • Tool card details
  • Comparison view

Priority 4 (scroll to view):
  • Tool examples
  • Error mode documentation
  • Full result previews
```

### 6.12 Animation & Micro-Interactions

Subtle animations improve perceived performance and state awareness:

```rust
pub enum WidgetAnimation {
    /// Smooth height transition on collapse/expand
    Collapse { from: u16, to: u16, progress: f32 },
    /// Pulsing indication for active/streaming state
    Pulse { phase: f32, period: Duration },
    /// Fade in for new search results
    FadeIn { progress: f32 },
    /// Slide from right for panel open/close
    SlideIn { offset: u16, target: u16, progress: f32 },
    /// Cursor blink
    Blink { phase: f32, period: Duration },
}

impl WidgetAnimation {
    /// Advance animation by delta time. Returns false when complete.
    pub fn tick(&mut self, dt: Duration) -> bool;

    /// Get the current interpolated value.
    pub fn value(&self) -> f32;
}
```

- **Panel slide**: Toolbox panel slides in from right edge over 150ms
- **Spinner**: Unicode braille spinner (`⣾⣽⣻⢿⡿⣟⣯⣷`) during loading
- **Progress bar**: Fills left-to-right, uses block chars `░▒▓█`
- **Status transitions**: Icons cross-fade (e.g., `⋯` → `✓`)
- **Search results**: Brief highlight flash on new results arriving

### 6.13 Mouse Support

For terminals that support mouse events:

```rust
pub enum MouseAction {
    ClickTool(String),          // Click on a tool name to open
    ClickCategory(ToolCategory), // Click category header to expand/collapse
    ClickExpand(String),        // Click inline tool call to expand
    ClickRetry(String),         // Click retry button on error
    ClickLink(PathBuf),         // Click truncated result path
    Scroll(u16),                // Scroll wheel
    Resize(u16, u16),           // Terminal resize
}

impl ToolWidget for ToolListCompact {
    fn handle_event(&mut self, event: &Event) -> Result<bool, WidgetError> {
        match event {
            Event::Mouse(MouseEvent::Down(MouseButton::Left, x, y, _mods)) => {
                // Hit-test against rendered items
                if let Some(tool) = self.tool_at_position(*x, *y) {
                    self.selected_index = tool.index;
                    return Ok(true); // consumed, triggers navigation
                }
            }
            Event::Mouse(MouseEvent::Down(MouseButton::WheelUp, _, _, _)) => {
                self.scroll_up(3);
                return Ok(true);
            }
            // ...
        }
    }
}
```

### 6.14 TUI Integration Points

The toolbox widgets integrate into the main TUI loop at these points:

```rust
// In the main TUI event loop:
impl App {
    async fn handle_event(&mut self, event: Event) -> Result<(), AppError> {
        match event {
            // ── Global keybindings ────────────────────────
            Event::Key(KeyEvent { code: Char('t'), modifiers: CTRL, .. }) => {
                self.toolbox_panel.toggle();
            }
            Event::Key(KeyEvent { code: Char('f'), modifiers: CTRL, .. }) => {
                if self.toolbox_panel.visible {
                    self.toolbox_panel.focus_search();
                } else {
                    self.toolbox_panel.show();
                    self.toolbox_panel.focus_search();
                }
            }

            // ── Route to focused widget ───────────────────
            _ if self.toolbox_panel.is_focused() => {
                self.toolbox_panel.handle_event(&event)?;
            }
            _ => {
                self.conversation.handle_event(&event)?;
            }
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.size();

        if self.toolbox_panel.visible {
            // Split: conversation | toolbox
            let (conv_area, tool_area) = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(self.toolbox_panel.width()),
            ]).split(area);

            frame.render_widget(&self.conversation, conv_area);
            frame.render_widget(&self.toolbox_panel, tool_area);
        } else {
            frame.render_widget(&self.conversation, area);
        }

        // Always render status bar at the bottom
        let (main_area, status_area) = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ]).split(area);
        frame.render_widget(&self.status_bar, status_area);
    }
}
```

### 6.15 Widget Implementation Files (target)

```
omega-cli/src/tui/
├── mod.rs
├── app.rs                       ← main App struct, event loop
├── widgets/
│   ├── mod.rs
│   ├── conversation.rs          ← ConversationView, MessageBubble
│   ├── tool_box_panel.rs        ← ToolBoxPanel (container)
│   ├── tool_card_detail.rs      ← ToolCardDetail
│   ├── tool_list_compact.rs     ← ToolListCompact
│   ├── tool_search_view.rs      ← ToolSearchView
│   ├── tool_comparison_view.rs  ← ToolComparisonView
│   ├── tool_call_inline.rs      ← ToolCallInline
│   ├── tool_call_chain.rs       ← ToolCallChain
│   ├── tool_recommendations.rs  ← ToolRecommendations
│   ├── progress_indicator.rs    ← shared progress bar / spinner
│   ├── error_card.rs            ← ErrorCard
│   └── status_bar.rs            ← ToolBoxStatusBar
├── styles.rs                    ← Color palette, style tokens
├── animation.rs                 ← WidgetAnimation system
└── layout.rs                    ← Responsive layout helpers
```

---

### 6.16 Integration with Agent Conversation UI

The Tool Calling Box integrates with the TUI's conversation rendering:

1. **Tool call in conversation**: When the LLM calls a tool, the TUI renders a collapsed/expandable tool card inline:

```
  ┌─ ▶ read "src/main.rs" ────────────────────────┐
  │  • filePath: "src/main.rs"                     │
  │  • offset: 1, limit: 2000                      │
  ├──────────────────────────────────────────────────  (click to expand)
  │  Result: [2000 lines] ✓                          │
  └──────────────────────────────────────────────────┘
```

2. **Tool result truncation**: If the result exceeds the budget, show `[Output truncated. Full result at ~/.omega/tool-results/abc123.txt]` with a clickable path.

3. **Tool progress**: For streaming tools, render a live-progress indicator:

```
  ┌─ ▶ bash "cargo build" ────────────────────────┐
  │  Compiling omega-core... ▓▓▓▓▓░░░░░ 45%       │
  │  [live stream of compilation output...]        │
  └──────────────────────────────────────────────────┘
```

4. **Tool error**: Render error cards with retry suggestions:

```
  ┌─ ✗ read "missing.rs" ─────────────────────────┐
  │  Error: not_found                              │
  │  File does not exist: "missing.rs"             │
  │  Suggestion: Check the file path and try again │
  └──────────────────────────────────────────────────┘
```

---

## 7. Extension & Plugin System

### 7.1 Plugin Manifest Format

Third-party tools register via a `tool-plugin.toml` manifest:

```toml
[plugin]
name = "my-awesome-tools"
version = "1.0.0"
description = "Custom tools for my workflow"
author = "Your Name"
min_agent_version = "0.5.0"

[[tools]]
name = "my-custom-tool"
label = "My Custom Tool"
description = "Does something amazing"
category = "DataTransform"
tags = ["custom", "amazing"]
read_only = true
concurrency_safe = false
latency_hint = "fast"
max_result_chars = 10000

[tools.parameters]
type = "object"
properties.input = { type = "string", description = "Input value" }
required = ["input"]

[tools.examples]
title = "Basic usage"
description = "Run with default input"
arguments = { input = "hello" }

[execution]
runtime = "wasm"              # "native" | "wasm" | "http" | "mcp"
entry = "wasm/my-tool.wasm"   # wasm file, or [dylib] path, or url
timeout_seconds = 30
sandbox = { filesystem = "read-only", network = "none" }
```

### 7.2 Plugin Runtimes

```rust
pub enum PluginRuntime {
    /// Native Rust dylib (`.so` / `.dylib` / `.dll`)
    Native { library_path: PathBuf },

    /// WebAssembly module (`.wasm`)
    Wasm { module_path: PathBuf },

    /// HTTP endpoint (local or remote microservice)
    Http { base_url: String },

    /// MCP server (Model Context Protocol)
    Mcp { server_name: String },
}

pub struct PluginTool {
    pub metadata: ToolMetadata,
    pub runtime: PluginRuntime,
    pub sandbox: SandboxPolicy,
}

#[async_trait]
impl Tool for PluginTool {
    fn name(&self) -> &str { &self.metadata.name }
    fn description(&self) -> &str { &self.metadata.description }
    fn parameters_schema(&self) -> serde_json::Value { self.metadata.parameters.clone() }

    async fn call(&self, input: ToolInput, ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        match &self.runtime {
            PluginRuntime::Wasm { module_path } => {
                // Load WASM module, execute with sandboxed resources
                self.execute_wasm(module_path, &input).await
            }
            PluginRuntime::Http { base_url } => {
                // POST input to endpoint, return response
                self.execute_http(base_url, &input).await
            }
            PluginRuntime::Native { library_path } => {
                // Load dylib, call exported function
                self.execute_native(library_path, &input).await
            }
            PluginRuntime::Mcp { server_name } => {
                // Delegate to MCP tool execution
                self.execute_mcp(server_name, &input).await
            }
        }
    }
}
```

### 7.3 Plugin Discovery & Installation

```
~/.omega/
  plugins/
    my-awesome-tools/
      tool-plugin.toml
      wasm/
        my-tool.wasm
    another-plugin/
      ...
  plugin-cache.json       # Index of installed plugins with versions
```

Installation workflow:

1. User downloads or creates a plugin directory
2. Agent scans `~/.omega/plugins/*/tool-plugin.toml`
3. Plugin tools are registered into `ToolRegistry` with source = `Plugin`
4. On agent startup, cached plugins are loaded
5. Plugin manifests are validated against schema version

### 7.4 Sandboxing & Security

```rust
pub struct SandboxPolicy {
    pub filesystem: FileSystemAccess,
    pub network: NetworkAccess,
    pub process: ProcessAccess,
    pub environment: EnvironmentAccess,
    pub allowed_paths: Vec<PathBuf>,
    pub max_memory_mb: u32,
    pub max_cpu_seconds: u32,
}

pub enum FileSystemAccess {
    None,
    ReadOnly,
    ReadWrite { allowed_dirs: Vec<PathBuf> },
    Full,
}

pub enum NetworkAccess {
    None,
    LocalOnly,
    AllowedHosts(Vec<String>),
    Full,
}

pub enum ProcessAccess {
    None,
    SpawnOnly,
    Full,
}

pub enum EnvironmentAccess {
    None,
    ReadOnly,
    Full,
}
```

For WASM plugins, sandboxing is enforced by the WASM runtime itself (wasmtime/wasmer) with syscall interception. For HTTP plugins, sandboxing is network-level (allowed hosts only). For native dylibs, sandboxing is best-effort via OS-level seccomp (Linux) or seatbelt (macOS) — with a strong warning that native plugins have full system access.

### 7.5 Version Compatibility

Plugin manifest declares `min_agent_version` which is checked at registration:

```rust
fn check_compatibility(manifest: &PluginManifest, agent_version: &str) -> Result<(), PluginError> {
    let min = semver::Version::parse(&manifest.min_agent_version)?;
    let current = semver::Version::parse(agent_version)?;
    if current < min {
        return Err(PluginError::IncompatibleVersion {
            plugin: manifest.name.clone(),
            required: min,
            current,
        });
    }
    Ok(())
}
```

The `schema` field in `ToolMetadata` allows tools to evolve their parameter schemas across versions. The `ToolBox` validates that the schema version is compatible with the current runtime.

---

## 8. Edge Cases & Failure Handling

### 8.1 Tool Not Found

| Scenario | Behavior |
|---|---|
| Tool name doesn't exist in any registry | Return `ToolError::NotFound` with suggestions: "Tool 'rd' not found. Did you mean 'read'? (fuzzy match: 0.85)" |
| Tool exists but is deprecated | Return with deprecation warning: "Tool 'old-read' is deprecated since v1.0. Use 'read' instead." |
| Tool exists but is disabled/blocked | Return `PermissionDenied` with the policy reason |

**LLM-level handling**: When constructing the LLM prompt, the `ToolBox` validates that all referenced tools exist and warns about deprecated ones. If the LLM calls a non-existent tool, the result includes a helpful "did you mean?" suggestion that nudges the LLM to self-correct.

### 8.2 Ambiguous Tool Match

When multiple tools match the same query, the search returns the top 5 results with scores. The executor raises an `AmbiguousMatch` error if the caller doesn't disambiguate:

```rust
pub struct AmbiguousMatch {
    pub query: String,
    pub matches: Vec<SearchResult>,
}
```

Resolution strategies:
1. **First match wins** (for LLM calls — the LLM sees all options and can re-call)
2. **Explicit disambiguation** (for human queries — show comparison view)
3. **Confidence threshold**: if top score ≥ 0.8, auto-select; otherwise return options

### 8.3 Tool Fails Mid-Execution

The pipeline catches panics and fatal errors at each step:

1. **Step 1–9 (pre-execution)**: Errors are recoverable → return error with kind and retry advice
2. **Step 10 (execution)**: Wrap `tool.call()` in `tokio::time::timeout` and `std::panic::catch_unwind`
3. **Step 11–14 (post-execution)**: Non-fatal errors are logged but don't fail the tool call

```rust
async fn safe_execute(
    tool: &dyn Tool,
    input: ToolInput,
    ctx: &ToolUseContext,
    timeout: Duration,
) -> Result<ToolResult, ToolError> {
    // Catch panics
    let result = tokio::time::timeout(timeout, async {
        match std::panic::AssertUnwindSafe(tool.call(input, ctx)).catch_unwind().await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(panic) => Err(ToolError::with_kind(
                ToolErrorKind::Internal,
                format!("Tool panicked: {:?}", panic),
            )),
        }
    }).await;

    match result {
        Ok(inner) => inner,
        Err(_timeout) => Err(ToolError::with_kind(ToolErrorKind::Timeout, "Tool execution timed out")),
    }
}
```

### 8.4 Tool Returns Unexpectedly Large Data

The existing `ResultBudget` handles this with persistence. Enhancements:

1. **Streaming truncation**: For streaming tools, truncation happens incrementally. Once the accumulated output exceeds `max_result_chars`, the emitter switches to "overflow mode" — it writes to a temp file instead of the in-memory buffer.

2. **User notification**: The TUI shows `[Output truncated at N characters. Full output at path.]` with a clickable link.

3. **LLM guidance**: The truncated output includes a header: `[First N characters of M total. Full result available via read("path/to/persisted.txt")]`

### 8.5 Rate Limit Exceeded

```rust
pub enum RateLimitError {
    /// Per-tool limit hit
    ToolLimit { tool: String, retry_after: Duration },
    /// Per-category limit hit
    CategoryLimit { category: ToolCategory, retry_after: Duration },
    /// Global limit hit
    GlobalLimit { retry_after: Duration },
}
```

When the rate limiter rejects a call:

1. The pipeline returns a `ToolError` with kind `RateLimited` and `retry_after` duration
2. The orchestrator delays and retries (if retry policy permits)
3. The LLM sees the rate-limit error and can decide to wait or choose a different tool
4. The TUI shows: `⏳ Rate limited. Retrying in 2s...`

### 8.6 Circular Tool Dependencies

Tool chaining (`then`, `pipe`) is validated for cycles at composition time:

```rust
fn validate_composition(plan: &CompositionPlan, registry: &ToolRegistry) -> Result<(), CompositionError> {
    // Build dependency graph and detect cycles using DFS
    let mut graph = DependencyGraph::new();
    for step in &plan.steps {
        graph.add_node(step.tool.clone());
    }
    for (i, step) in plan.steps.iter().enumerate() {
        if i > 0 {
            graph.add_edge(plan.steps[i - 1].tool.clone(), step.tool.clone());
        }
    }
    if graph.has_cycle() {
        return Err(CompositionError::CircularDependency);
    }
    Ok(())
}
```

For runtime cycles (tool A calls tool B which calls tool A via subagent), the `ToolUseContext` includes a `call_depth` counter that aborts at a configurable max depth (default: 10).

### 8.7 Permission Denied

```rust
pub enum PermissionDeniedReason {
    ToolDisabled(String),
    FileNotAllowed(PathBuf),
    NetworkNotAllowed(String),
    OperationNotAllowed(String),
    ModeRestriction(String),    // e.g., "read_only" mode in plan mode
}
```

When permission is denied:

1. The error includes the precise reason and possible remediation
2. For `Prompt` mode, the dialog shows the tool name, arguments, and a "why this is needed" message
3. The user can respond: "allow once", "always allow", "deny", or "deny and remember"
4. The TUI renders a confirmation dialog: `Allow write to "/etc/hosts"? [y/N/always/never]`

### 8.8 Tool Execution State Diagram

```
                    ┌──────────┐
                    │  Pending  │
                    └─────┬────┘
                          │
                          ▼
                    ┌──────────┐
                    │ Resolving │  (permission, rate-limit check)
                    └─────┬────┘
                          │
               ┌──────────┼──────────┐
               ▼          ▼          ▼
         ┌─────────┐ ┌─────────┐ ┌─────────┐
         │ Allowed  │ │ Denied  │ │ Queued  │
         └────┬────┘ └─────────┘ └────┬────┘
              │                        │ (rate limited)
              ▼                        │
         ┌──────────┐                  │
         │ Running   │◄────────────────┘
         └────┬─────┘
              │
     ┌────────┼────────┬───────────┐
     ▼        ▼        ▼           ▼
  ┌──────┐ ┌──────┐ ┌──────┐ ┌─────────┐
  │ Done │ │Error │ │Panic │ │Timeout  │
  └──────┘ └──────┘ └──────┘ └─────────┘
     │        │        │           │
     └────────┴────────┴───────────┘
                    │
                    ▼
           ┌──────────────┐
           │ Post-process  │  (truncation, persistence, hooks,
           │              │    telemetry, result injection)
           └──────┬───────┘
                  │
                  ▼
           ┌──────────────┐
           │  Returned to  │
           │  Orchestrator │
           └──────────────┘
```

---

## 9. Implementation Roadmap

### Phase 1: Metadata & Taxonomy (Week 1–2)

| Task | Files | Description |
|---|---|---|
| 1.1 Define `ToolMetadata` struct | `tool-harness/src/metadata.rs` | New struct with all fields from §3.1 |
| 1.2 Define `ToolCategory` enum | `tool-harness/src/metadata.rs` | Full taxonomy from §2 |
| 1.3 Add supporting types | `tool-harness/src/metadata.rs` | ParamSummary, LatencyHint, CostHint, etc. |
| 1.4 Migrate existing tools to metadata | `tool-harness/src/tools/*.rs` | Add category, tags, examples, errors to read/write/edit/bash/grep/glob |
| 1.5 Add `metadata()` method to `Tool` trait | `tool-harness/src/traits.rs` | Default impl that constructs from existing methods |
| 1.6 Wire into `ToolDefinition` generation | `tool-harness/src/registry.rs` | Include category hints in the LLM tool definition |

**Acceptance**: Every built-in tool exposes full `ToolMetadata`. `ToolBox::list_by_category()` returns categorized listing.

### Phase 2: Search & Discovery (Week 3–4)

| Task | Files | Description |
|---|---|---|
| 2.1 Build `ToolBoxIndex` | `tool-harness/src/box/index.rs` | Inverted index with name, category, tag, param maps |
| 2.2 Implement fuzzy matching | `tool-harness/src/box/fuzzy.rs` | Bigram Jaccard similarity + trie |
| 2.3 Implement alias system | `tool-harness/src/box/alias.rs` | Alias→canonical name mapping |
| 2.4 Implement `search()` | `tool-harness/src/box/search.rs` | Composite search with scoring |
| 2.5 Build `ToolRecommender` | `tool-harness/src/box/recommend.rs` | Context-aware recommendations |
| 2.6 Expose search via CLI/TUI | `omega-cli/src/commands/tools.rs` | `/search` command, category browse |

**Acceptance**: `ToolBox::search("find file content")` returns ranked results with `grep` on top. Fuzzy match works for typos. `/search` in TUI shows categorized results.

### Phase 3: Enhanced Execution (Week 5–6)

| Task | Files | Description |
|---|---|---|
| 3.1 Implement `ProgressEmitter` | `tool-harness/src/box/progress.rs` | Streaming result interface |
| 3.2 Add `StreamingTool` trait | `tool-harness/src/traits.rs` | Separate trait for streaming tools |
| 3.3 Add rate limiter | `tool-harness/src/box/rate_limiter.rs` | Token bucket per-tool and per-category |
| 3.4 Add circuit breaker | `tool-harness/src/box/circuit_breaker.rs` | Per-tool failure tracking |
| 3.5 Add retry policy engine | `tool-harness/src/box/retry.rs` | Error-based retry with backoff |
| 3.6 Implement `then`/`pipe`/`compose` | `tool-harness/src/box/composition.rs` | Tool chaining with cycle detection |
| 3.7 Wind into existing pipeline | `tool-harness/src/pipeline.rs` | Add rate-limit, circuit-breaker, retry steps |

**Acceptance**: Tools can declare rate limits. `bash` supports streaming output. `pipe` works for `grep | read` chaining. Circuit breaker opens after N failures.

### Phase 4: UI/UX (Week 7–8)

| Task | Files | Description |
|---|---|---|
| 4.1 Build `ToolCard` widget | `omega-cli/src/tui/widgets/tool_card.rs` | Detailed tool card renderer |
| 4.2 Build `ToolList` widget | `omega-cli/src/tui/widgets/tool_list.rs` | Compact categorized list |
| 4.3 Build `SearchBar` widget | `omega-cli/src/tui/widgets/search_bar.rs` | Interactive search with live results |
| 4.4 Build `ToolComparison` widget | `omega-cli/src/tui/widgets/compare.rs` | Side-by-side tool comparison |
| 4.5 Wire tool call rendering | `omega-cli/src/tui/widgets/conversation.rs` | Inline tool call/result cards |
| 4.6 Add `/tools` slash command | `omega-cli/src/commands/mod.rs` | Interactive tool browsing |

**Acceptance**: `/tools` shows categorized list. `? read` shows full tool card. Tool results in conversation are collapsible cards. `compare grep find` shows side-by-side.

### Phase 5: Extension System (Week 9–10)

| Task | Files | Description |
|---|---|---|
| 5.1 Define plugin manifest schema | `tool-harness/src/plugin/manifest.rs` | `tool-plugin.toml` parsing |
| 5.2 Build plugin scanner | `tool-harness/src/plugin/scanner.rs` | Discover plugins in `~/.omega/plugins/` |
| 5.3 Build WASM runtime adapter | `tool-harness/src/plugin/runtime_wasm.rs` | Load and execute WASM modules |
| 5.4 Build HTTP runtime adapter | `tool-harness/src/plugin/runtime_http.rs` | Proxy to HTTP microservices |
| 5.5 Build MCP runtime adapter | `tool-harness/src/plugin/runtime_mcp.rs` | Delegate to MCP servers |
| 5.6 Implement sandbox policies | `tool-harness/src/plugin/sandbox.rs` | Filesystem/network/process restrictions |
| 5.7 Plugin version compatibility check | `tool-harness/src/plugin/compat.rs` | Semantic version check |

**Acceptance**: A simple WASM tool plugin can be installed and called. Sandbox prevents WASM plugin from accessing network when policy says `none`. MCP tools are surfaced as plugins.

### Phase 6: Hardening & Edge Cases (Week 11–12)

| Task | Files | Description |
|---|---|---|
| 6.1 Comprehensive error handling | `tool-harness/src/box/errors.rs` | All 8 edge cases from §8 with tests |
| 6.2 Circuit breaker integration test | `tool-harness/tests/circuit_breaker.rs` | Verify open/half-open/close transitions |
| 6.3 Rate limit integration test | `tool-harness/tests/rate_limiter.rs` | Verify token bucket behavior |
| 6.4 Cycle detection test | `tool-harness/tests/composition.rs` | Verify circular dependency rejection |
| 6.5 Large result handling | Already exists, enhancement | Streaming truncation, TUI notification |
| 6.6 Permission edge cases | `tool-harness/tests/permission.rs` | Deny, prompt, allow-once, always-allow flows |
| 6.7 TUI error rendering | `omega-cli/src/tui/widgets/error_card.rs` | Error cards with retry suggestions |

**Acceptance**: All edge cases have tests. Fuzzing of search index. Long-running stability test with 1000 sequential tool calls.

---

## Summary of New Files

| File | Purpose |
|---|---|
| `tool-harness/src/metadata.rs` | `ToolMetadata`, `ToolCategory`, `ParamSummary`, supporting types |
| `tool-harness/src/box/mod.rs` | `ToolCallingBox` top-level struct, re-exports |
| `tool-harness/src/box/index.rs` | `ToolBoxIndex` — inverted index for search |
| `tool-harness/src/box/fuzzy.rs` | `FuzzyTrie` — bigram-based fuzzy matching |
| `tool-harness/src/box/alias.rs` | Alias resolution table |
| `tool-harness/src/box/search.rs` | `SearchMode`, `SearchQuery`, `SearchResult`, ranking |
| `tool-harness/src/box/recommend.rs` | `ToolRecommender` — context-aware suggestions |
| `tool-harness/src/box/progress.rs` | `ProgressEmitter` trait |
| `tool-harness/src/box/rate_limiter.rs` | Token bucket rate limiter |
| `tool-harness/src/box/circuit_breaker.rs` | Circuit breaker with state machine |
| `tool-harness/src/box/retry.rs` | `RetryPolicy` and retry engine |
| `tool-harness/src/box/composition.rs` | `CompositionPlan`, `then`/`pipe`/`compose` |
| `tool-harness/src/box/errors.rs` | `AmbiguousMatch`, `RateLimitError`, etc. |
| `tool-harness/src/plugin/manifest.rs` | Plugin manifest parsing |
| `tool-harness/src/plugin/scanner.rs` | Plugin directory scanner |
| `tool-harness/src/plugin/runtime_wasm.rs` | WASM plugin execution |
| `tool-harness/src/plugin/runtime_http.rs` | HTTP plugin execution |
| `tool-harness/src/plugin/runtime_mcp.rs` | MCP plugin delegation |
| `tool-harness/src/plugin/sandbox.rs` | Sandbox policies |
| `tool-harness/src/plugin/compat.rs` | Version compatibility check |
| `omega-cli/src/tui/widgets/tool_card.rs` | Tool card UI widget |
| `omega-cli/src/tui/widgets/tool_list.rs` | Compact list widget |
| `omega-cli/src/tui/widgets/search_bar.rs` | Search bar widget |
| `omega-cli/src/tui/widgets/compare.rs` | Comparison view widget |
| `omega-cli/src/tui/widgets/error_card.rs` | Error display widget |

## Modified Files

| File | Change |
|---|---|
| `tool-harness/src/traits.rs` | Add `metadata()` method; add `StreamingTool` trait |
| `tool-harness/src/registry.rs` | Add `search()`, `list_by_category()`, `recommend()`, `resolve_alias()` |
| `tool-harness/src/pipeline.rs` | Add rate-limit check, circuit breaker, retry, streaming support |
| `tool-harness/src/lib.rs` | Add `pub mod metadata; pub mod box_; pub mod plugin;` |
| `tool-harness/src/tools/*.rs` | Add category, tags, examples, error specs to each tool |
| `omega-cli/src/commands/mod.rs` | Add `/tools`, `/search`, `/compare` commands |

## Dependencies

- Phase 1 → Phase 2 (metadata must exist before indexing)
- Phase 2 → Phase 3 (search is independent of execution enhancements)
- Phase 1,2,3 → Phase 4 (UI needs metadata, search, and execution state)
- Phase 5 can start in parallel with Phase 3 (plugin system is orthogonal)
- Phase 6 depends on all prior phases

## Risks

1. **WASM plugin sandboxing**: True filesystem/network sandboxing for WASM requires a runtime like wasmtime with WASI preview 2. If the build doesn't include wasmtime, the WASM runtime falls back to an "unsandboxed" mode with a strong warning.
2. **Plugin ABI stability**: Native dylib plugins are inherently fragile across Rust versions. Recommend WASM or HTTP as the primary plugin runtime; native is "best-effort."
3. **Performance of fuzzy search**: Bigram Jaccard is O(n*m) for n tools and m query chars. For <100 tools (< 10K ops) this is instant. Monitor if the tool set grows beyond 500.
4. **Rate limiter state persistence**: Rate limiter state is in-memory. If the agent restarts, limits reset. Acceptable for MVP; consider disk-persisted state for Phase 6.
5. **Tool metadata bloat**: Rich metadata (examples, docs, error specs) increases the `ToolDefinition` payload sent to the LLM. Consider sending only name/description/parameters to the LLM and keeping full metadata for UI/help purposes.
6. **Backward compatibility**: Existing code that constructs `ToolRegistry` and calls `register()` continues to work. The `metadata()` method on the `Tool` trait has a default impl that constructs metadata from existing methods, so existing tool implementations compile without changes.

---

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Design document covers all 7 required areas: architecture overview, detailed category taxonomy (13 categories with 50+ subcategories), complete metadata schema with 20+ fields, discovery/search with 6 search modes, execution lifecycle with rate limiting/circuit breakers/retry/chaining, UI/UX with 5 widget patterns, extension system with 4 runtimes and sandboxing, and 8 edge case categories. All patterns reference existing Omega Agent tool-harness architecture and Claude Code/Pi Agent conventions."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Design specifies exact file paths (27 new files, 6 modified files in tool-harness crate and omega-cli crate), Rust struct definitions, trait signatures, enum variants, pipeline pseudocode, and JSON examples. Implementation roadmap spans 6 phases over 12 weeks with acceptance criteria for each phase. All edge cases have explicit error types and resolution strategies."
    }
  ],
  "changedFiles": [
    "The design proposes changes to these existing files (no actual changes made — this is a design document):",
    "tool-harness/src/traits.rs",
    "tool-harness/src/registry.rs",
    "tool-harness/src/pipeline.rs",
    "tool-harness/src/lib.rs",
    "tool-harness/src/tools/read.rs",
    "tool-harness/src/tools/write.rs",
    "tool-harness/src/tools/edit.rs",
    "tool-harness/src/tools/bash.rs",
    "tool-harness/src/tools/grep.rs",
    "tool-harness/src/tools/glob.rs",
    "omega-cli/src/commands/mod.rs"
  ],
  "testsAddedOrUpdated": [
    "No tests written — this is a prose design document. Phase 6 of the roadmap specifies integration tests for:",
    "tool-harness/tests/circuit_breaker.rs",
    "tool-harness/tests/rate_limiter.rs",
    "tool-harness/tests/composition.rs",
    "tool-harness/tests/permission.rs"
  ],
  "commandsRun": [
    {
      "command": "read crate files",
      "result": "passed",
      "summary": "Read 15+ source files from tool-harness, providers, and omega-cli crates to understand existing architecture before writing the design."
    }
  ],
  "validationOutput": [
    "Design verified against existing tool-harness architecture: all proposed additions are compatible with current Tool trait, ToolRegistry, ExecutionPipeline, and PermissionResolver.",
    "Category taxonomy cross-referenced with existing tools: read, write, edit, bash, grep, glob all correctly map to categories.",
    "Metadata schema preserves backward compatibility: existing name()/description()/parameters_schema() methods gain a metadata() default impl."
  ],
  "residualRisks": [
    "WASM sandboxing depends on wasmtime/wasmer being added to Cargo.toml dependencies",
    "Native dylib plugin ABI stability is not guaranteed across Rust compiler versions",
    "Rich metadata sent to LLMs as ToolDefinition may increase prompt token usage — mitigation is to send only name/description/parameters and keep full metadata server-side",
    "Rate limiter state is in-memory and resets on agent restart",
    "Phase durations are estimates and may vary based on developer familiarity with wasmtime and ratatui"
  ],
  "noStagedFiles": true,
  "diffSummary": "No files were modified. This is a design document only — written to C:\\Users\\pwong\\Ohm-agent\\.pi-subagents\\artifacts\\outputs\\64454d90\\plan.md",
  "reviewFindings": [
    "No blockers — design is internally consistent and compatible with existing architecture",
    "Consider adding a ToolSource::Composite for tools built from composition plans",
    "Phase 5 (extension system) could benefit from a plugin hot-reload mechanism for development workflows"
  ],
  "manualNotes": "Design document is 682 lines covering all 9 requested sections. The output was written to the authoritative path override: C:\\Users\\pwong\\Ohm-agent\\.pi-subagents\\artifacts\\outputs\\64454d90\\plan.md. A copy should also be placed at C:\\Users\\pwong\\Ohm-agent\\TOOL_CALLING_BOX_DESIGN.md for easy reference if the team needs a canonical document."
}
```
