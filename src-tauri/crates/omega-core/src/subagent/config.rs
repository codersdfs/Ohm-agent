//! Subagent configuration.

/// Context fork strategy — how much parent context to copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextForkMode {
    /// Copy entire parent context; system prompt swapped.
    Full,
    /// Task-relevant messages only (Cognition's recommendation).
    TaskScoped,
    /// None — task description only.
    CleanSlate,
}

/// Configuration for a single subagent spawn.
#[derive(Debug, Clone)]
pub struct SubagentConfig {
    /// How much parent context to copy.
    pub context_mode: ContextForkMode,
    /// Token budget cap for subagent (delta from fork point).
    pub token_budget: u64,
    /// Max turns before budget-capped stop.
    pub max_turns: u32,
    /// Allowed tool names; empty = read-only.
    pub tool_whitelist: Vec<String>,
    /// Deliverable description for the summary template.
    pub deliverable: String,
    /// Task message appended after forked context.
    pub task: String,
}

impl SubagentConfig {
    /// Defaults for `Full` fork mode.
    pub fn from_mode(mode: ContextForkMode) -> Self {
        match mode {
            ContextForkMode::Full => Self {
                context_mode: mode,
                token_budget: 30_000,
                max_turns: 10,
                tool_whitelist: Vec::new(),
                deliverable: "condensed summary with structured outcome line".into(),
                task: String::new(),
            },
            ContextForkMode::TaskScoped => Self {
                context_mode: mode,
                token_budget: 20_000,
                max_turns: 8,
                tool_whitelist: Vec::new(),
                deliverable: "condensed summary".into(),
                task: String::new(),
            },
            ContextForkMode::CleanSlate => Self {
                context_mode: mode,
                token_budget: 10_000,
                max_turns: 5,
                tool_whitelist: Vec::new(),
                deliverable: "condensed summary".into(),
                task: String::new(),
            },
        }
    }

    /// Set the task to delegate.
    pub fn with_task(mut self, task: impl Into<String>) -> Self {
        self.task = task.into();
        self
    }

    /// Set allowed tools; empty = read-only.
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tool_whitelist = tools;
        self
    }

    /// Returns true if write tools are allowed.
    pub fn allows_writes(&self) -> bool {
        !self.tool_whitelist.is_empty()
    }
}

impl Default for SubagentConfig {
    fn default() -> Self {
        Self::from_mode(ContextForkMode::Full)
    }
}
