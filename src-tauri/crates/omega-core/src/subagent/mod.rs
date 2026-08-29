//! Subagent delegation system.
//!
//! Subagents fork from a parent session, run with their own isolated context
//! window (reduced token budget), and return a condensed summary to the parent.
//! Sequential delegation only — one subagent at a time.
//!
//! See `plans/p2-subagent-delegation/map.md` for design decisions.

pub mod config;
pub mod result;
pub mod subagent;

pub use config::ContextForkMode;
pub use config::SubagentConfig;
pub use result::RunOutcome;
pub use result::SubagentResult;
pub use subagent::spawn_subagent;
pub use subagent::Subagent;
