// Tool execution context

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use std::collections::HashMap;

/// LRU cache entry for file state tracking
#[derive(Debug, Clone)]
pub struct FileStateEntry {
    pub path: String,
    pub content: String,
    pub last_access: chrono::DateTime<chrono::Utc>,
}

/// UI callbacks for interactive prompts
pub type PromptCallback = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// The execution context passed to tools
pub struct ToolUseContext {
    /// Cancellation token for abort signals
    pub abort_token: Option<CancellationToken>,
    /// LRU cache of recently read files
    pub read_file_state: HashMap<String, FileStateEntry>,
    /// Conversation history
    pub conversation_history: Vec<providers::ChatMessage>,
    /// UI callbacks for interactive prompts
    pub prompt_callback: Option<PromptCallback>,
    /// Agent identifier
    pub agent_id: String,
    /// Optional gate check callback (for omega-core integration)
    pub gate_check_fn: Option<Arc<dyn Fn(&str) -> GateCheckResult + Send + Sync>>,
}

impl ToolUseContext {
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            abort_token: None,
            read_file_state: HashMap::new(),
            conversation_history: Vec::new(),
            prompt_callback: None,
            agent_id: agent_id.into(),
            gate_check_fn: None,
        }
    }

    pub fn with_abort_token(mut self, token: CancellationToken) -> Self {
        self.abort_token = Some(token);
        self
    }

    pub fn with_prompt_callback(mut self, cb: PromptCallback) -> Self {
        self.prompt_callback = Some(cb);
        self
    }

    pub fn with_gate_check_fn(mut self, cb: Arc<dyn Fn(&str) -> GateCheckResult + Send + Sync>) -> Self {
        self.gate_check_fn = Some(cb);
        self
    }

    pub fn clone_for_subagent(&self, new_agent_id: impl Into<String>) -> Self {
        Self {
            abort_token: self.abort_token.clone(),
            read_file_state: HashMap::new(), // Fresh cache for subagent
            conversation_history: self.conversation_history.clone(),
            prompt_callback: self.prompt_callback.clone(),
            agent_id: new_agent_id.into(),
            gate_check_fn: self.gate_check_fn.clone(),
        }
    }
}

use crate::GateCheckResult;