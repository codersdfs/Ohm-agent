use crate::Language;
use crate::Violation;
use crate::ViolationCategory;

/// Golden rules are permanent anti-patterns that are always enforced.
/// They are seeded with high-frequency count so they're auto-promoted from the start.
pub struct GoldenRules;

impl GoldenRules {
    pub fn check(content: &str, path: &str, lang: &Language) -> Vec<Violation> {
        let mut violations = vec![];

        match lang {
            Language::Rust => Self::check_rust(content, &mut violations),
            Language::TypeScript | Language::TypeScriptReact => {
                Self::check_typescript(content, path, &mut violations)
            }
            Language::JavaScript => Self::check_javascript(content, &mut violations),
            Language::Python => Self::check_python(content, &mut violations),
            Language::Go => Self::check_go(content, &mut violations),
            _ => {}
        }

        // Cross-language golden rules
        Self::check_cross_language(content, path, &mut violations);

        violations
    }

    fn check_rust(content: &str, violations: &mut Vec<Violation>) {
        // Unsafe blocks without justification
        if let Ok(re) = regex::Regex::new(r"unsafe\s*\{") {
            if re.is_match(content) && !content.contains("// SAFETY:") {
                violations.push(Violation {
                    category: ViolationCategory::Golden,
                    message: "Unsafe block without SAFETY comment: every unsafe block must document safety invariants".into(),
                    tool_hint: Some("Add `// SAFETY: <reason>` comment above the unsafe block".into()),
                    line: None,
                });
            }
        }

        // TODO/FIXME tracking (once per file)
        let marker_re = regex::Regex::new(r"(?i)\bTODO\b|\bFIXME\b|\bHACK\b|\bXXX\b").ok();
        if let Some(ref re) = marker_re {
            if re.is_match(content) {
                violations.push(Violation {
                    category: ViolationCategory::Golden,
                    message: "Unresolved TODO/FIXME/HACK/XXX markers: address before merging"
                        .into(),
                    tool_hint: Some(
                        "Either fix the issue or create a tracking ticket and reference it".into(),
                    ),
                    line: None,
                });
            }
        }

        // Debug formatting in production code
        if let Ok(re) = regex::Regex::new(r#"\{:\?\}"#) {
            if re.is_match(content) && !content.contains("#[cfg(test)]") {
                violations.push(Violation {
                    category: ViolationCategory::Golden,
                    message:
                        "Debug formatting `{:?}` in non-test code: remove or gate behind cfg(test)"
                            .into(),
                    tool_hint: Some("Use `Display` impl instead or wrap in `#[cfg(test)]`".into()),
                    line: None,
                });
            }
        }
    }

    fn check_typescript(content: &str, _path: &str, violations: &mut Vec<Violation>) {
        // eslint-disable without re-enable
        let eslint_re = regex::Regex::new(r"//\s*eslint-disable\s").ok();
        if let Some(ref re) = eslint_re {
            if re.is_match(content) && !content.contains("eslint-enable") {
                violations.push(Violation {
                    category: ViolationCategory::Golden,
                    message:
                        "eslint-disable without matching eslint-enable: disable at minimum scope"
                            .into(),
                    tool_hint: Some(
                        "Add `// eslint-enable <rule>` after the suppressed block".into(),
                    ),
                    line: None,
                });
            }
        }

        // TODO/FIXME tracking (once per file)
        let marker_re = regex::Regex::new(r"(?i)\bTODO\b|\bFIXME\b|\bHACK\b|\bXXX\b").ok();
        if let Some(ref re) = marker_re {
            if re.is_match(content) {
                violations.push(Violation {
                    category: ViolationCategory::Golden,
                    message: "Unresolved TODO/FIXME/HACK/XXX markers: address before merging"
                        .into(),
                    tool_hint: Some(
                        "Either fix the issue or create a tracking ticket and reference it".into(),
                    ),
                    line: None,
                });
            }
        }

        // console.log in production
        if content.contains("console.log") && !_path.contains("test") {
            violations.push(Violation {
                category: ViolationCategory::Golden,
                message: "console.log in production code: use structured logging".into(),
                tool_hint: Some("Replace with a proper logger or remove".into()),
                line: None,
            });
        }
    }

