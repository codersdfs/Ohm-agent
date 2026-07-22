pub mod engine;
pub mod golden;
pub mod patterns;
pub mod persistence;
pub mod repeated;
pub mod rules;
pub mod scoring;
pub mod structural;
pub mod taste;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    TypeScriptReact,
    JavaScript,
    Python,
    Go,
    CSharp,
    Java,
    Other(String),
}

impl Language {
    pub fn detect(paths: &[String]) -> Self {
        for p in paths {
            if p.ends_with("Cargo.toml") {
                return Self::Rust;
            }
            if p.ends_with("package.json") {
                if paths.iter().any(|x| x.ends_with(".tsx")) {
                    return Self::TypeScriptReact;
                }
                return Self::TypeScript;
            }
            if p.ends_with("pyproject.toml") || p.ends_with("requirements.txt") {
                return Self::Python;
            }
            if p.ends_with("go.mod") {
                return Self::Go;
            }
            if p.ends_with(".csproj") {
                return Self::CSharp;
            }
            if p.ends_with("pom.xml") || p.ends_with("build.gradle") {
                return Self::Java;
            }
        }
        if paths.iter().any(|x| x.ends_with(".rs")) {
            return Self::Rust;
        }
        if paths.iter().any(|x| x.ends_with(".tsx")) {
            return Self::TypeScriptReact;
        }
        if paths.iter().any(|x| x.ends_with(".ts")) {
            return Self::TypeScript;
        }
        if paths.iter().any(|x| x.ends_with(".js")) {
            return Self::JavaScript;
        }
        if paths.iter().any(|x| x.ends_with(".py")) {
            return Self::Python;
        }
        if paths.iter().any(|x| x.ends_with(".go")) {
            return Self::Go;
        }
        if paths.iter().any(|x| x.ends_with(".cs")) {
            return Self::CSharp;
        }
        if paths.iter().any(|x| x.ends_with(".java")) {
            return Self::Java;
        }
        Self::Other("unknown".into())
    }

    pub fn to_key(&self) -> String {
        match self {
            Self::Rust => "rust".into(),
            Self::TypeScript => "typescript".into(),
            Self::TypeScriptReact => "typescript".into(),
            Self::JavaScript => "javascript".into(),
            Self::Python => "python".into(),
            Self::Go => "go".into(),
            Self::CSharp => "csharp".into(),
            Self::Java => "java".into(),
            Self::Other(o) => o.clone(),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Rust => "Rust",
            Self::TypeScript => "TypeScript",
            Self::TypeScriptReact => "TypeScript (React)",
            Self::JavaScript => "JavaScript",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::CSharp => "C#",
            Self::Java => "Java",
            Self::Other(o) => o.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub passed: bool,
    pub score: u32,
    pub violations: Vec<Violation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub category: ViolationCategory,
    pub message: String,
    pub tool_hint: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ViolationCategory {
    Structural,
    Taste,
    Golden,
    Repeated,
}

impl GateResult {
    pub fn pass() -> Self {
        Self {
            passed: true,
            score: 100,
            violations: vec![],
        }
    }

    pub fn fail(score: u32, violations: Vec<Violation>) -> Self {
        Self {
            passed: score >= 80,
            score,
            violations,
        }
    }
}
