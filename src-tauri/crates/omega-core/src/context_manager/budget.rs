//! Per-section token budget allocation for the context window.

/// How the model context window is split across sections.
///
/// Sections mirror the proposal's budget tree; allocations are proportional
/// to the provider's window so they scale across models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenBudget {
    /// Full model context window (input + output), provider-dependent.
    pub total: u32,
    /// System prompt allocation.
    pub system: u32,
    /// Ranked repo-map allocation.
    pub repo_map: u32,
    /// JIT memory retrieval allocation.
    pub memory: u32,
    /// Conversation history (compacted when it overflows).
    pub history: u32,
    /// Reserved for model output — never consumed by input sections.
    pub output_reserve: u32,
    /// Fraction of the input-side budget that triggers compaction.
    pub trigger_ratio: f64,
}

impl TokenBudget {
    /// Allocate a budget from a provider window using the proposal's ratios.
    ///
    /// Fractions of the window (minus an output reserve) rather than fixed
    /// counts, so the same code scales from 8k local models to 1M-token
    /// Gemini windows.
    pub fn from_window(total: u64) -> Self {
        let total = total.max(4096) as u32;
        let output_reserve = match total {
            t if t >= 200_000 => 16_384,
            t if t >= 32_000 => 8_192,
            _ => 4_096,
        };
        let input_budget = total.saturating_sub(output_reserve);
        let system = (input_budget as f64 * 0.05) as u32;
        let repo_map = (input_budget as f64 * 0.08) as u32;
        let memory = (input_budget as f64 * 0.06) as u32;
        let history = input_budget
            .saturating_sub(system)
            .saturating_sub(repo_map)
            .saturating_sub(memory);
        Self {
            total,
            system,
            repo_map,
            memory,
            history,
            output_reserve,
            trigger_ratio: 0.75,
        }
    }

    /// Derive a reduced budget for a subagent context (P2 compatibility).
    ///
    /// Scales every section by `reduction` (0.0–1.0) against the parent's
    /// total, keeping the same relative shape.
    pub fn for_subagent(parent: &Self, reduction: f64) -> Self {
        let reduction = reduction.clamp(0.05, 1.0);
        let total = ((parent.total as f64) * reduction) as u32;
        let mut scaled = Self::from_window(total as u64);
        scaled.trigger_ratio = parent.trigger_ratio;
        scaled
    }

    /// Input-side capacity (everything except the output reserve).
    pub fn input_budget(&self) -> u32 {
        self.total.saturating_sub(self.output_reserve)
    }

    /// Token count at which compaction triggers.
    pub fn trigger_threshold(&self) -> u32 {
        ((self.input_budget() as f64) * self.trigger_ratio) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_fit_within_window() {
        let b = TokenBudget::from_window(128_000);
        assert_eq!(b.total, 128_000);
        let consumed = b.system + b.repo_map + b.memory + b.history + b.output_reserve;
        assert!(
            consumed <= b.total,
            "sections exceed window: {} > {}",
            consumed,
            b.total
        );
        assert!(b.repo_map >= 1024, "repo map too small: {}", b.repo_map);
    }

    #[test]
    fn output_reserve_scales_with_window() {
        assert_eq!(TokenBudget::from_window(8_000).output_reserve, 4_096);
        assert_eq!(TokenBudget::from_window(128_000).output_reserve, 8_192);
        assert_eq!(TokenBudget::from_window(1_000_000).output_reserve, 16_384);
    }

    #[test]
    fn trigger_threshold_is_below_input_budget() {
        let b = TokenBudget::from_window(32_000);
        assert!(b.trigger_threshold() < b.input_budget());
    }

    #[test]
    fn subagent_budget_is_smaller() {
        let parent = TokenBudget::from_window(128_000);
        let child = TokenBudget::for_subagent(&parent, 0.25);
        assert!(child.total < parent.total);
        assert_eq!(child.trigger_ratio, parent.trigger_ratio);
    }
}
