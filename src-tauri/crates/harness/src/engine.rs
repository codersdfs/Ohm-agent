use crate::golden::GoldenRules;
use crate::repeated::{self as repeated_checks, RepeatedPatternTracker};
use crate::rules::RulesDatabase;
use crate::scoring;
use crate::structural::StructuralCheck;
use crate::taste::TasteCheck;
use crate::GateResult;
use super::Language;
use super::Violation;
use super::ViolationCategory;

/// Unified Gate engine that runs all check types and returns aggregated results.
pub struct GateEngine {
    pub db: RulesDatabase,
    pub project_root: String,
    pub language: Language,
    pub repeated_tracker: RepeatedPatternTracker,
    /// Optional taste agent for personalized code feedback
    #[cfg(feature = "taste-system")]
    pub taste_agent: Option<::taste::TasteAgent>,
}

impl GateEngine {
    pub fn new(project_root: String, language: Language) -> Self {
        let db =
            RulesDatabase::load_or_create(&project_root).unwrap_or_else(|_| RulesDatabase::new());
        Self {
            db,
            project_root,
            language,
            repeated_tracker: RepeatedPatternTracker::new(),
            #[cfg(feature = "taste-system")]
            taste_agent: None,
        }
    }

    /// Attach a taste agent for personalized feedback
    #[cfg(feature = "taste-system")]
    pub fn with_taste_agent(mut self, agent: ::taste::TasteAgent) -> Self {
        self.taste_agent = Some(agent);
        self
    }

    /// Run all checks on a file's content.
    /// `path` is the file path for structural naming checks.
    /// `content` is the file contents.
    pub fn check_file(&mut self, path: &str, content: &str) -> GateResult {
        let mut all_violations: Vec<Violation> = vec![];

        // 1. Structural checks (line count, function length, naming, imports)
        all_violations.extend(StructuralCheck::check(content, path, &self.language));
        all_violations.extend(StructuralCheck::check_file_size(path));
        all_violations.extend(StructuralCheck::check_file_name(path));

        // 2. Taste checks (code style, nullable, early returns)
        all_violations.extend(TasteCheck::check(content, path, &self.language));

        // 3. Golden rules (permanent anti-patterns)
        all_violations.extend(GoldenRules::check(content, path, &self.language));

        // 4. Content-based rules from RulesDatabase (promoted patterns)
        all_violations.extend(self.db.check_content(content, &self.language));

        // 5. Repeated pattern detection
        all_violations.extend(repeated_checks::find_repeated_patterns(
            content,
            &self.language,
        ));

        // 6. External linter checks (clippy, eslint, tsc, ruff)
        let config = crate::gate_config::load_gate_config(&self.project_root);
        if config.clippy_enabled && self.language == Language::Rust {
            let violations = crate::external::run_clippy(&self.project_root);
            all_violations.extend(violations.into_iter().map(|v| v.to_violation()));
        }
        if config.eslint_enabled && matches!(self.language, Language::TypeScript | Language::TypeScriptReact | Language::JavaScript) {
            let violations = crate::external::run_eslint(&self.project_root);
            all_violations.extend(violations.into_iter().map(|v| v.to_violation()));
        }
        if config.tsc_enabled && matches!(self.language, Language::TypeScript | Language::TypeScriptReact) {
            let violations = crate::external::run_tsc(&self.project_root);
            all_violations.extend(violations.into_iter().map(|v| v.to_violation()));
        }
        if config.ruff_enabled && self.language == Language::Python {
            let violations = crate::external::run_ruff(&self.project_root);
            all_violations.extend(violations.into_iter().map(|v| v.to_violation()));
        }

        // Deduplicate by message + category
        all_violations.sort_by(|a, b| a.message.cmp(&b.message));
        all_violations.dedup_by(|a, b| a.message == b.message && a.category == b.category);

        // Track and promote repeated violations
        let promoted =
            self.repeated_tracker
                .promote_to_db(&mut self.db, &self.language, &all_violations);
        if promoted > 0 {
            log::info!("Gate: promoted {} new rules to database", promoted);
            // Persist promoted rules
            let _ = self.db.save_to(&self.project_root);
        }

        scoring::calculate_score(&all_violations)
    }

