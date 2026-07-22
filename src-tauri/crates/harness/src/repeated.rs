use crate::rules::RulesDatabase;
use crate::Language;
use std::collections::HashMap;

/// Tracks repeated pattern occurrences and auto-promotes at frequency ≥ 3.
pub struct RepeatedPatternTracker {
    /// Map of (language_key, pattern) -> was already promoted
    already_promoted: std::collections::HashSet<(String, String)>,
}

impl RepeatedPatternTracker {
    pub fn new() -> Self {
        Self {
            already_promoted: std::collections::HashSet::new(),
        }
    }

    /// Promote violations into the rules database.
    /// Returns the count of rules newly promoted.
    pub fn promote_to_db(
        &mut self,
        db: &mut RulesDatabase,
        lang: &Language,
        violations: &[crate::Violation],
    ) -> u32 {
        let mut promoted = 0u32;

        for v in violations {
            let cat = format!("{:?}", v.category).to_lowercase();
            let pattern = v
                .message
                .rsplit(": ")
                .next()
                .unwrap_or(&v.message)
                .to_string();
            let key = (lang.to_key(), pattern.clone());

            // Always promote_or_increment — this is an idempotent database operation
            let was_promoted = db.is_pattern_promoted(lang, &pattern);
            db.promote_or_increment(lang, &cat, &pattern, &v.message, "error");
            let now_promoted = db.is_pattern_promoted(lang, &pattern);

            // Count only the first time this pattern transitions from not promoted to promoted
            if now_promoted && !was_promoted && !self.already_promoted.contains(&key) {
                promoted += 1;
                self.already_promoted.insert(key);
            }
        }

        promoted
    }

    pub fn clear(&mut self) {
        self.already_promoted.clear();
    }
}

