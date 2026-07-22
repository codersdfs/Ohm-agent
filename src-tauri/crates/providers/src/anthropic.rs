use crate::{
    ChatRequest, ChatResponse, LlmProvider, StreamChunk, ToolCall, ToolCallFunction, Usage,
};
use serde::Serialize;

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Serialize)]
struct AnthropicToolDef {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicToolDef>>,
}

#[derive(serde::Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    input: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    model: String,
    #[allow(dead_code)]
    stop_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct AnthropicStreamDelta {
    #[serde(rename = "type")]
    delta_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    #[allow(dead_code)]
    id: String,
    #[serde(default)]
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    partial_json: String,
}

#[derive(serde::Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    delta: Option<AnthropicStreamDelta>,
    #[serde(default)]
    content_block: Option<AnthropicContentBlock>,
    #[serde(default)]
    message: Option<AnthropicStreamMessage>,
}

#[derive(serde::Deserialize)]
struct AnthropicStreamMessage {
    #[allow(dead_code)]
    model: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(serde::Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com/v1".into()),
        }
    }

    fn convert_messages(request: &ChatRequest) -> Vec<AnthropicMessage> {
        let mut messages: Vec<AnthropicMessage> = Vec::new();

        for msg in &request.messages {
            match msg.role.as_str() {
                "system" => {
                    // Skipped — extracted separately as the request-level `system` field
                }
                "user" => {
                    messages.push(AnthropicMessage {
                        role: "user".into(),
                        content: serde_json::json!([{"type": "text", "text": &msg.content}]),
                    });
                }
                "assistant" => {
                    // Build content blocks: text + tool_calls
                    let mut blocks: Vec<serde_json::Value> = Vec::new();
                    if !msg.content.is_empty() {
                        blocks.push(serde_json::json!({"type": "text", "text": &msg.content}));
                    }
                    if let Some(ref tool_calls) = msg.tool_calls {
                        for tc in tool_calls {
                            let input: serde_json::Value =
                                serde_json::from_str(&tc.function.arguments)
                                    .unwrap_or(serde_json::json!({}));
                            blocks.push(serde_json::json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.function.name,
                                "input": input,
                            }));
                        }
                    }
                    if blocks.is_empty() {
                        blocks.push(serde_json::json!({"type": "text", "text": ""}));
                    }
                    messages.push(AnthropicMessage {
                        role: "assistant".into(),
                        content: serde_json::Value::Array(blocks),
                    });
                }
                "tool" => {
                    // Tool results go as user message with tool_result blocks
                    let tool_result_id = msg.tool_call_id.as_deref().unwrap_or("");
                    messages.push(AnthropicMessage {
                        role: "user".into(),
                        content: serde_json::json!([{
                            "type": "tool_result",
                            "tool_use_id": tool_result_id,
                            "content": &msg.content,
                        }]),
                    });
                }
                _ => {}
            }
        }

        messages
    }

    fn convert_tools(tools: &[crate::ToolDefinition]) -> Vec<AnthropicToolDef> {
        tools
            .iter()
            .map(|t| AnthropicToolDef {
                name: t.function.name.clone(),
                description: t.function.description.clone(),
                input_schema: t.function.parameters.clone(),
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, String> {
        let client = reqwest::Client::new();
        let messages = Self::convert_messages(&request);
        let tools = request.tools.as_ref().map(|t| Self::convert_tools(t));

        let system = request
            .messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
            .filter(|s| !s.is_empty());

        let body = AnthropicRequest {
            model: request.config.model.clone(),
            system,
            messages,
            max_tokens: request.config.max_tokens,
            temperature: request.config.temperature,
            stream: false,
            tools,
        };

        let resp = client
            .post(format!("{}/messages", self.base_url.trim_end_matches('/')))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Anthropic error: {}", text));
        }

        let data: AnthropicResponse = resp
            .json()
            .await
            .map_err(|e| format!("parse failed: {}", e))?;

        let mut text = String::new();
        let mut tool_calls = Vec::new();

        for block in &data.content {
            match block.block_type.as_str() {
                "text" => {
                    text.push_str(&block.text);
                }
                "tool_use" => {
                    let args_str = if block.input.is_object() {
                        serde_json::to_string(&block.input).unwrap_or_default()
                    } else {
                        "{}".to_string()
                    };
                    tool_calls.push(ToolCall {
                        id: block.id.clone(),
                        tool_type: "function".into(),
                        function: ToolCallFunction {
                            name: block.name.clone(),
                            arguments: args_str,
                        },
                    });
                }
                _ => {}
            }
        }

        let tool_calls = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        };

        Ok(ChatResponse {
            content: text,
            model: data.model,
            usage: None,
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
        tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), String> {
        let client = reqwest::Client::new();
        let messages = Self::convert_messages(&request);
        let tools = request.tools.as_ref().map(|t| Self::convert_tools(t));

        let system = request
            .messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
            .filter(|s| !s.is_empty());

        let body = AnthropicRequest {
            model: request.config.model.clone(),
            system,
            messages,
            max_tokens: request.config.max_tokens,
            temperature: request.config.temperature,
            stream: true,
            tools,
        };

        let resp = client
            .post(format!("{}/messages", self.base_url.trim_end_matches('/')))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("stream request failed: {}", e))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Anthropic stream error: {}", text));
        }

        let stream = resp.bytes_stream();
        use futures_util::StreamExt;
        let mut buf = String::new();

        // Track current content block type and tool call state
        let mut current_tool_id = String::new();
        let mut tool_call_index: usize = 0;
        // Map from tool_use_id to (index, accumulated_name, accumulated_args)
        let mut tool_calls_map: std::collections::HashMap<String, (usize, String, String)> =
            std::collections::HashMap::new();
        let mut current_block_type = String::new();

        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("stream read error: {}", e))?;
            let text = String::from_utf8_lossy(&chunk);
            buf.push_str(&text);

            while let Some(line_end) = buf.find('\n') {
                let line = buf[..line_end].trim().to_string();
                buf.drain(..=line_end);

                if line.is_empty() || line.starts_with("event:") {
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                        match event.event_type.as_str() {
                            "content_block_start" => {
                                if let Some(ref block) = event.content_block {
                                    current_block_type = block.block_type.clone();
                                    if block.block_type == "tool_use" {
                                        current_tool_id = block.id.clone();
                                        let idx = tool_call_index;
                                        tool_call_index += 1;
                                        tool_calls_map.insert(
                                            current_tool_id.clone(),
                                            (idx, String::new(), String::new()),
                                        );
                                    }
                                }
                            }
                            "content_block_delta" => {
                                if let Some(ref delta) = event.delta {
                                    match delta.delta_type.as_str() {
                                        "text_delta" => {
                                            let _ = tx.send(StreamChunk {
                                                content: delta.text.clone(),
                                                thinking: String::new(),
                                                done: false,
                                                model: None,
                                                usage: None,
                                                delta_tool_calls: None,
                                            });
                                        }
                                        "input_json_delta" => {
                                            if let Some(entry) =
                                                tool_calls_map.get_mut(&current_tool_id)
                                            {
                                                entry.2.push_str(&delta.partial_json);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            "content_block_stop" => {
                                current_block_type.clear();
                            }
                            "message_delta" => {
                                // Stream ended
                                let usage = event.message.as_ref().and_then(|m| {
                                    m.usage.as_ref().map(|u| Usage {
                                        input_tokens: u.input_tokens,
                                        output_tokens: u.output_tokens,
                                    })
                                });

                                // Convert accumulated tool calls to delta format
                                if !tool_calls_map.is_empty() {
                                    let deltas: Vec<crate::DeltaToolCall> = tool_calls_map
                                        .iter()
                                        .map(|(tool_id, (idx, name, args))| crate::DeltaToolCall {
                                            index: *idx,
                                            id: Some(tool_id.clone()),
                                            tool_type: None,
                                            function: Some(crate::DeltaToolCallFunction {
                                                name: Some(name.clone()),
                                                arguments: Some(args.clone()),
                                            }),
                                        })
                                        .collect();

                                    let _ = tx.send(StreamChunk {
                                        content: String::new(),
                                        thinking: String::new(),
                                        done: true,
                                        model: None,
                                        usage,
                                        delta_tool_calls: Some(deltas),
                                    });
                                } else {
                                    let _ = tx.send(StreamChunk {
                                        content: String::new(),
                                        thinking: String::new(),
                                        done: true,
                                        model: None,
                                        usage,
                                        delta_tool_calls: None,
                                    });
                                }
                                return Ok(());
                            }
                            "message_start" => {
                                // Message started, continue
                            }
                            "ping" => {}
                            "error" => {
                                return Err(format!("Anthropic stream error event"));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Stream ended without message_delta
        let _ = tx.send(StreamChunk {
            content: String::new(),
            thinking: String::new(),
            done: true,
            model: None,
            usage: None,
            delta_tool_calls: None,
        });
        Ok(())
    }
}
