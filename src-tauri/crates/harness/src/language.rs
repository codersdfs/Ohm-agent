use serde::{Deserialize, Serialize};

/// Programming language enumeration for the Gate engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "rust")]
    Rust,
    #[serde(rename = "typescript")]
    TypeScript,
    #[serde(rename = "typescript-react")]
    TypeScriptReact,
    #[serde(rename = "javascript")]
    JavaScript,
    #[serde(rename = "python")]
    Python,
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "csharp")]
    CSharp,
    #[serde(rename = "java")]
    Java,
    #[serde(rename = "other")]
    Other(String),
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

    /// Detect the primary programming language of a project from a set of
    /// directory entry paths (or manifest filenames). Looks for well-known
    /// manifest files: `Cargo.toml` → Rust, `package.json` → TypeScript,
    /// `requirements.txt`/`setup.py`/`pyproject.toml` → Python,
    /// `go.mod`/`go.sum` → Go, `*.csproj`/`*.sln` → C#, `*.java`/`pom.xml` → Java.
    /// Falls back to `Language::Other("unknown")` if no known manifest is found.
    pub fn detect(paths: &[String]) -> Language {
        let lower: Vec<String> = paths.iter().map(|p| p.to_lowercase()).collect();
        let has = |needle: &str| -> bool { lower.iter().any(|p| p.contains(needle)) };

        if has("cargo.toml") {
            return Language::Rust;
        }
        if has("package.json") {
            return Language::TypeScript;
        }
        if has("pyproject.toml") || has("requirements.txt") || has("setup.py") || has("setup.cfg") {
            return Language::Python;
        }
        if has("go.mod") || has("go.sum") {
            return Language::Go;
        }
        if has(".csproj") || has(".sln") {
            return Language::CSharp;
        }
        if has("pom.xml") || has("build.gradle") || has(".java") {
            return Language::Java;
        }
        Language::Other("unknown".to_string())
    }

    /// Returns a human-readable label for use in prompts and diagnostics.
    pub fn label(&self) -> String {
        match self {
            Language::Rust => "Rust".to_string(),
            Language::TypeScript => "TypeScript".to_string(),
            Language::TypeScriptReact => "TypeScript (React)".to_string(),
            Language::JavaScript => "JavaScript".to_string(),
            Language::Python => "Python".to_string(),
            Language::Go => "Go".to_string(),
            Language::CSharp => "C#".to_string(),
            Language::Java => "Java".to_string(),
            Language::Other(s) => s.clone(),
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
