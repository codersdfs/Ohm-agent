use crate::{DomainScore, EntropyReport};
use harness::engine::GateEngine;
use harness::Language;
use std::path::Path;

pub struct DriftScanner;

impl DriftScanner {
    pub fn new() -> Self {
        Self
    }

    pub async fn scan(&self, project_root: &str) -> Result<EntropyReport, String> {
        let lang = detect_language(project_root);
        let mut engine = GateEngine::new(project_root.to_string(), lang.clone());

        let mut domains: Vec<DomainScore> = vec![];

        if let Ok(entries) = std::fs::read_dir(project_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && !path.to_string_lossy().starts_with('.') {
                    let domain_name = path.file_name().unwrap().to_string_lossy().to_string();
                    let (drift, violations) = scan_domain(&path, &mut engine, &lang);
                    let priority = if drift > 50.0 { 1 } else if drift > 20.0 { 2 } else { 3 };
                    domains.push(DomainScore {
                        name: domain_name,
                        drift,
                        priority,
                        violations: violations as usize,
                    });
                }
            }
        }

        let (root_drift, root_violations) = scan_domain(Path::new(project_root), &mut engine, &lang);
        domains.push(DomainScore {
            name: "root".to_string(),
            drift: root_drift,
            priority: if root_drift > 50.0 { 1 } else if root_drift > 20.0 { 2 } else { 3 },
            violations: root_violations as usize,
        });

        Ok(EntropyReport {
            domains,
            generated_pr: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }
}

fn detect_language(project_root: &str) -> Language {
    let paths: Vec<String> = std::fs::read_dir(project_root)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();

    Language::detect(&paths)
}

fn scan_domain(dir: &Path, engine: &mut GateEngine, _lang: &Language) -> (f64, u32) {
    let mut total_violations = 0u32;
    let mut total_files = 0u32;

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_source_file(&path) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let result = engine.check_file(&path.to_string_lossy(), &content);
                    total_violations += result.violations.len() as u32;
                    total_files += 1;
                }
            }
        }
    }

    let drift = if total_files == 0 {
        0.0
    } else {
        (total_violations as f64 / total_files as f64) * 10.0
    };

    (drift, total_violations)
}

fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rs") | Some("ts") | Some("tsx") | Some("js") | Some("py") | Some("go")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scan_detects_drift_in_project() {
        let scanner = DriftScanner::new();
        let report = scanner.scan(".").await.unwrap();
        assert!(!report.domains.is_empty(), "Should detect at least one domain");
    }

    #[test]
    fn detect_language_rust() {
        let dir = std::env::temp_dir().join("omega_lang_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        let lang = detect_language(dir.to_str().unwrap());
        assert_eq!(lang, Language::Rust);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_source_file_rs() {
        assert!(is_source_file(std::path::Path::new("test.rs")));
        assert!(is_source_file(std::path::Path::new("test.ts")));
        assert!(!is_source_file(std::path::Path::new("test.txt")));
    }
}
