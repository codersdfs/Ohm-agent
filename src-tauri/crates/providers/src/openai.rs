use crate::{ChatRequest, ChatResponse, LlmProvider, StreamChunk, ToolCall, Usage};
use serde::Serialize;

#[derive(Serialize, serde::Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(default)]
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Serialize, serde::Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIToolCallFunction,
}

#[derive(Serialize, serde::Deserialize)]
struct OpenAIToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    stream: bool,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAIToolDef>>,
    /// Required for usage on streaming responses (OpenAI + many OpenAI-compatible APIs).
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<OpenAIStreamOptions>,
}

#[derive(Serialize)]
struct OpenAIStreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct OpenAIToolDef {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIToolFunctionDef,
}

#[derive(Serialize)]
struct OpenAIToolFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct OpenAIResponseChoice {
    message: OpenAIMessage,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIResponseChoice>,
    model: String,
    usage: Option<OpenAIUsage>,
}

#[derive(serde::Deserialize)]
struct OpenAIUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

#[derive(serde::Deserialize)]
struct StreamDelta {
    content: Option<String>,
    /// Reasoning/thinking content emitted by o1/o3 and compatible models.
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<StreamDeltaToolCall>>,
}

#[derive(serde::Deserialize)]
struct StreamDeltaToolCall {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    tool_type: Option<String>,
    function: Option<StreamDeltaToolCallFunction>,
}

#[derive(serde::Deserialize)]
struct StreamDeltaToolCallFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(serde::Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct StreamEvent {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    model: Option<String>,
    usage: Option<OpenAIUsage>,
}

pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
}

#[derive(serde::Deserialize)]
struct OpenAIErrorResponse {
    error: OpenAIErrorDetail,
}

#[derive(serde::Deserialize)]
struct OpenAIErrorDetail {
    message: String,
    #[allow(dead_code)]
    r#type: Option<String>,
}

impl OpenAIProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self { api_key, base_url }
    }

    fn build_request(&self, request: &ChatRequest) -> OpenAIRequest {
        let tools = request.tools.as_ref().map(|tools| {
            tools
                .iter()
                .map(|t| OpenAIToolDef {
                    tool_type: t.tool_type.clone(),
                    function: OpenAIToolFunctionDef {
                        name: t.function.name.clone(),
                        description: t.function.description.clone(),
                        parameters: t.function.parameters.clone(),
                    },
                })
                .collect()
        });

        OpenAIRequest {
            model: request.config.model.clone(),
            messages: request
                .messages
                .iter()
                .map(|m| {
                    let tool_calls = m.tool_calls.as_ref().map(|calls| {
                        calls
                            .iter()
                            .map(|tc| OpenAIToolCall {
                                id: tc.id.clone(),
                                tool_type: tc.tool_type.clone(),
                                function: OpenAIToolCallFunction {
                                    name: tc.function.name.clone(),
                                    arguments: tc.function.arguments.clone(),
                                },
                            })
                            .collect()
                    });
                    OpenAIMessage {
                        role: m.role.clone(),
                        content: m.content.clone(),
                        tool_calls,
                        tool_call_id: m.tool_call_id.clone(),
                        name: m.name.clone(),
                    }
                })
                .collect(),
            stream: request.stream,
            max_tokens: request.config.max_tokens,
            temperature: request.config.temperature,
            tools,
            stream_options: if request.stream {
                Some(OpenAIStreamOptions {
                    include_usage: true,
                })
            } else {
                None
            },
        }
    }

    fn url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAIProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, String> {
        let client = reqwest::Client::new();
        let body = self.build_request(&request);

        let resp = client
            .post(self.url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("API error {}: {}", status, text));
        }

        let body_bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("failed to read response body: {}", e))?;

        // Some providers (OpenRouter, etc.) return 200 with an error body
        if let Ok(err_resp) = serde_json::from_slice::<OpenAIErrorResponse>(&body_bytes) {
            return Err(format!("API error: {}", err_resp.error.message));
        }

        let data: OpenAIResponse = serde_json::from_slice(&body_bytes).map_err(|e| {
            let preview = String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(1000)]);
            format!("parse failed: {} (body: {}...)", e, preview)
        })?;

        let choice = data
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| "no choices returned".to_string())?;

        let tool_calls = choice.message.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|tc| ToolCall {
                    id: tc.id,
                    tool_type: tc.tool_type,
                    function: crate::ToolCallFunction {
                        name: tc.function.name,
                        arguments: tc.function.arguments,
                    },
                })
                .collect()
        });

        Ok(ChatResponse {
            content: choice.message.content,
            model: data.model,
            usage: data.usage.map(|u| Usage {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
            }),
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
        tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), String> {
        let client = reqwest::Client::new();
        let body = self.build_request(&request);

        let resp = client
            .post(self.url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("stream request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("stream API error {}: {}", status, text));
        }

        let stream = resp.bytes_stream();
        use futures_util::StreamExt;
        let mut buf = String::new();
        let mut last_usage: Option<Usage> = None;
        let mut last_model: Option<String> = None;

        tokio::pin!(stream);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("stream read error: {}", e))?;
            let text = String::from_utf8_lossy(&chunk);
            buf.push_str(&text);

            while let Some(line_end) = buf.find('\n') {
                let line = buf[..line_end].trim().to_string();
                buf.drain(..=line_end);

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if line == "data: [DONE]" {
                    let _ = tx.send(StreamChunk {
                        content: String::new(),
                        thinking: String::new(),
                        done: true,
                        model: last_model.clone(),
                        usage: last_usage.clone(),
                        delta_tool_calls: None,
                    });
                    return Ok(());
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                        if let Some(u) = event.usage.as_ref() {
                            last_usage = Some(Usage {
                                input_tokens: u.prompt_tokens,
                                output_tokens: u.completion_tokens,
                            });
                        }
                        if event.model.is_some() {
                            last_model = event.model.clone();
                        }

                        // Usage-only trailer (include_usage) has empty choices.
                        if event.choices.is_empty() {
                            continue;
                        }

                        if let Some(choice) = event.choices.into_iter().next() {
                            let content = choice.delta.content.unwrap_or_default();
                            let thinking = choice.delta.reasoning_content.unwrap_or_default();

                            let delta_tool_calls = choice.delta.tool_calls.map(|calls| {
                                calls
                                    .into_iter()
                                    .map(|tc| crate::DeltaToolCall {
                                        index: tc.index,
                                        id: tc.id,
                                        tool_type: tc.tool_type,
                                        function: tc.function.map(|f| {
                                            crate::DeltaToolCallFunction {
                                                name: f.name,
                                                arguments: f.arguments,
                                            }
                                        }),
                                    })
                                    .collect()
                            });

                            // Never mark done here — wait for [DONE] (or stream end)
                            // so trailing usage from stream_options is captured first.
                            let _ = tx.send(StreamChunk {
                                content,
                                thinking,
                                done: false,
                                model: last_model.clone(),
                                usage: None,
                                delta_tool_calls,
                            });
                        }
                    }
                }
            }
        }

        // Stream closed without [DONE]; still emit terminal chunk with best-effort usage.
        let _ = tx.send(StreamChunk {
            content: String::new(),
            thinking: String::new(),
            done: true,
            model: last_model,
            usage: last_usage,
            delta_tool_calls: None,
        });
        Ok(())
    }
}