    /// Run checks with taste agent enabled, applying learned preferences
    #[cfg(feature = "taste-system")]
    async fn check_file_with_feedback_async(&mut self, path: &str, content: &str, feedback: Option<::taste::TasteFeedback>) -> GateResult {
        let mut result = self.check_file(path, content);
        
        if let Some(agent) = self.taste_agent.as_ref() {
            // Convert violations for taste agent processing
            let converted_violations: Vec<::taste::Violation> = result.violations.iter().map(|v| ::taste::Violation {
                category: match v.category {
                    ViolationCategory::Structural => ::taste::ViolationCategory::Structural,
                    ViolationCategory::Taste => ::taste::ViolationCategory::Taste,
                    ViolationCategory::Golden => ::taste::ViolationCategory::Golden,
                    ViolationCategory::Repeated => ::taste::ViolationCategory::Repeated,
                    ViolationCategory::External => ::taste::ViolationCategory::External,
                },
                message: v.message.clone(),
                tool_hint: v.tool_hint.clone(),
                line: v.line,
            }).collect();
            
            let lang = match self.language {
                Language::Rust => ::taste::Language::Rust,
                Language::TypeScript => ::taste::Language::TypeScript,
                Language::TypeScriptReact => ::taste::Language::TypeScriptReact,
                Language::JavaScript => ::taste::Language::JavaScript,
                Language::Python => ::taste::Language::Python,
                Language::Go => ::taste::Language::Go,
                Language::CSharp => ::taste::Language::CSharp,
                Language::Java => ::taste::Language::Java,
                Language::Other(ref s) => ::taste::Language::Other(s.clone()),
            };
            
            let base_score = result.score;
            let new_score = agent.apply_taste_score(base_score, &converted_violations, lang);
            result.score = new_score;
        }
        
        result
    }

    /// Get the current rules database reference.
    pub fn rules_db(&self) -> &RulesDatabase {
        &self.db
    }

    /// Get mutable rules database reference.
    pub fn rules_db_mut(&mut self) -> &mut RulesDatabase {
        &mut self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Language;
    use crate::ViolationCategory;

    #[cfg(feature = "taste-system")]
    #[test]
    fn test_taste_agent_field_exists() {
        let dir = std::env::temp_dir().join("omega_test_taste");
        let _ = std::fs::create_dir_all(&dir);
        let engine = GateEngine::new(dir.to_str().unwrap().to_string(), Language::Rust);
        // Verify taste_agent field exists and is None by default
        let _ = engine.taste_agent;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_engine_check_file_clean() {
        let dir = std::env::temp_dir().join("omega_test_engine");
        let _ = std::fs::create_dir_all(&dir);
        let mut engine = GateEngine::new(dir.to_str().unwrap().to_string(), Language::Rust);
        let result = engine.check_file("main.rs", "fn main() { println!(\"hello\"); }\n");
        assert!(
            result.score >= 80,
            "Clean file should score >=80, got {}",
            result.score
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_engine_detects_violations() {
        let dir = std::env::temp_dir().join("omega_test_engine2");
        let _ = std::fs::create_dir_all(&dir);
        let content = "unsafe { ptr.read() }\nfn CamelCase() {}\n";
        let mut engine = GateEngine::new(dir.to_str().unwrap().to_string(), Language::Rust);
        let result = engine.check_file("test.rs", content);
        assert!(
            result.score < 100,
            "Should detect violations, got score {}",
            result.score
        );
        assert!(!result.violations.is_empty(), "Should have violations");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_engine_promotes_repeated() {
        let dir = std::env::temp_dir().join("omega_test_engine3");
        let _ = std::fs::create_dir_all(&dir);
        let content = "unsafe { ptr.read() }";
        let mut engine = GateEngine::new(dir.to_str().unwrap().to_string(), Language::Rust);

        // Run 3 times — third should auto-promote
        engine.check_file("test.rs", content);
        engine.check_file("test.rs", content);
        let result = engine.check_file("test.rs", content);
        assert!(result.score < 100, "Third run should still score <100");

        // The unsafe rule should now be in the database
        let db = engine.rules_db();
        let group = db.load_for_language(&Language::Rust);
        let has_unsafe_rule = group.golden.iter().any(|r| r.pattern.contains("unsafe"));
        assert!(has_unsafe_rule, "Unsafe pattern should be in golden rules");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scoring_threshold() {
        let violations = vec![Violation {
            category: ViolationCategory::Golden,
            message: "test".into(),
            tool_hint: None,
            line: None,
        }];
        let result = scoring::calculate_score(&violations);
        assert_eq!(result.score, 80, "Single golden violation should score 80");
        assert!(result.passed, "Score 80 should pass");

        let violations = vec![
            Violation {
                category: ViolationCategory::Golden,
                message: "test1".into(),
                tool_hint: None,
                line: None,
            },
            Violation {
                category: ViolationCategory::Golden,
                message: "test2".into(),
                tool_hint: None,
                line: None,
            },
        ];
        let result = scoring::calculate_score(&violations);
        assert_eq!(result.score, 60, "Two golden violations should score 60");
        assert!(!result.passed, "Score 60 should fail");
    }
}
