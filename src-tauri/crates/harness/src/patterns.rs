use crate::Violation;
use crate::ViolationCategory;

#[derive(Debug, Clone)]
pub struct NegativePattern {
    pub pattern: &'static str,
    pub description: &'static str,
    pub category: ViolationCategory,
    pub frequency: u32,
    pub promoted: bool,
}

pub fn check_patterns(content: &str, patterns: &[NegativePattern]) -> Vec<Violation> {
    let mut violations = vec![];
    for pattern in patterns {
        if content.contains(pattern.pattern) {
            violations.push(Violation {
                category: pattern.category.clone(),
                message: format!("Found pattern '{}': {}", pattern.pattern, pattern.description),
                tool_hint: None,
                line: None,
            });
        }
    }
    violations
}