    fn check_javascript(content: &str, violations: &mut Vec<Violation>) {
        if content.contains("==") && !content.contains("===") {
            violations.push(Violation {
                category: ViolationCategory::Golden,
                message: "Using `==` instead of `===`: use strict equality".into(),
                tool_hint: Some("Replace `==` with `===`".into()),
                line: None,
            });
        } else if let Ok(re) = regex::Regex::new(r"(?<![!<>=])==(?!=)") {
            if re.is_match(content) {
                violations.push(Violation {
                    category: ViolationCategory::Golden,
                    message: "Using `==` instead of `===`: use strict equality".into(),
                    tool_hint: Some("Replace `==` with `===`".into()),
                    line: None,
                });
            }
        }
    }

    fn check_python(content: &str, violations: &mut Vec<Violation>) {
        // Wildcard imports
        if content.contains("from ") && content.contains(" import *") {
            violations.push(Violation {
                category: ViolationCategory::Golden,
                message: "Wildcard import `import *`: import only what you need".into(),
                tool_hint: Some(
                    "Replace `from module import *` with `from module import Foo`".into(),
                ),
                line: None,
            });
        }
    }

    fn check_go(content: &str, violations: &mut Vec<Violation>) {
        if content.contains("import _ ") {
            violations.push(Violation {
                category: ViolationCategory::Golden,
                message: "Blank import `import _` used: ensure it has a side-effect init function"
                    .into(),
                tool_hint: Some("Verify the imported package init function or remove".into()),
                line: None,
            });
        }
    }

