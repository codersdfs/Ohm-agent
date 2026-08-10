// Tool orchestrator — canonical agent loop.
//
// This is the single source of truth for the agent loop. CLI/TUI paths
// (chat.rs) delegate here rather than duplicating the loop. Library-style
// embedding and tests also use this directly.
//
// When adding new features (hooks, context management, subagent support),
// extend THIS struct — do not create parallel loops.

use crate::{ExecutionPipeline, ToolRegistry, ToolRequest, ToolUseContext};
use providers::{ChatMessage, ChatRequest, LlmProvider, ProviderConfig, ToolCall};
use std::collections::HashSet;
use std::sync::Arc;

/// Canonical agent loop. `chat.rs` delegates to this for CLI/TUI paths.
pub struct ToolOrchestrator {
    registry: ToolRegistry,
    pipeline: Arc<ExecutionPipeline>,
    max_loops: u32,
}

impl ToolOrchestrator {
    pub fn new() -> Self {
        Self {
            registry: ToolRegistry::new(),
            pipeline: Arc::new(ExecutionPipeline::new()),
            max_loops: 20,
        }
    }

    pub fn with_registry(mut self, registry: ToolRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn with_pipeline(mut self, pipeline: ExecutionPipeline) -> Self {
        self.pipeline = Arc::new(pipeline);
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

            let response = provider
                .chat(request)
                .await
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

                // Execute tool calls in parallel batches
                let batch_size = config.max_concurrent_tools.max(1);
                let batches: Vec<Vec<_>> = tool_calls
                    .chunks(batch_size)
                    .map(|chunk| chunk.to_vec())
                    .collect();

                for batch in batches {
                    let pipeline = self.pipeline.clone();
                    
                    // Execute batch in parallel
                    let results: Vec<(usize, Result<(String, String), String>)> = {
                        let mut handles = Vec::new();
                        for (i, tc) in batch.iter().enumerate() {
                            let pipeline = pipeline.clone();
                            let tc = tc.clone();
                            let handle = tokio::spawn(async move {
                                let tool_request = match ToolRequest::from_call(tc.clone()) {
                                    Ok(r) => r,
                                    Err(e) => return (i, Err(format!("Error parsing tool arguments: {}", e))),
                                };
                                let input = tool_request.into_input();
                                let ctx = ToolUseContext::new("orchestrator");
                                match pipeline.execute(&tc.function.name, input, &ctx).await {
                                    Ok((result, _budget_check)) => {
                                        let output = if result.success {
                                            result.output
                                        } else {
                                            result.error.unwrap_or_default()
                                        };
                                        (i, Ok((tc.function.name.clone(), output)))
                                    }
                                    Err(e) => (i, Err(e.message)),
                                }
                            });
                            handles.push(handle);
                        }
                        
                        let mut results = Vec::new();
                        for handle in handles {
                            match handle.await {
                                Ok(result) => results.push(result),
                                Err(e) => results.push((usize::MAX, Err(format!("Tool call panicked: {}", e)))),
                            }
                        }
                        results.sort_by_key(|(i, _)| *i);
                        results
                    };

                    // Collect results and push tool messages
                    for (_idx, result) in results {
                        match result {
                            Ok((tool_name, output)) => {
                                messages.push(ChatMessage {
                                    role: "tool".into(),
                                    content: output,
                                    tool_calls: None,
                                    tool_call_id: None,
                                    name: Some(tool_name),
                                });
                            }
                            Err(e) => {
                                messages.push(ChatMessage {
                                    role: "tool".into(),
                                    content: e,
                                    tool_calls: None,
                                    tool_call_id: None,
                                    name: None,
                                });
                            }
                        }
                    }
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

            provider
                .chat_stream(chat_request, tx)
                .await
                .map_err(OrchestratorError::ProviderError)?;

            // Build tool calls by index — O(n) vs O(n²) with position() search.
            // Each index gets its own buffer so concurrent tool calls are tracked independently.
            let mut tool_call_buffers: Vec<(String, String, String)> = vec![]; // (id, name, args)
            let mut seen_tool_call_indices = HashSet::new();

            while let Some(chunk) = rx.recv().await {
                // Flush thinking content
                if !chunk.thinking.is_empty() {
                    emitter
                        .emit_thinking(&chunk.thinking)
                        .map_err(OrchestratorError::ProviderError)?;
                }

                // Flush text content as it arrives
                if !chunk.content.is_empty() {
                    emitter
                        .emit_token(&chunk.content)
                        .map_err(OrchestratorError::ProviderError)?;
                    full_response.push_str(&chunk.content);
                }

                // Accumulate tool-call deltas by index
                if let Some(ref deltas) = chunk.delta_tool_calls {
                    for d in deltas {
                        let idx = d.index;
                        if !seen_tool_call_indices.insert(idx) {
                            // First chunk for this index — allocate a buffer
                            tool_call_buffers.resize_with(idx + 1, || (String::new(), String::new(), String::new()));
                        }
                        let buf = &mut tool_call_buffers[idx];
                        if let Some(ref id_val) = d.id {
                            if buf.0.is_empty() {
                                buf.0.push_str(id_val);
                            }
                        }
                        if let Some(ref name) = d.function.as_ref().and_then(|f| f.name.as_ref()) {
                            buf.1.push_str(name);
                        }
                        if let Some(ref args) = d.function.as_ref().and_then(|f| f.arguments.as_ref()) {
                            buf.2.push_str(args);
                        }
                        // Emit the tool-call name as soon as we know it
                        if !buf.1.is_empty() && buf.2.is_empty() {
                            let _ = emitter.emit_tool_call(&buf.1, &buf.2);
                        }
                    }
                }

                if chunk.done {
                    break;
                }
            }

            if !tool_call_buffers.is_empty() {
                let tool_calls: Vec<ToolCall> = tool_call_buffers
                    .iter()
                    .map(|(id, name, args)| ToolCall {
                        id: if id.is_empty() {
                            // Fallback: unlikely since we set id on first chunk, but safety net
                            format!("call_{}", full_response.len())
                        } else {
                            id.clone()
                        },
                        tool_type: "function".into(),
                        function: providers::ToolCallFunction {
                            name: name.clone(),
                            arguments: args.clone(),
                        },
                    })
                    .collect();

                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: String::new(),
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                let batch_size = config.max_concurrent_tools.max(1);
                let batches: Vec<Vec<_>> = tool_calls
                    .chunks(batch_size)
                    .map(|chunk| chunk.to_vec())
                    .collect();

                for batch in batches {
                    let pipeline = self.pipeline.clone();
                    
                    // Execute batch in parallel
                    let results: Vec<(usize, Result<(String, String, String), String>)> = {
                        let mut handles = Vec::new();
                        for (i, tc) in batch.iter().enumerate() {
                            let pipeline = pipeline.clone();
                            let tc = tc.clone();
                            let handle = tokio::spawn(async move {
                                let tool_request = match ToolRequest::from_call(tc.clone()) {
                                    Ok(r) => r,
                                    Err(e) => {
                                        let err_msg = format!(
                                            "Error parsing arguments for `{}`: {}.\nArguments received: {}",
                                            tc.function.name, e, tc.function.arguments
                                        );
                                        return (i, Err(err_msg));
                                    }
                                };
                                let input = tool_request.into_input();
                                let ctx = ToolUseContext::new("orchestrator");
                                match pipeline.execute(&tc.function.name, input, &ctx).await {
                                    Ok((result, _budget_check)) => {
                                        let output = if result.success {
                                            result.output.clone()
                                        } else {
                                            result.error.unwrap_or_default()
                                        };
                                        (i, Ok((tc.function.name.clone(), output, result.output)))
                                    }
                                    Err(e) => (i, Err(e.message)),
                                }
                            });
                            handles.push(handle);
                        }
                        
                        let mut results = Vec::new();
                        for handle in handles {
                            match handle.await {
                                Ok(result) => results.push(result),
                                Err(e) => results.push((usize::MAX, Err(format!("Tool call panicked: {}", e)))),
                            }
                        }
                        results.sort_by_key(|(i, _)| *i);
                        results
                    };

                    // Collect results and push tool messages
                    for (_idx, result) in results {
                        match result {
                            Ok((tool_name, output, raw_output)) => {
                                messages.push(ChatMessage {
                                    role: "tool".into(),
                                    content: output,
                                    tool_calls: None,
                                    tool_call_id: None,
                                    name: Some(tool_name.clone()),
                                });
                                let _ = emitter.emit_tool_result(&tool_name, true, &raw_output);
                            }
                            Err(e) => {
                                messages.push(ChatMessage {
                                    role: "tool".into(),
                                    content: e.clone(),
                                    tool_calls: None,
                                    tool_call_id: None,
                                    name: None,
                                });
                                // Try to extract tool name from error if possible
                                let tool_name = e.split(':').next()
                                    .and_then(|s| s.split('`').nth(1))
                                    .unwrap_or("unknown");
                                let _ = emitter.emit_tool_result(tool_name, false, &e);
                            }
                        }
                    }
                }
            }

            // No tool calls — signal completion
            emitter
                .emit_done(&full_response)
                .map_err(OrchestratorError::ProviderError)?;
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
    fn emit_thinking(&self, _token: &str) -> Result<(), String> {
        Ok(())
    }
    /// Called when thinking is complete. `full` is the entire thinking text.
    fn emit_thinking_done(&self, _full: &str) -> Result<(), String> {
        Ok(())
    }
    /// Called when a tool call starts. `args` is the JSON arguments string.
    fn emit_tool_call(&self, _name: &str, _args: &str) -> Result<(), String> {
        Ok(())
    }
    /// Called when a tool call completes. `success` and `output` describe the result.
    fn emit_tool_result(&self, _name: &str, _success: bool, _output: &str) -> Result<(), String> {
        Ok(())
    }
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
