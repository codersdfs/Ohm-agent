use crate::Language;
use crate::Violation;
use crate::ViolationCategory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEntry {
    pub pattern: String,
    pub severity: String,
    pub message: String,
    pub tool_hint: Option<String>,
    pub frequency: u32,
    pub promoted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryGroup {
    pub structural: Vec<RuleEntry>,
    pub taste: Vec<RuleEntry>,
    pub golden: Vec<RuleEntry>,
    pub repeated: Vec<RuleEntry>,
    pub frontend: Vec<RuleEntry>,
    pub backend: Vec<RuleEntry>,
    pub data: Vec<RuleEntry>,
}

impl CategoryGroup {
    pub fn new() -> Self {
        Self {
            structural: vec![],
            taste: vec![],
            golden: vec![],
            repeated: vec![],
            frontend: vec![],
            backend: vec![],
            data: vec![],
        }
    }

    pub fn all_rules(&self) -> Vec<(&RuleEntry, ViolationCategory)> {
        let mut out: Vec<(&RuleEntry, ViolationCategory)> = Vec::new();
        for r in &self.structural {
            out.push((r, ViolationCategory::Structural));
        }
        for r in &self.taste {
            out.push((r, ViolationCategory::Taste));
        }
        for r in &self.golden {
            out.push((r, ViolationCategory::Golden));
        }
        for r in &self.repeated {
            out.push((r, ViolationCategory::Repeated));
        }
        for r in &self.frontend {
            out.push((r, ViolationCategory::Structural));
        }
        for r in &self.backend {
            out.push((r, ViolationCategory::Structural));
        }
        for r in &self.data {
            out.push((r, ViolationCategory::Structural));
        }
        out
    }

    pub fn category_mut(&mut self, cat: &str) -> Option<&mut Vec<RuleEntry>> {
        match cat {
            "structural" => Some(&mut self.structural),
            "taste" => Some(&mut self.taste),
            "golden" => Some(&mut self.golden),
            "repeated" => Some(&mut self.repeated),
            "frontend" => Some(&mut self.frontend),
            "backend" => Some(&mut self.backend),
            "data" => Some(&mut self.data),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesDatabase {
    pub languages: HashMap<String, CategoryGroup>,
}

impl RulesDatabase {
    pub fn new() -> Self {
        let mut db = Self {
            languages: HashMap::new(),
        };
        db.seed_defaults();
        db
    }

    pub fn load_for_language(&self, lang: &Language) -> CategoryGroup {
        let key = lang.to_key();
        self.languages
            .get(&key)
            .cloned()
            .unwrap_or_else(CategoryGroup::new)
    }

    pub fn promote_or_increment(
        &mut self,
        lang: &Language,
        category: &str,
        pattern: &str,
        message: &str,
        severity: &str,
    ) {
        let key = lang.to_key();
        let group = self.languages.entry(key).or_insert_with(CategoryGroup::new);
        if let Some(rules) = group.category_mut(category) {
            if let Some(existing) = rules.iter_mut().find(|r| r.pattern == pattern) {
                existing.frequency += 1;
                if existing.frequency >= 3 {
                    existing.promoted = true;
                }
            } else {
                rules.push(RuleEntry {
                    pattern: pattern.to_string(),
                    severity: severity.to_string(),
                    message: message.to_string(),
                    tool_hint: None,
                    frequency: 1,
                    promoted: false,
                });
            }
        }
    }

    pub fn check_content(&self, content: &str, lang: &Language) -> Vec<Violation> {
        let group = self.load_for_language(lang);
        let mut violations = vec![];
        for (rule, category) in group.all_rules() {
            if !rule.promoted && rule.frequency < 3 {
                continue;
            }
            if content.contains(&rule.pattern) {
                violations.push(Violation {
                    category: category.clone(),
                    message: format!("[{}] {}: {}", rule.severity, rule.message, rule.pattern),
                    tool_hint: rule.tool_hint.clone(),
                    line: None,
                });
            }
        }
        violations
    }

    pub fn is_pattern_promoted(&self, lang: &Language, pattern: &str) -> bool {
        let group = self.load_for_language(lang);
        group
            .all_rules()
            .iter()
            .any(|(r, _)| r.pattern == pattern && r.promoted)
    }

    /// Demote rules that are promoted but have low frequency (stale).
    /// Returns the number of demoted rules.
    pub fn demote_stale_rules(&mut self, lang: &Language) -> usize {
        let key = lang.to_key();
        let mut demoted = 0;
        if let Some(group) = self.languages.get_mut(&key) {
            for rules in [
                &mut group.structural,
                &mut group.taste,
                &mut group.golden,
                &mut group.repeated,
                &mut group.frontend,
                &mut group.backend,
                &mut group.data,
            ] {
                for rule in rules.iter_mut() {
                    // Demote promoted rules that have frequency == 0 (seeded defaults)
                    // or that were promoted but never triggered again
                    if rule.promoted && rule.frequency < 2 {
                        rule.promoted = false;
                        demoted += 1;
                    }
                }
            }
        }
        demoted
    }

    fn seed_defaults(&mut self) {
        let rust = self
            .languages
            .entry("rust".into())
            .or_insert_with(CategoryGroup::new);
        rust.structural.push(RuleEntry {
            pattern: "use std".into(),
            severity: "warn".into(),
            message: "Prefer `use crate` over `use std` in library code".into(),
            tool_hint: Some("Replace with `use crate::...`".into()),
            frequency: 0,
            promoted: true,
        });
        rust.taste.push(RuleEntry {
            pattern: "fn main".into(),
            severity: "warn".into(),
            message: "Library crates should not have `fn main`".into(),
            tool_hint: Some("Remove `fn main` or move to binary crate".into()),
            frequency: 0,
            promoted: true,
        });

        let ts = self
            .languages
            .entry("typescript".into())
            .or_insert_with(CategoryGroup::new);
        ts.frontend.push(RuleEntry {
            pattern: "style=".into(),
            severity: "error".into(),
            message: "Use Tailwind classes, not inline styles".into(),
            tool_hint: Some("Replace `style={{...}}` with Tailwind utility classes".into()),
            frequency: 0,
            promoted: true,
        });
        ts.structural.push(RuleEntry {
            pattern: ": any".into(),
            severity: "warn".into(),
            message: "Avoid `any` type, use `unknown`".into(),
            tool_hint: Some("Replace `any` with `unknown` or a proper type".into()),
            frequency: 0,
            promoted: true,
        });
        ts.golden.push(RuleEntry {
            pattern: "console.log".into(),
            severity: "error".into(),
            message: "Use structured logging, not console.log".into(),
            tool_hint: Some("Replace with `log::info!` or a logger call".into()),
            frequency: 0,
            promoted: true,
        });
    }
}

pub fn check_rules(content: &str, rules: &[(&RuleEntry, ViolationCategory)]) -> Vec<Violation> {
    let mut violations = vec![];
    for (rule, category) in rules {
        if !rule.promoted && rule.frequency < 3 {
            continue;
        }
        if content.contains(&rule.pattern) {
            violations.push(Violation {
                category: category.clone(),
                message: format!("[{}] {}: {}", rule.severity, rule.message, rule.pattern),
                tool_hint: rule.tool_hint.clone(),
                line: None,
            });
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Language;

    #[test]
    fn test_seeded_rules_exist() {
        let db = RulesDatabase::new();
        let rust_rules = db.load_for_language(&Language::Rust);
        assert!(
            !rust_rules.structural.is_empty(),
            "Rust should have seeded structural rules"
        );
        assert!(
            !rust_rules.taste.is_empty(),
            "Rust should have seeded taste rules"
        );
    }

    #[test]
    fn test_content_check_matches() {
        let db = RulesDatabase::new();
        let violations = db.check_content("use std::collections::HashMap;", &Language::Rust);
        assert!(!violations.is_empty(), "Should detect `use std` pattern");
    }

    #[test]
    fn test_content_check_clean_passes() {
        let db = RulesDatabase::new();
        let violations = db.check_content("use crate::utils;", &Language::Rust);
        let std_v = violations.iter().find(|v| v.message.contains("use std"));
        assert!(std_v.is_none(), "Should not flag `use crate`");
    }

    #[test]
    fn test_promote_or_increment() {
        let mut db = RulesDatabase::new();
        let lang = Language::Rust;

        // First occurrence
        db.promote_or_increment(
            &lang,
            "structural",
            "bad_pattern",
            "Avoid bad_pattern",
            "error",
        );
        let v1 = db.check_content("bad_pattern", &lang);
        assert!(v1.is_empty(), "Should not enforce at frequency 1");

        // Second occurrence
        db.promote_or_increment(
            &lang,
            "structural",
            "bad_pattern",
            "Avoid bad_pattern",
            "error",
        );
        let v2 = db.check_content("bad_pattern", &lang);
        assert!(v2.is_empty(), "Should not enforce at frequency 2");

        // Third occurrence — should promote
        db.promote_or_increment(
            &lang,
            "structural",
            "bad_pattern",
            "Avoid bad_pattern",
            "error",
        );
        let v3 = db.check_content("bad_pattern", &lang);
        assert!(
            !v3.is_empty(),
            "Should enforce promoted rule at frequency 3"
        );
    }

    #[test]
    fn test_seeded_ts_rules() {
        let db = RulesDatabase::new();
        let ts_rules = db.load_for_language(&Language::TypeScript);
        assert!(
            !ts_rules.frontend.is_empty(),
            "TypeScript should have seeded frontend rules"
        );
        assert!(
            !ts_rules.golden.is_empty(),
            "TypeScript should have seeded golden rules"
        );
    }

    #[test]
    fn test_ts_inline_style_detected() {
        let db = RulesDatabase::new();
        let violations = db.check_content("style={{ color: 'red' }}", &Language::TypeScript);
        let style_v = violations
            .iter()
            .find(|v| v.message.contains("inline styles"));
        assert!(
            style_v.is_some(),
            "Should detect inline style usage: {:?}",
            violations
        );
    }
}