/// Scans content for repeated code blocks (same pattern appearing 3+ times).
pub fn find_repeated_patterns(content: &str, lang: &Language) -> Vec<crate::Violation> {
    let mut violations = vec![];

    match lang {
        Language::Rust => {
            // Repeated `impl` blocks for same type
            if let Ok(re) = regex::Regex::new(r"impl\s+(\w+)\s") {
                let mut impls: HashMap<String, u32> = HashMap::new();
                for cap in re.captures_iter(content) {
                    if let Some(name) = cap.get(1) {
                        *impls.entry(name.as_str().to_string()).or_insert(0) += 1;
                    }
                }
                for (name, count) in &impls {
                    if *count >= 4 {
                        violations.push(crate::Violation {
                            category: crate::ViolationCategory::Repeated,
                            message: format!(
                                "Type `{}` has {} impl blocks: consider combining or using macros",
                                name, count
                            ),
                            tool_hint: Some(
                                "Merge related impl blocks or use derive macros".into(),
                            ),
                            line: None,
                        });
                    }
                }
            }

            // Repeated error variants (heuristic: many => in a derive(Error) context)
            if content.contains("#[derive(Error)]") || content.contains("#[error(") {
                let variant_count = content.matches("=>").count();
                if variant_count > 8 {
                    violations.push(crate::Violation {
                        category: crate::ViolationCategory::Repeated,
                        message: format!(
                            "Error enum has {} variants: consider grouping into sub-enums",
                            variant_count + 1
                        ),
                        tool_hint: Some("Split large error enums into nested enums".into()),
                        line: None,
                    });
                }
            }
        }
        Language::TypeScript | Language::TypeScriptReact => {
            // Repeated `interface` declarations with overlapping fields
            if let Ok(re) = regex::Regex::new(r"\binterface\s+(\w+)") {
                let count = re.find_iter(content).count();
                if count > 6 {
                    violations.push(crate::Violation {
                        category: crate::ViolationCategory::Repeated,
                        message: format!(
                            "{} interface declarations: consider extracting shared types",
                            count
                        ),
                        tool_hint: Some(
                            "Use `type` intersections or base interfaces to reduce duplication"
                                .into(),
                        ),
                        line: None,
                    });
                }
            }

            // Repeated conditional chains (if/else if > 5)
            let if_count = content.matches("} else if ").count();
            if if_count >= 5 {
                violations.push(crate::Violation {
                    category: crate::ViolationCategory::Repeated,
                    message: format!("Long if-else chain ({} branches): consider switch, map, or pattern matching", if_count + 1),
                    tool_hint: Some("Replace with `switch`/`match` or a lookup map".into()),
                    line: None,
                });
            }
        }
        Language::Python => {
            // Repeated try/except blocks
            let try_count = content.matches("try:").count();
            if try_count > 5 {
                violations.push(crate::Violation {
                    category: crate::ViolationCategory::Repeated,
                    message: format!(
                        "{} try/except blocks: consider a context manager or wrapper",
                        try_count
                    ),
                    tool_hint: Some(
                        "Extract error handling into a decorator or context manager".into(),
                    ),
                    line: None,
                });
            }
        }
        _ => {}
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RulesDatabase;
    use crate::Language;

    #[test]
    fn test_repeated_impl_blocks() {
        let content = "impl Foo {}\nimpl Foo {}\nimpl Foo {}\nimpl Foo {}\nimpl Foo {}";
        let violations = find_repeated_patterns(content, &Language::Rust);
        let impl_v = violations
            .iter()
            .find(|v| v.message.contains("impl blocks"));
        assert!(
            impl_v.is_some(),
            "Should flag repeated impl blocks: {:?}",
            violations
        );
    }

    #[test]
    fn test_long_if_else_chain() {
        // `} else if ` on same lines so `.matches("} else if ")` works
        let content =
            "if (a) {} else if (b) {} else if (c) {} else if (d) {} else if (e) {} else if (f) {}";
        let violations = find_repeated_patterns(content, &Language::TypeScript);
        let if_v = violations.iter().find(|v| v.message.contains("if-else"));
        assert!(if_v.is_some(), "Should flag long if-else chain");
    }

    #[test]
    fn test_repeated_try_except() {
        let content = "try:\n    pass\ntry:\n    pass\ntry:\n    pass\ntry:\n    pass\ntry:\n    pass\ntry:\n    pass";
        let violations = find_repeated_patterns(content, &Language::Python);
        let try_v = violations.iter().find(|v| v.message.contains("try/except"));
        assert!(
            try_v.is_some(),
            "Should flag repeated try blocks: {:?}",
            violations
        );
    }

    #[test]
    fn test_tracker_promotion() {
        let mut db = RulesDatabase::new();
        let mut tracker = RepeatedPatternTracker::new();
        let lang = Language::Rust;

        let violation = crate::Violation {
            category: crate::ViolationCategory::Repeated,
            message: "repeated: bad_anti_pattern".into(),
            tool_hint: None,
            line: None,
        };

        // First occurrence — extracted pattern is "bad_anti_pattern"
        let p1 = tracker.promote_to_db(&mut db, &lang, &[violation.clone()]);
        assert_eq!(p1, 0, "Should not promote on first occurrence");

        let check1 = db.check_content("clean code", &lang);
        assert!(check1.is_empty(), "Should not enforce at frequency 1");

        // Second occurrence
        let p2 = tracker.promote_to_db(&mut db, &lang, &[violation.clone()]);
        assert_eq!(p2, 0, "Should not promote on second occurrence");

        // Third occurrence
        let p3 = tracker.promote_to_db(&mut db, &lang, &[violation.clone()]);
        assert_eq!(p3, 1, "Should promote on third occurrence");

        // Now the rule should be enforced — content containing "bad_anti_pattern"
        let check3 = db.check_content("bad_anti_pattern", &lang);
        assert!(!check3.is_empty(), "Should enforce promoted rule");
    }

    #[test]
    fn test_clean_code_no_repeated_violations() {
        let content = "fn foo() {}\nfn bar() {}";
        let violations = find_repeated_patterns(content, &Language::Rust);
        assert!(
            violations.is_empty(),
            "Should not flag clean code: {:?}",
            violations
        );
    }
}
