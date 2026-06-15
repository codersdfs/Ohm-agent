use crate::GateResult;
use crate::Violation;
use crate::ViolationCategory;

const BASE_SCORE: u32 = 100;
const STRUCTURAL_PENALTY: u32 = 15;
const TASTE_PENALTY: u32 = 10;
const GOLDEN_PENALTY: u32 = 20;
const REPEATED_PENALTY: u32 = 25;
const PASS_THRESHOLD: u32 = 80;

pub fn calculate_score(violations: &[Violation]) -> GateResult {
    let mut score = BASE_SCORE;

    for v in violations {
        score = match v.category {
            ViolationCategory::Structural => score.saturating_sub(STRUCTURAL_PENALTY),
            ViolationCategory::Taste => score.saturating_sub(TASTE_PENALTY),
            ViolationCategory::Golden => score.saturating_sub(GOLDEN_PENALTY),
            ViolationCategory::Repeated => score.saturating_sub(REPEATED_PENALTY),
        };
    }

    GateResult::fail(score, violations.to_vec())
}
