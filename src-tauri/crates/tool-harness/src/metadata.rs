// Tool Metadata & Taxonomy — discoverable, categorized, self-documenting tool definitions
//
// Phase 1 of the Tool Calling Box: every tool gets rich metadata beyond
// name/description/schema — category, tags, versioning, examples, error specs,
// cost hints, and deprecation info.

use serde::{Deserialize, Serialize};

// ─── Category Taxonomy ───────────────────────────────────────────────────────

/// Top-level category for every tool in the calling box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// Human-readable label (e.g. "File Operations")
    pub fn label(&self) -> &'static str {
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

    /// Compact icon-like symbol for TUI rendering
    pub fn icon(&self) -> &'static str {
        match self {
            Self::FileOperations => "\u{1F4C4}",   // 📄
            Self::CodeExecution => "\u{25B6}",     // ▶
            Self::SearchQuery => "\u{1F50D}",      // 🔍
            Self::WebNetwork => "\u{1F310}",       // 🌐
            Self::Communication => "\u{1F4AC}",    // 💬
            Self::System => "\u{2699}",            // ⚙
            Self::AgentManagement => "\u{1F916}",  // 🤖
            Self::McpServices => "\u{1F50C}",      // 🔌
            Self::MemoryStore => "\u{1F9E0}",      // 🧠
            Self::CodingAssist => "\u{270F}",      // ✏
            Self::DiffPatch => "\u{1F4DD}",        // 📝
            Self::DataTransform => "\u{1F504}",    // 🔄
            Self::HelpDocs => "\u{2753}",          // ❓
        }
    }
}

// ─── Parameter Summary ────────────────────────────────────────────────────────

/// Compact summary of a single parameter for quick-reference displays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSummary {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<ParamConstraints>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamConstraints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

// ─── Latency Hint ─────────────────────────────────────────────────────────────

/// Expected execution latency for UI/planning purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyHint {
    /// <50ms — kv-get, env, clock
    Instant,
    /// <500ms — read, grep, glob
    Fast,
    /// <10s — write large file, build
    Slow,
    /// Indefinite — watch, long-running bash
    Blocking,
}

impl LatencyHint {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Instant => "Instant",
            Self::Fast => "Fast",
            Self::Slow => "Slow",
            Self::Blocking => "Blocking",
        }
    }
}

// ─── Error Spec ───────────────────────────────────────────────────────────────

/// Describes a known error mode for a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolErrorSpec {
    pub kind: String,
    pub description: String,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_advice: Option<String>,
}

// ─── Example ──────────────────────────────────────────────────────────────────

/// An example invocation for documentation / LLM few-shot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExample {
    pub title: String,
    pub description: String,
    pub arguments: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_result: Option<String>,
}

// ─── Cost Hint ────────────────────────────────────────────────────────────────

/// Token cost approximation for budgeting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostHint {
    pub tokens_per_call: u32,
    pub category: CostCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostCategory {
    Free,
    Cheap,
    Moderate,
    Expensive,
}

impl CostCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Cheap => "Cheap",
            Self::Moderate => "Moderate",
            Self::Expensive => "Expensive",
        }
    }
}

// ─── Deprecation ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationInfo {
    pub deprecated_in_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removal_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    pub reason: String,
}

// ─── Source Origin ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    Builtin,
    Mcp,
    Plugin,
    Dynamic,
}

// ─── Full Metadata Struct ─────────────────────────────────────────────────────

