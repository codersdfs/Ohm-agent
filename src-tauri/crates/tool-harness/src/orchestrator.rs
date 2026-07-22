// Tool orchestrator — secondary/library helper.
//
// CANONICAL production agent loop lives in:
//   omega_core::commands::chat::stream_message_with_history_cancel
// Prefer that path for CLI/TUI. This orchestrator remains for library-style
// embedding and tests; do not diverge permission/gate/cancel behavior without
// also updating chat.rs.

use crate::{ToolRegistry, ToolUseContext, ExecutionPipeline, ToolRequest};
use providers::{ChatMessage, ToolCall, LlmProvider, ChatRequest, ProviderConfig};

pub struct ToolOrchestrator {
    registry: ToolRegistry,
    pipeline: ExecutionPipeline,
    max_loops: u32,
}

impl ToolOrchestrator {
    pub fn new() -> Self {
        Self {
            registry: ToolRegistry::new(),
            pipeline: ExecutionPipeline::new(),
            max_loops: 20,
        }
    }

    pub fn with_registry(mut self, registry: ToolRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn with_pipeline(mut self, pipeline: ExecutionPipeline) -> Self {
        self.pipeline = pipeline;
        self
    }

    pub fn with_max_loops(mut self, max: u32) -> Self {
        self.max_loops = max;
        self
    }

    /// Run a single turn: send messages to provider, execute any tool calls, return final text
    pub async fn run_turn(
        &self,
        messages: &mut Vec<ChatMessage>,
        provider: &dyn LlmProvider,
        config: &ProviderConfig,
    ) -> Result<String, OrchestratorError> {
        let tools = self.registry.tool_definitions();
        let mut full_response = String::new();
        let mut loops = self.max_loops;

        loop {
            if loops == 0 {
                return Err(OrchestratorError::MaxLoopsExceeded);
            }
            loops -= 1;

            let request = ChatRequest {
                messages: messages.clone(),
                config: config.clone(),
                stream: false,
                tools: Some(tools.clone()),
            };

            let response = provider.chat(request).await
                .map_err(|e| OrchestratorError::ProviderError(e))?;

            if let Some(tool_calls) = response.tool_calls {
                // Add assistant message with tool calls
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                // Execute each tool call
                for tc in &tool_calls {
                    let tool_request = match ToolRequest::from_call(tc.clone()) {
                        Ok(r) => r,
                        Err(e) => {
                            messages.push(ChatMessage {
                                role: "tool".into(),
                                content: format!("Error parsing tool arguments: {}", e),
                                tool_calls: None,
                                tool_call_id: Some(tc.id.clone()),
                                name: Some(tc.function.name.clone()),
                            });
                            continue;
                        }
                    };

                    let ctx = ToolUseContext::new("orchestrator");
                    let input = tool_request.clone().into_input();
                    let (result, _budget_check) = self.pipeline.execute(
                        &tc.function.name,
                        input,
                        &ctx
                    ).await.map_err(|e| OrchestratorError::ToolError(e.message.clone()))?;

                    messages.push(ChatMessage {
                        role: "tool".into(),
                        content: if result.success { result.output } else { result.error.unwrap_or_default() },
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }
                continue;
            }

            // No tool calls - return the content
            if !response.content.is_empty() {
                full_response = response.content;
            }
            return Ok(full_response);
        }
    }

    /// Handle streaming responses
    pub async fn run_turn_stream<E: ChatEmitter>(
        &self,
        messages: &mut Vec<ChatMessage>,
        provider: &dyn LlmProvider,
        config: &ProviderConfig,
        emitter: &E,
    ) -> Result<String, OrchestratorError> {
        let tools = self.registry.tool_definitions();
        let mut full_response = String::new();
        let mut loops = self.max_loops;

        loop {
            if loops == 0 {
                return Err(OrchestratorError::MaxLoopsExceeded);
            }
            loops -= 1;

            // Send request and collect streaming response
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let chat_request = ChatRequest {
                messages: messages.clone(),
                config: config.clone(),
                stream: true,
                tools: Some(tools.clone()),
            };

            provider.chat_stream(chat_request, tx).await
                .map_err(OrchestratorError::ProviderError)?;

            let mut tool_call_deltas: Vec<(usize, String, String, String)> = vec![];
            let mut streaming_text = false;

            while let Some(chunk) = rx.recv().await {
                if !chunk.thinking.is_empty() {
                    emitter.emit_thinking(&chunk.thinking).map_err(OrchestratorError::ProviderError)?;
                }

                if !chunk.content.is_empty() {
                    if !streaming_text {
                        streaming_text = true;
                        emitter.emit_token(&chunk.content).map_err(OrchestratorError::ProviderError)?;
                    }
                    full_response.push_str(&chunk.content);
                }

                if let Some(ref deltas) = chunk.delta_tool_calls {
                    for d in deltas {
                        let pos = tool_call_deltas.iter().position(|(idx, _, _, _)| *idx == d.index);
                        if let Some(p) = pos {
                            let entry = &mut tool_call_deltas[p];
                            if let Some(ref id_val) = d.id {
                                if entry.1.is_empty() { entry.1.push_str(id_val); }
                            }
                            if let Some(ref name) = d.function.as_ref().and_then(|f| f.name.as_ref()) {
                                entry.2.push_str(name);
                            }
                            if let Some(ref args) = d.function.as_ref().and_then(|f| f.arguments.as_ref()) {
                                entry.3.push_str(args);
                            }
                        } else {
                            let mut id_buf = String::new();
                            let mut name_buf = String::new();
                            let mut args_buf = String::new();
                            if let Some(ref id_val) = d.id { id_buf.push_str(id_val); }
                            if let Some(ref n) = d.function.as_ref().and_then(|f| f.name.as_ref()) { name_buf.push_str(n); }
                            if let Some(ref a) = d.function.as_ref().and_then(|f| f.arguments.as_ref()) { args_buf.push_str(a); }
                            tool_call_deltas.push((d.index, id_buf, name_buf, args_buf));
                        }
                    }
                }

                if chunk.done {
                    break;
                }
            }

            if !tool_call_deltas.is_empty() {
                let tool_calls: Vec<ToolCall> = tool_call_deltas.iter().map(|(_idx, id, name, args)| {
                    ToolCall {
                        id: if id.is_empty() { format!("call_{}", _idx) } else { id.clone() },
                        tool_type: "function".into(),
                        function: providers::ToolCallFunction {
                            name: name.clone(),
                            arguments: args.clone(),
                        },
                    }
                }).collect();

                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                for tc in &tool_calls {
                    let tool_request = match ToolRequest::from_call(tc.clone()) {
                        Ok(r) => r,
                        Err(e) => {
                            messages.push(ChatMessage {
                                role: "tool".into(),
                                content: format!("Error parsing arguments for `{}`: {}.\nArguments received: {}",
                                    tc.function.name, e, tc.function.arguments),
                                tool_calls: None,
                                tool_call_id: Some(tc.id.clone()),
                                name: Some(tc.function.name.clone()),
                            });
                            continue;
                        }
                    };

                    let ctx = ToolUseContext::new("orchestrator");
                    let input = tool_request.into_input();
                    let (result, _budget_check) = self.pipeline.execute(
                        &tc.function.name,
                        input,
                        &ctx
                    ).await.map_err(|e| OrchestratorError::ToolError(e.message.clone()))?;

                    messages.push(ChatMessage {
                        role: "tool".into(),
                        content: if result.success { result.output } else { result.error.unwrap_or_default() },
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }
                continue;
            }

            emitter.emit_done(&full_response).map_err(OrchestratorError::ProviderError)?;
            return Ok(full_response);
        }
    }
}

/// Chat emitter trait for streaming output
pub trait ChatEmitter {
    fn emit_token(&self, token: &str) -> Result<(), String>;
    fn emit_done(&self, full_response: &str) -> Result<(), String>;
    fn emit_error(&self, error: &str) -> Result<(), String>;

    /// Called when the model emits a thinking/reasoning token.
    fn emit_thinking(&self, _token: &str) -> Result<(), String> { Ok(()) }
    /// Called when thinking is complete. `full` is the entire thinking text.
    fn emit_thinking_done(&self, _full: &str) -> Result<(), String> { Ok(()) }
    /// Called when a tool call starts. `args` is the JSON arguments string.
    fn emit_tool_call(&self, _name: &str, _args: &str) -> Result<(), String> { Ok(()) }
    /// Called when a tool call completes. `success` and `output` describe the result.
    fn emit_tool_result(&self, _name: &str, _success: bool, _output: &str) -> Result<(), String> { Ok(()) }
}

/// Orchestrator errors
#[derive(Debug, Clone)]
pub enum OrchestratorError {
    MaxLoopsExceeded,
    ProviderError(String),
    ToolError(String),
}

impl std::fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MaxLoopsExceeded => write!(f, "Tool call loop exceeded max iterations"),
            Self::ProviderError(e) => write!(f, "Provider error: {}", e),
            Self::ToolError(e) => write!(f, "Tool error: {}", e),
        }
    }
}

impl std::error::Error for OrchestratorError {}

#[cfg(test)]
mod tests {
    // Tests require a mock provider setup
    // These will be tested via integration tests in omega-core
}