    fn check_cross_language(content: &str, path: &str, violations: &mut Vec<Violation>) {
        // Hardcoded secrets (match password/secret/token followed by = or : and a quoted value)
        let secret_re =
            regex::Regex::new(r#"(?i)\b(password|secret|api[_\s]?key|token|credential)\b"#).ok();
        if let Some(re) = secret_re {
            if re.is_match(content) && !path.contains("test") && !path.contains(".env.example") {
                violations.push(Violation {
                    category: ViolationCategory::Golden,
                    message: "Hardcoded credential detected: use environment variables or a secrets manager".into(),
                    tool_hint: Some("Replace with `std::env::var(\"KEY\")` or `.env` file".into()),
                    line: None,
                });
            }
        }

        // Binary blob in text files
        if let Ok(re) = regex::Regex::new(r"([\x00-\x08\x0B\x0C\x0E-\x1F]){4,}") {
            if re.is_match(content) {
                violations.push(Violation {
                    category: ViolationCategory::Golden,
                    message: "Binary content detected in text file".into(),
                    tool_hint: Some("Remove binary content or move to a binary asset file".into()),
                    line: None,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Language;

    #[test]
    fn test_rust_unsafe_without_safety() {
        let content = "unsafe { ptr.read() }";
        let violations = GoldenRules::check(content, "test.rs", &Language::Rust);
        let unsafe_v = violations.iter().find(|v| v.message.contains("SAFETY"));
        assert!(
            unsafe_v.is_some(),
            "Should flag unsafe without SAFETY comment: {:?}",
            violations
        );
    }

    #[test]
    fn test_rust_unsafe_with_safety_passes() {
        let content = "// SAFETY: we checked the pointer is valid\nunsafe { ptr.read() }";
        let violations = GoldenRules::check(content, "test.rs", &Language::Rust);
        let unsafe_v = violations.iter().find(|v| v.message.contains("SAFETY"));
        assert!(unsafe_v.is_none(), "Should pass unsafe with SAFETY comment");
    }

    #[test]
    fn test_todo_markers_detected() {
        let content = "// TODO: implement this later\nfn foo() {}";
        let violations = GoldenRules::check(content, "test.rs", &Language::Rust);
        let todo_v = violations.iter().find(|v| v.message.contains("TODO"));
        assert!(
            todo_v.is_some(),
            "Should flag TODO markers: {:?}",
            violations
        );
    }

    #[test]
    fn test_console_log_detected() {
        let content = "console.log('hello');";
        let violations = GoldenRules::check(content, "app.ts", &Language::TypeScript);
        let log_v = violations
            .iter()
            .find(|v| v.message.contains("console.log"));
        assert!(log_v.is_some(), "Should flag console.log: {:?}", violations);
    }

    #[test]
    fn test_console_log_in_test_passes() {
        let content = "console.log('debug');";
        let violations = GoldenRules::check(content, "app.test.ts", &Language::TypeScript);
        let log_v = violations
            .iter()
            .find(|v| v.message.contains("console.log"));
        assert!(log_v.is_none(), "Should pass console.log in test files");
    }

    #[test]
    fn test_hardcoded_secret() {
        let content = "let password = 'supersecret123';";
        let violations = GoldenRules::check(content, "config.ts", &Language::TypeScript);
        let secret_v = violations.iter().find(|v| v.message.contains("credential"));
        assert!(
            secret_v.is_some(),
            "Should flag hardcoded secret: {:?}",
            violations
        );
    }

    #[test]
    fn test_hardcoded_secret_in_test_passes() {
        let content = "let password = 'test123';";
        let violations = GoldenRules::check(content, "config.test.ts", &Language::TypeScript);
        let secret_v = violations.iter().find(|v| v.message.contains("credential"));
        assert!(secret_v.is_none(), "Should pass hardcoded secret in test");
    }

    #[test]
    fn test_wildcard_import_python() {
        let content = "from math import *";
        let violations = GoldenRules::check(content, "calc.py", &Language::Python);
        let wildcard_v = violations.iter().find(|v| v.message.contains("Wildcard"));
        assert!(
            wildcard_v.is_some(),
            "Should flag wildcard import: {:?}",
            violations
        );
    }

    #[test]
    fn test_loose_equality_javascript() {
        let content = "if (x == 5) { }";
        let violations = GoldenRules::check(content, "app.js", &Language::JavaScript);
        let eq_v = violations.iter().find(|v| v.message.contains("=="));
        assert!(
            eq_v.is_some(),
            "Should flag loose equality: {:?}",
            violations
        );
    }

    #[test]
    fn test_debug_formatting() {
        let content = "println!(\"{:?}\", x);";
        let violations = GoldenRules::check(content, "main.rs", &Language::Rust);
        let debug_v = violations
            .iter()
            .find(|v| v.message.contains("Debug formatting"));
        assert!(
            debug_v.is_some(),
            "Should flag debug formatting: {:?}",
            violations
        );
    }

    #[test]
    fn test_eslint_disable_without_enable() {
        let content = "// eslint-disable no-unused-vars\nconst x = 5;";
        let violations = GoldenRules::check(content, "app.ts", &Language::TypeScript);
        let eslint_v = violations
            .iter()
            .find(|v| v.message.contains("eslint-disable"));
        assert!(
            eslint_v.is_some(),
            "Should flag eslint-disable without enable: {:?}",
            violations
        );
    }

    #[test]
    fn test_eslint_with_enable_passes() {
        let content =
            "// eslint-disable no-unused-vars\nconst x = 5;\n// eslint-enable no-unused-vars";
        let violations = GoldenRules::check(content, "app.ts", &Language::TypeScript);
        let eslint_v = violations
            .iter()
            .find(|v| v.message.contains("eslint-disable"));
        assert!(eslint_v.is_none(), "Should pass eslint-disable with enable");
    }
}