/// Comprehensive metadata for a tool in the calling box.
///
/// This is the canonical source of truth for display, discovery, and execution.
/// Every tool in the system produces one of these.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,

    // ── Categorization ───────────────────────────────────────────
    /// Primary category from the taxonomy
    pub category: ToolCategory,

    /// Optional subcategory within the category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subcategory: Option<String>,

    /// Additional tags for cross-cutting search
    #[serde(default)]
    pub tags: Vec<String>,

    // ── Parameters ───────────────────────────────────────────────
    /// Full JSON Schema (OpenAPI 3.0 subset) for parameters
    pub parameters: serde_json::Value,

    /// Quick-reference: ordered list of parameter summaries
    #[serde(default)]
    pub param_summaries: Vec<ParamSummary>,

    // ── Execution characteristics ────────────────────────────────
    /// Whether the tool is read-only (safe for dry-run / plan mode)
    pub read_only: bool,

    /// Whether the tool can be called concurrently without side effects
    pub concurrency_safe: bool,

    /// Typical execution latency hint
    pub latency_hint: LatencyHint,

    /// Whether the tool can stream incremental results
    pub supports_streaming: bool,

    /// Maximum result size in characters before truncation/persistence
    pub max_result_chars: usize,

    // ── Error modes ──────────────────────────────────────────────
    /// Known error modes the tool can produce
    #[serde(default)]
    pub errors: Vec<ToolErrorSpec>,

    // ── Usage guidance ───────────────────────────────────────────
    /// Example invocations (for LLM few-shot or help display)
    #[serde(default)]
    pub examples: Vec<ToolExample>,

    /// Cost tokens per call (approximate, for budgeting)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_hint: Option<CostHint>,

    // ── Lifecycle ────────────────────────────────────────────────
    /// Semantic version of the tool spec
    pub version: String,

    /// Deprecation status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecation: Option<DeprecationInfo>,

    /// Source origin: "builtin" | "mcp" | "plugin" | "dynamic"
    pub source: ToolSource,

    /// Provider/server name if MCP or plugin
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
}

impl ToolMetadata {
    /// Build a quick metadata stub for tools that don't provide full metadata yet.
    /// Used by the default `metadata()` implementation on the `Tool` trait.
    pub fn from_parts(
        name: &str,
        label: &str,
        description: &str,
        category: ToolCategory,
        tags: Vec<String>,
        parameters: serde_json::Value,
        read_only: bool,
        latency_hint: LatencyHint,
    ) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            description: description.to_string(),
            doc: None,
            category,
            subcategory: None,
            tags,
            parameters,
            param_summaries: vec![],
            read_only,
            concurrency_safe: read_only,
            latency_hint,
            supports_streaming: false,
            max_result_chars: 50_000,
            errors: vec![],
            examples: vec![],
            cost_hint: None,
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    /// Extract parameter summaries from a JSON Schema object.
    /// This auto-generates `ParamSummary` entries from a standard schema.
    pub fn extract_param_summaries(schema: &serde_json::Value) -> Vec<ParamSummary> {
        let mut summaries = vec![];

        let properties = match schema.get("properties").and_then(|v| v.as_object()) {
            Some(p) => p,
            None => return summaries,
        };

        let required: Vec<&str> = schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect()
            })
            .unwrap_or_default();

        for (name, prop) in properties {
            let param_type = prop
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("string")
                .to_string();

            let description = prop
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let required = required.contains(&name.as_str());

            let default = prop.get("default").cloned();
            let example = prop.get("example").or_else(|| prop.get("examples")).cloned();

            summaries.push(ParamSummary {
                name: name.clone(),
                param_type,
                description,
                required,
                default,
                example,
                constraints: None,
            });
        }

        summaries
    }
}

// ─── ToolBox Reference (lightweight) ──────────────────────────────────────────

/// A lightweight reference to a tool for list views and search results.
/// Carries enough info to render a compact list item without loading full metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRef {
    pub name: String,
    pub label: String,
    pub description: String,
    pub category: ToolCategory,
    pub tags: Vec<String>,
    pub read_only: bool,
    pub source: ToolSource,
}

