use super::Language;
use super::Violation;
use super::ViolationCategory;

pub struct TasteCheck;

impl TasteCheck {
    pub fn check(content: &str, path: &str, lang: &Language) -> Vec<Violation> {
        let mut violations = vec![];

        match lang {
            Language::Rust => Self::check_rust(content, &mut violations),
            Language::TypeScript | Language::TypeScriptReact | Language::JavaScript => {
                Self::check_typescript(content, path, &mut violations);
            }
            Language::Python => Self::check_python(content, &mut violations),
            _ => {}
        }

        violations
    }

    fn check_rust(content: &str, violations: &mut Vec<Violation>) {
        // Check for unnecessary cloning
        let re = regex::Regex::new(r"\.clone\(\)").ok();
        if let Some(re) = re {
            if re.find_iter(content).count() > 3 {
                violations.push(Violation {
                    category: ViolationCategory::Taste,
                    message: "Excessive `.clone()` usage (>3): prefer borrowing or Cow".into(),
                    tool_hint: Some("Use `&` references or `Cow<'_, T>` instead of cloning".into()),
                    line: None,
                });
            }
        }

        // Check for unwrap usage
        let re = regex::Regex::new(r"\.unwrap\(\)").ok();
        if let Some(re) = re {
            let count = re.find_iter(content).count();
            if count > 2 {
                violations.push(Violation {
                    category: ViolationCategory::Taste,
                    message: format!("Excessive `.unwrap()` usage ({}): prefer `?`, `.ok()`, or proper error handling", count),
                    tool_hint: Some("Replace `unwrap()` with `?` or pattern match on Result/Option".into()),
                    line: None,
                });
            }
        }

        // Check for missing error types (bare String errors)
        if content.contains("-> Result<") && content.contains(", String>") {
            violations.push(Violation {
                category: ViolationCategory::Taste,
                message:
                    "Using `String` as error type: prefer a proper error enum or `anyhow::Error`"
                        .into(),
                tool_hint: Some(
                    "Define a custom error enum or use `anyhow::Error` / `thiserror`".into(),
                ),
                line: None,
            });
        }

        // Check for commented-out code
        if let Ok(re) = regex::Regex::new(r"//\s*.*fn\s+\w+\s*\(|//\s*.*impl\s+\w+|//\s*.*pub\s+fn")
        {
            if re.is_match(content) {
                violations.push(Violation {
                    category: ViolationCategory::Taste,
                    message: "Commented-out code detected: remove dead code instead of commenting"
                        .into(),
                    tool_hint: Some("Delete commented-out code or move to a scratch file".into()),
                    line: None,
                });
            }
        }
    }

    fn check_typescript(content: &str, path: &str, violations: &mut Vec<Violation>) {
        // Check for explicit `any` type
        let re = regex::Regex::new(r":\s*any(\s|;|,|\)|\])").ok();
        if let Some(re) = re {
            if re.is_match(content) {
                violations.push(Violation {
                    category: ViolationCategory::Taste,
                    message: "Using `any` type: prefer `unknown` or a proper interface".into(),
                    tool_hint: Some(
                        "Replace `any` with `unknown` and add type guards, or define an interface"
                            .into(),
                    ),
                    line: None,
                });
            }
        }

        // Check for var usage (should be const/let)
        if content.contains("\nvar ") {
            violations.push(Violation {
                category: ViolationCategory::Taste,
                message: "Using `var`: prefer `const` or `let`".into(),
                tool_hint: Some(
                    "Replace `var` with `const` for immutable bindings or `let` for mutable".into(),
                ),
                line: None,
            });
        }

        // Check for non-null assertion (!.)
        // F-harness-05 fix: operator precedence — && binds tighter than ||,
        // so the original `re.is_match(content) && path.ends_with(".ts") || path.ends_with(".tsx")`
        // parsed as `(match && .ts) || .tsx`, firing on ANY .tsx file regardless of regex match.
        let non_null_re = regex::Regex::new(r"\w+!\.\w+").unwrap();
        if non_null_re.is_match(content) && (path.ends_with(".ts") || path.ends_with(".tsx")) {
            violations.push(Violation {
                category: ViolationCategory::Taste,
                message: "Non-null assertion (`!`) bypasses type safety: prefer optional chaining or type guards".into(),
                tool_hint: Some("Replace `x!.foo` with `x?.foo` or add a proper null check".into()),
                line: None,
            });
        }

        // Check for magic numbers
        // F-harness-02 fix: `(?<!\w)` and `(?!\w)` look-around not supported by `regex` crate.
        // Replace with numeric regex + manual word-boundary check.
        let num_re = regex::Regex::new(r"[0-9]+").unwrap();
        let chars: Vec<char> = content.chars().collect();
        let mut magic_count = 0usize;
        for m in num_re.find_iter(content) {
            let s = m.start();
            let e = m.end();
            if let Ok(n) = content[s..e].parse::<u64>() {
                // Magic number: single digit 3-9, or any number >= 10
                let is_magic = (n >= 3 && n <= 9) || n >= 10;
                if is_magic {
                    // Check word boundaries: preceding char not word, following char not word
                    let prev_ok = s == 0 || !chars[s.saturating_sub(1)].is_alphanumeric();
                    let next_ok = e >= chars.len() || !chars[e].is_alphanumeric();
                    if prev_ok && next_ok {
                        magic_count += 1;
                    }
                }
            }
        }
        if magic_count > 3 {
            violations.push(Violation {
                category: ViolationCategory::Taste,
                message: "Magic numbers detected: extract to named constants".into(),
                tool_hint: Some("Replace magic numbers with `const` declarations".into()),
                line: None,
            });
        }
    }

