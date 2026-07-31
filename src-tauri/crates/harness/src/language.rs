use serde::{Deserialize, Serialize};

/// Programming language enumeration for the Gate engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "rust")] Rust,
    #[serde(rename = "typescript")] TypeScript,
    #[serde(rename = "typescript-react")] TypeScriptReact,
    #[serde(rename = "javascript")] JavaScript,
    #[serde(rename = "python")] Python,
    #[serde(rename = "go")] Go,
    #[serde(rename = "csharp")] CSharp,
    #[serde(rename = "java")] Java,
    #[serde(rename = "other")] Other(String),
}

impl Language {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "rust" => Language::Rust,
            "typescript" => Language::TypeScript,
            "typescript-react" => Language::TypeScriptReact,
            "javascript" => Language::JavaScript,
            "python" => Language::Python,
            "go" => Language::Go,
            "csharp" => Language::CSharp,
            "java" => Language::Java,
            _ => Language::Other(s.to_string()),
        }
    }

    /// Returns an integer index suitable for model feature vectors
    pub fn to_index(&self) -> usize {
        match self {
            Language::Rust => 0,
            Language::TypeScript | Language::JavaScript | Language::TypeScriptReact => 1,
            Language::Python => 2,
            Language::Go => 3,
            Language::CSharp => 4,
            Language::Java => 5,
            Language::Other(_) => 6,
        }
    }

    /// Returns a string key suitable for lookup operations
    pub fn to_key(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::TypeScriptReact => "typescript-react",
            Language::JavaScript => "javascript",
            Language::Python => "python",
            Language::Go => "go",
            Language::CSharp => "csharp",
            Language::Java => "java",
            Language::Other(_) => "other",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "rust"),
            Language::TypeScript => write!(f, "typescript"),
            Language::TypeScriptReact => write!(f, "typescript-react"),
            Language::JavaScript => write!(f, "javascript"),
            Language::Python => write!(f, "python"),
            Language::Go => write!(f, "go"),
            Language::CSharp => write!(f, "csharp"),
            Language::Java => write!(f, "java"),
            Language::Other(s) => write!(f, "{}", s),
        }
    }
}