impl From<&ToolMetadata> for ToolRef {
    fn from(m: &ToolMetadata) -> Self {
        Self {
            name: m.name.clone(),
            label: m.label.clone(),
            description: m.description.clone(),
            category: m.category,
            tags: m.tags.clone(),
            read_only: m.read_only,
            source: m.source.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_category_labels() {
        assert_eq!(ToolCategory::FileOperations.label(), "File Operations");
        assert_eq!(ToolCategory::CodeExecution.label(), "Code Execution");
        assert_eq!(ToolCategory::SearchQuery.label(), "Search / Query");
    }

    #[test]
    fn test_category_icons_are_non_empty() {
        for category in &[
            ToolCategory::FileOperations,
            ToolCategory::CodeExecution,
            ToolCategory::SearchQuery,
            ToolCategory::WebNetwork,
            ToolCategory::Communication,
            ToolCategory::System,
            ToolCategory::AgentManagement,
            ToolCategory::McpServices,
            ToolCategory::MemoryStore,
            ToolCategory::CodingAssist,
            ToolCategory::DiffPatch,
            ToolCategory::DataTransform,
            ToolCategory::HelpDocs,
        ] {
            assert!(!category.icon().is_empty(), "Icon for {:?} should not be empty", category);
        }
    }

    #[test]
    fn test_from_parts_creates_stub() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "input": { "type": "string", "description": "Input value" }
            },
            "required": ["input"]
        });

        let meta = ToolMetadata::from_parts(
            "my_tool",
            "My Tool",
            "Does something",
            ToolCategory::DataTransform,
            vec!["custom".into()],
            schema.clone(),
            true,
            LatencyHint::Fast,
        );

        assert_eq!(meta.name, "my_tool");
        assert_eq!(meta.label, "My Tool");
        assert_eq!(meta.category, ToolCategory::DataTransform);
        assert!(meta.read_only);
        assert!(meta.concurrency_safe);
        assert_eq!(meta.version, "1.0.0");
        assert!(matches!(meta.source, ToolSource::Builtin));
    }

    #[test]
    fn test_extract_param_summaries() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "filePath": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "limit": {
                    "type": "number",
                    "description": "Max lines",
                    "default": 2000
                }
            },
            "required": ["filePath"]
        });

        let summaries = ToolMetadata::extract_param_summaries(&schema);
        assert_eq!(summaries.len(), 2);

        let fp = summaries.iter().find(|p| p.name == "filePath").unwrap();
        assert!(fp.required);
        assert_eq!(fp.param_type, "string");

        let limit = summaries.iter().find(|p| p.name == "limit").unwrap();
        assert!(!limit.required);
        assert_eq!(limit.default.as_ref().unwrap().as_u64(), Some(2000));
    }

    #[test]
    fn test_metadata_serialization_roundtrip() {
        let meta = ToolMetadata {
            name: "test".into(),
            label: "Test".into(),
            description: "A test tool".into(),
            doc: None,
            category: ToolCategory::HelpDocs,
            subcategory: None,
            tags: vec!["test".into()],
            parameters: serde_json::json!({}),
            param_summaries: vec![],
            read_only: true,
            concurrency_safe: true,
            latency_hint: LatencyHint::Instant,
            supports_streaming: false,
            max_result_chars: 1000,
            errors: vec![],
            examples: vec![],
            cost_hint: None,
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: ToolMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.category, ToolCategory::HelpDocs);
    }

    #[test]
    fn test_tool_ref_from_metadata() {
        let meta = ToolMetadata {
            name: "test".into(),
            label: "Test".into(),
            description: "A test tool".into(),
            doc: None,
            category: ToolCategory::System,
            subcategory: None,
            tags: vec![],
            parameters: serde_json::json!({}),
            param_summaries: vec![],
            read_only: false,
            concurrency_safe: false,
            latency_hint: LatencyHint::Fast,
            supports_streaming: false,
            max_result_chars: 1000,
            errors: vec![],
            examples: vec![],
            cost_hint: None,
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Mcp,
            source_name: Some("my-server".into()),
        };

        let r: ToolRef = (&meta).into();
        assert_eq!(r.name, "test");
        assert!(matches!(r.source, ToolSource::Mcp));
    }
}