    fn check_python(content: &str, violations: &mut Vec<Violation>) {
        // Check for bare except
        if content.contains("except:") {
            violations.push(Violation {
                category: ViolationCategory::Taste,
                message: "Bare `except:` clause: specify exception type".into(),
                tool_hint: Some("Replace `except:` with `except SpecificError:`".into()),
                line: None,
            });
        }

        // Check for mutable default args
        if let Ok(re) = regex::Regex::new(r"def \w+\(.*=\s*\[|def \w+\(.*=\s*\{") {
            if re.is_match(content) {
                violations.push(Violation {
                    category: ViolationCategory::Taste,
                    message: "Mutable default argument: use `None` instead of `[]` or `{}`".into(),
                    tool_hint: Some(
                        "Use `def fn(x=None):` and assign inside the function body".into(),
                    ),
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
    fn test_rust_excessive_clone() {
        let content =
            "let a = x.clone();\nlet b = y.clone();\nlet c = z.clone();\nlet d = w.clone();";
        let violations = TasteCheck::check(content, "test.rs", &Language::Rust);
        let clone_v = violations.iter().find(|v| v.message.contains("clone()"));
        assert!(
            clone_v.is_some(),
            "Should flag excessive cloning: {:?}",
            violations
        );
    }

    #[test]
    fn test_rust_excessive_unwrap() {
        let content = "let a = x.unwrap();\nlet b = y.unwrap();\nlet c = z.unwrap();";
        let violations = TasteCheck::check(content, "test.rs", &Language::Rust);
        let unwrap_v = violations.iter().find(|v| v.message.contains("unwrap()"));
        assert!(
            unwrap_v.is_some(),
            "Should flag excessive unwrap: {:?}",
            violations
        );
    }

    #[test]
    fn test_rust_string_error_type() {
        let content = "fn do_stuff() -> Result<String, String> { Ok(\"hi\".into()) }";
        let violations = TasteCheck::check(content, "test.rs", &Language::Rust);
        let err_type = violations.iter().find(|v| v.message.contains("error type"));
        assert!(
            err_type.is_some(),
            "Should flag String error type, got: {:?}",
            violations.iter().map(|v| &v.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_typescript_any() {
        let content = "const x: any = 5;";
        let violations = TasteCheck::check(content, "test.ts", &Language::TypeScript);
        let any_v = violations.iter().find(|v| v.message.contains("any"));
        assert!(any_v.is_some(), "Should flag `any` type: {:?}", violations);
    }

    #[test]
    fn test_typescript_var() {
        let content = "\nvar x = 5;";
        let violations = TasteCheck::check(content, "test.ts", &Language::TypeScript);
        let var_v = violations.iter().find(|v| v.message.contains("var"));
        assert!(var_v.is_some(), "Should flag `var` usage: {:?}", violations);
    }

    #[test]
    fn test_python_bare_except() {
        let content = "try:\n    pass\nexcept:\n    pass";
        let violations = TasteCheck::check(content, "test.py", &Language::Python);
        let except_v = violations.iter().find(|v| v.message.contains("except:"));
        assert!(
            except_v.is_some(),
            "Should flag bare except: {:?}",
            violations
        );
    }

    #[test]
    fn test_python_mutable_default() {
        let content = "def add(item, items=[]):\n    items.append(item)\n    return items";
        let violations = TasteCheck::check(content, "test.py", &Language::Python);
        let default_v = violations
            .iter()
            .find(|v| v.message.contains("Mutable default"));
        assert!(
            default_v.is_some(),
            "Should flag mutable default: {:?}",
            violations
        );
    }

    #[test]
    fn test_typescript_passes_for_clean_code() {
        let content = "const x: number = 5;\nconst y = x?.foo ?? 'bar';";
        let violations = TasteCheck::check(content, "test.ts", &Language::TypeScript);
        assert!(
            violations.is_empty(),
            "Should pass clean TS code: {:?}",
            violations
        );
    }

    #[test]
    fn test_magic_numbers_detected() {
        let content = "const a = 42;\nconst b = 100;\nconst c = 999;\nconst d = 7;\nconst e = 5;";
        let violations = TasteCheck::check(content, "test.ts", &Language::TypeScript);
        let magic_v = violations
            .iter()
            .find(|v| v.message.contains("Magic numbers"));
        assert!(
            magic_v.is_some(),
            "Should flag magic numbers (>3): {:?}",
            violations
        );
    }

    #[test]
    fn test_no_magic_numbers_below_threshold() {
        let content = "const a = 42;\nconst b = 100;\nconst c = 999;";
        let violations = TasteCheck::check(content, "test.ts", &Language::TypeScript);
        let magic_v = violations
            .iter()
            .find(|v| v.message.contains("Magic numbers"));
        assert!(
            magic_v.is_none(),
            "Should not flag magic numbers (<=3): {:?}",
            violations
        );
    }

    #[test]
    fn test_non_null_assertion_precheck_fix() {
        // A .tsx file without non-null assertion should NOT trigger the check
        let content = "const x: number = 5;\nconst y = x?.foo ?? 'bar';";
        let violations = TasteCheck::check(content, "test.tsx", &Language::TypeScriptReact);
        let nn_v = violations
            .iter()
            .find(|v| v.message.contains("Non-null assertion"));
        assert!(
            nn_v.is_none(),
            "Should not flag non-null assertion when absent in .tsx: {:?}",
            violations
        );
    }

    #[test]
    fn test_non_null_assertion_fires_in_tsx() {
        let content = "const val = obj!.property;";
        let violations = TasteCheck::check(content, "test.tsx", &Language::TypeScriptReact);
        let nn_v = violations
            .iter()
            .find(|v| v.message.contains("Non-null assertion"));
        assert!(
            nn_v.is_some(),
            "Should flag non-null assertion in .tsx: {:?}",
            violations
        );
    }
}
