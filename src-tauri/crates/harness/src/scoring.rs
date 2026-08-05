use super::GateResult;
use super::Violation;
use super::ViolationCategory;

const BASE_SCORE: u32 = 100;
const STRUCTURAL_PENALTY: u32 = 15;
const TASTE_PENALTY: u32 = 10;
const GOLDEN_PENALTY: u32 = 20;
const REPEATED_PENALTY: u32 = 25;
const EXTERNAL_PENALTY: u32 = 20;

pub fn calculate_score(violations: &[Violation]) -> GateResult {
    let mut score = BASE_SCORE;

    for v in violations {
        score = match v.category {
            ViolationCategory::Structural => score.saturating_sub(STRUCTURAL_PENALTY),
            ViolationCategory::Taste => score.saturating_sub(TASTE_PENALTY),
            ViolationCategory::Golden => score.saturating_sub(GOLDEN_PENALTY),
            ViolationCategory::Repeated => score.saturating_sub(REPEATED_PENALTY),
            ViolationCategory::External => score.saturating_sub(EXTERNAL_PENALTY),
        };
    }

    GateResult::evaluate(score, violations.to_vec())
}
