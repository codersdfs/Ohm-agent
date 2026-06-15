use crate::Violation;
use crate::ViolationCategory;

pub struct Rule {
    pub name: &'static str,
    pub category: ViolationCategory,
    pub check: fn(&str) -> Vec<Violation>,
}

pub fn default_rules() -> Vec<Rule> {
    vec![]
}

pub fn check_all(content: &str, rules: &[Rule]) -> Vec<Violation> {
    let mut violations = vec![];
    for rule in rules {
        violations.extend((rule.check)(content));
    }
    violations
}
