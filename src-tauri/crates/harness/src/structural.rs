use super::Language;
use super::Violation;
use super::ViolationCategory;

const MAX_FILE_LINES: usize = 500;
const MAX_FUNCTION_LINES: usize = 80;
const MAX_LINE_LENGTH: usize = 120;

pub struct StructuralCheck;

impl StructuralCheck {
    pub fn check(content: &str, path: &str, _lang: &Language) -> Vec<Violation> {
        let mut violations = vec![];

        // Line count
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        if total_lines > MAX_FILE_LINES {
            violations.push(Violation {
                category: ViolationCategory::Structural,
                message: format!(
                    "File too long: {} lines (max {})",
                    total_lines, MAX_FILE_LINES
                ),
                tool_hint: Some(format!(
                    "Split into modules. Max {} lines per file.",
                    MAX_FILE_LINES
                )),
                line: None,
            });
        }

        // Line length
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                continue;
            }
            if line.chars().count() > MAX_LINE_LENGTH {
                violations.push(Violation {
                    category: ViolationCategory::Structural,
                    message: format!(
                        "Line too long: {} chars (max {})",
                        line.chars().count(),
                        MAX_LINE_LENGTH
                    ),
                    tool_hint: Some("Break line or extract expression".into()),
                    line: Some((i + 1) as u32),
                });
                // Cap at 5 line length violations to avoid noise
                if violations
                    .iter()
                    .filter(|v| v.message.contains("Line too long"))
                    .count()
                    >= 5
                {
                    break;
                }
            }
        }

        // Function length (tree-sitter based)
        Self::check_function_length(content, path, _lang, &mut violations);

        // Naming conventions
        Self::check_naming(path, content, &mut violations);

        // Import ordering
        if content.contains("use ") && path.ends_with(".rs") {
            Self::check_import_order(content, &mut violations);
        }

        violations
    }

    fn check_function_length(content: &str, _path: &str, lang: &Language, violations: &mut Vec<Violation>) {
        let metrics = crate::tree_sitter_metrics::analyze_file(content, lang);
        for m in metrics {
            if m.line_count > MAX_FUNCTION_LINES as u32 {
                violations.push(Violation {
                    category: ViolationCategory::Structural,
                    message: format!(
                        "Function `{}` too long: {} lines (max {})",
                        m.name, m.line_count, MAX_FUNCTION_LINES
                    ),
                    tool_hint: Some(format!(
                        "Refactor into smaller functions. Max {} lines per function.",
                        MAX_FUNCTION_LINES
                    )),
                    line: Some(m.start_line),
                });
            }
            // Also check cyclomatic complexity
            if m.cyclomatic_complexity > 10 {
                violations.push(Violation {
                    category: ViolationCategory::Structural,
                    message: format!(
                        "Function `{}` too complex: cyclomatic complexity {} (max 10)",
                        m.name, m.cyclomatic_complexity
                    ),
                    tool_hint: Some("Extract helper functions to reduce branching".into()),
                    line: Some(m.start_line),
                });
            }
        }
    }

    fn check_naming(path: &str, content: &str, violations: &mut Vec<Violation>) {
        if path.ends_with(".rs") {
            // Check for non-snake-case function names
            let re = regex::Regex::new(r"\bfn\s+([A-Z][a-zA-Z0-9_]*)").ok();
            if let Some(re) = re {
                for cap in re.captures_iter(content) {
                    if let Some(name) = cap.get(1) {
                        violations.push(Violation {
                            category: ViolationCategory::Structural,
                            message: format!(
                                "Function `{}` should use snake_case naming",
                                name.as_str()
                            ),
                            tool_hint: Some(format!(
                                "Rename to `{}`",
                                name.as_str()
                                    .chars()
                                    .enumerate()
                                    .map(|(i, c)| if i == 0 { c.to_ascii_lowercase() } else { c })
                                    .collect::<String>()
                            )),
                            line: None,
                        });
                    }
                }
            }
        }

        if path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".js") {
            // Check classes use PascalCase
            let re = regex::Regex::new(r"\bclass\s+([a-z][a-zA-Z0-9_]*)").ok();
            if let Some(re) = re {
                for cap in re.captures_iter(content) {
                    if let Some(name) = cap.get(1) {
                        let pascal = name
                            .as_str()
                            .chars()
                            .next()
                            .unwrap_or(' ')
                            .to_ascii_uppercase()
                            .to_string()
                            + &name.as_str()[1..];
                        violations.push(Violation {
                            category: ViolationCategory::Structural,
                            message: format!("Class `{}` should use PascalCase", name.as_str()),
                            tool_hint: Some(format!("Rename to `{}`", pascal)),
                            line: None,
                        });
                    }
                }
            }
        }
    }

    fn check_import_order(content: &str, violations: &mut Vec<Violation>) {
        let use_lines: Vec<(usize, &str)> = content
            .lines()
            .enumerate()
            .filter(|(_, l)| {
                l.trim().starts_with("use ")
                    && !l.trim().starts_with("use crate::")
                    && !l.trim().starts_with("use self::")
            })
            .collect();

        if use_lines.len() < 2 {
            return;
        }

        // Check std imports come before external
        let mut saw_external = false;
        for (i, line) in &use_lines {
            let trimmed = line.trim();
            if !trimmed.starts_with("use std::")
                && !trimmed.starts_with("use core::")
                && !trimmed.starts_with("use alloc::")
            {
                saw_external = true;
            } else if saw_external {
                violations.push(Violation {
                    category: ViolationCategory::Structural,
                    message: "Import order: std/core/alloc imports should come before external crate imports".into(),
                    tool_hint: Some("Group std imports first, then external crates, then crate:: imports".into()),
                    line: Some((i + 1) as u32),
                });
                break;
            }
        }
    }

    pub fn check_file_size(path: &str) -> Vec<Violation> {
        let mut violations = vec![];
        if let Ok(meta) = std::fs::metadata(path) {
            let size = meta.len();
            if size > 100_000 {
                violations.push(Violation {
                    category: ViolationCategory::Structural,
                    message: format!("File too large: {} KB (max 100 KB)", size / 1024),
                    tool_hint: Some("Split into multiple files".into()),
                    line: None,
                });
            }
        }
        violations
    }

    pub fn check_file_name(path: &str) -> Vec<Violation> {
        let mut violations = vec![];
        if let Some(name) = std::path::Path::new(path).file_stem() {
            let name = name.to_string_lossy();
            if name.contains('-') && path.ends_with(".rs") {
                violations.push(Violation {
                    category: ViolationCategory::Structural,
                    message: format!(
                        "Rust file names should use underscores, not hyphens: `{}`",
                        name
                    ),
                    tool_hint: Some(format!("Rename to `{}`", name.replace('-', "_"))),
                    line: None,
                });
            }
        }
        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_too_long() {
        let lines = vec!["// test"; 600];
        let content = lines.join("\n");
        let violations = StructuralCheck::check(&content, "test.rs", &Language::Rust);
        let line_violation = violations
            .iter()
            .find(|v| v.message.contains("File too long"));
        assert!(
            line_violation.is_some(),
            "Should detect file too long: {:?}",
            violations
        );
    }

    #[test]
    fn test_function_too_long() {
        let lines = vec!["    println!(\"test\");"; 100];
        let content = format!("fn too_long() {{\n{}\n}}", lines.join("\n"));
        let violations = StructuralCheck::check(&content, "test.rs", &Language::Rust);
        let fn_violation = violations.iter().find(|v| v.message.contains("Function"));
        assert!(
            fn_violation.is_some(),
            "Should detect function too long: {:?}",
            violations
        );
    }

    #[test]
    fn test_line_too_long() {
        let long_line = format!("let x = {};", "1 + ".repeat(40));
        let content = format!("fn test() {{}}\n{}", long_line);
        let violations = StructuralCheck::check(&content, "test.rs", &Language::Rust);
        let line_len = violations
            .iter()
            .find(|v| v.message.contains("Line too long"));
        assert!(
            line_len.is_some(),
            "Should detect line too long: {:?}",
            violations
        );
    }

    #[test]
    fn test_snake_case_naming() {
        let content = "fn CamelCase() {}";
        let violations = StructuralCheck::check(content, "test.rs", &Language::Rust);
        let naming = violations.iter().find(|v| v.message.contains("snake_case"));
        assert!(
            naming.is_some(),
            "Should flag non-snake-case function names: {:?}",
            violations
        );
    }

    #[test]
    fn test_import_order() {
        let content = "use serde::Serialize;\nuse std::collections::HashMap;\n";
        let violations = StructuralCheck::check(content, "test.rs", &Language::Rust);
        let import = violations
            .iter()
            .find(|v| v.message.contains("Import order"));
        assert!(
            import.is_some(),
            "Should detect wrong import order: {:?}",
            violations
        );
    }

    #[test]
    fn test_file_name_hyphen() {
        let violations = StructuralCheck::check_file_name("my-mod.rs");
        assert!(
            !violations.is_empty(),
            "Should flag hyphen in Rust filename"
        );
    }

    #[test]
    fn test_file_name_underscore() {
        let violations = StructuralCheck::check_file_name("my_mod.rs");
        assert!(
            violations.is_empty(),
            "Should accept underscores in Rust filename"
        );
    }

    #[test]
    fn test_short_file_passes() {
        let content = "fn main() { println!(\"hi\"); }";
        let violations = StructuralCheck::check(content, "main.rs", &Language::Rust);
        let line_violation = violations
            .iter()
            .find(|v| v.message.contains("File too long"));
        assert!(line_violation.is_none(), "Should not flag short file");
    }

    #[test]
    fn test_short_function_passes() {
        let content = "fn short() {\n    let x = 1;\n    println!(\"{}\", x);\n}";
        let violations = StructuralCheck::check(content, "test.rs", &Language::Rust);
        let fn_violation = violations.iter().find(|v| v.message.contains("Function"));
        assert!(
            fn_violation.is_none(),
            "Should not flag short function: {:?}",
            violations
        );
    }
}
