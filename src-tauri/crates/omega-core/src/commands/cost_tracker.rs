use std::sync::atomic::{AtomicU64, Ordering};

static COST_INPUT: AtomicU64 = AtomicU64::new(0);
static COST_OUTPUT: AtomicU64 = AtomicU64::new(0);
static COST_COUNT: AtomicU64 = AtomicU64::new(0);

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn cost_report() -> String {
    format!(
        "  {}cost: session total — {} in / {} out ({} messages){}",
        DIM,
        COST_INPUT.load(Ordering::Relaxed),
        COST_OUTPUT.load(Ordering::Relaxed),
        COST_COUNT.load(Ordering::Relaxed),
        RESET,
    )
}

pub fn record_cost(input: u32, output: u32) {
    COST_INPUT.fetch_add(input as u64, Ordering::Relaxed);
    COST_OUTPUT.fetch_add(output as u64, Ordering::Relaxed);
    COST_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn session_token_counts() -> (u64, u64) {
    (
        COST_INPUT.load(Ordering::Relaxed),
        COST_OUTPUT.load(Ordering::Relaxed),
    )
}
