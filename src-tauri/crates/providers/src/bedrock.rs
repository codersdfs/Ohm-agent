use crate::{ChatRequest, ChatResponse, LlmProvider, ProviderConfig, StreamChunk, Usage};
use bytes::Buf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::time::SystemTime;

type HmacSha256 = Hmac<Sha256>;

// ---------------------------------------------------------------------------
// SigV4 — canonical-request hash (extractable for unit tests)
// ---------------------------------------------------------------------------

/// SHA-256 hex digest of a byte slice.
pub(crate) fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Canonical-request hash suitable for SigV4.
///
/// `canonical_request_hash(method, path, query, headers, signed_headers, payload_hash)` →
/// hex SHA-256 of the canonical request string.
///
/// Exposed `pub(crate)` so that tests can verify the hash without HTTP.
pub(crate) fn canonical_request_hash(
    method: &str,
    path: &str,
    query: &str,
    headers: &[(&str, &str)],
    signed_headers: &[&str],
    payload_hash: &str,
) -> String {
    // Sort headers by lowercase name for canonical form
    let mut sorted_headers: Vec<(&str, &str)> = headers.to_vec();
    sorted_headers.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let canonical_headers: String = sorted_headers
        .iter()
        .map(|(k, v)| format!("{}:{}\n", k.to_lowercase(), v.trim()))
        .collect();

    let signed_headers_str = signed_headers
        .iter()
        .map(|h| h.to_lowercase())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, path, query, canonical_headers, signed_headers_str, payload_hash
    );

    sha256_hex(canonical_request.as_bytes())
}

/// Derive the AWS SigV4 signing key.
fn signing_key(secret_key: &str, date_stamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = {
        let mut mac = HmacSha256::new_from_slice(format!("AWS4{}", secret_key).as_bytes())
            .expect("HMAC accepts any key");
        mac.update(date_stamp.as_bytes());
        mac.finalize().into_bytes()
    };
    let k_region = {
        let mut mac = HmacSha256::new_from_slice(&k_date).expect("HMAC key");
        mac.update(region.as_bytes());
        mac.finalize().into_bytes()
    };
    let k_service = {
        let mut mac = HmacSha256::new_from_slice(&k_region).expect("HMAC key");
        mac.update(service.as_bytes());
        mac.finalize().into_bytes()
    };
    let k_signing = {
        let mut mac = HmacSha256::new_from_slice(&k_service).expect("HMAC key");
        mac.update(b"aws4_request");
        mac.finalize().into_bytes()
    };
    k_signing.to_vec()
}

/// Compute a full SigV4 signature and return `(authorization_header, date_header_value)`.
fn sign_request(
    method: &str,
    path: &str,
    query: &str,
    host: &str,
    region: &str,
    access_key: &str,
    secret_key: &str,
    session_token: Option<&str>,
    payload: &[u8],
    now: SystemTime,
) -> (String, String) {
    let date_stamp = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let date_header = format_date(date_stamp);
    let date_stamp_str = &date_header[..8]; // YYYYMMDD

    let payload_hash = sha256_hex(payload);

    let mut headers: Vec<(&str, &str)> = vec![
        ("host", host),
        ("x-amz-date", &date_header),
        ("x-amz-content-sha256", &payload_hash),
    ];
    let mut signed_headers_list = vec!["host", "x-amz-date", "x-amz-content-sha256"];

    // Session token is optional — include as an extra signed header when present
    let session_token_header;
    if let Some(token) = session_token {
        session_token_header = token.to_string();
        headers.push(("x-amz-security-token", &session_token_header));
        signed_headers_list.push("x-amz-security-token");
    }

    let cr_hash = canonical_request_hash(
        method,
        path,
        query,
        &headers,
        &signed_headers_list,
        &payload_hash,
    );

    let credential_scope = format!("{}/{}/bedrock/aws4_request", date_stamp_str, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        date_header, credential_scope, cr_hash
    );

    let key = signing_key(secret_key, date_stamp_str, region, "bedrock");
    let mut mac = HmacSha256::new_from_slice(&key).expect("HMAC key");
    mac.update(string_to_sign.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let signed_headers = signed_headers_list.join(";");
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        access_key, credential_scope, signed_headers, signature
    );

    (authorization, date_header)
}

/// Format a UNIX timestamp as `YYYYMMDDTHHmmSSZ`.
fn format_date(secs: u64) -> String {
    // Simple UTC formatting without pulling in chrono
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let mins = (rem % 3600) / 60;
    let secs = rem % 60;

    // Compute Y/M/D from days since epoch
    let mut y = 1970u64;
    let mut remaining_days = days;
    loop {
        let leap = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < leap {
            break;
        }
        remaining_days -= leap;
        y += 1;
    }
    let leap = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
        366
    } else {
        365
    };
    let md = month_day_from_yearday(y, remaining_days, leap == 366);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        y, md.0, md.1, hours, mins, secs
    )
}

fn month_day_from_yearday(_year: u64, yearday: u64, leap: bool) -> (u64, u64) {
    let days_in_month: Vec<(u64, u64)> = if leap {
        vec![
            (1, 31),
            (2, 29),
            (3, 31),
            (4, 30),
            (5, 31),
            (6, 30),
            (7, 31),
            (8, 31),
            (9, 30),
            (10, 31),
            (11, 30),
            (12, 31),
        ]
    } else {
        vec![
            (1, 31),
            (2, 28),
            (3, 31),
            (4, 30),
            (5, 31),
            (6, 30),
            (7, 31),
            (8, 31),
            (9, 30),
            (10, 31),
            (11, 30),
            (12, 31),
        ]
    };
    let mut day_count = 0u64;
    for &(m, dim) in &days_in_month {
        if yearday < day_count + dim {
            return (m, yearday - day_count + 1);
        }
        day_count += dim;
    }
    (12, 31) // fallback
}

// ---------------------------------------------------------------------------
// BedrockProvider
// ---------------------------------------------------------------------------

pub struct BedrockProvider {
    region: String,
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    model_id: String,
    max_tokens: u32,
    temperature: f32,
}

impl BedrockProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        // Region: extract from base_url if it looks like a Bedrock endpoint, else default
        let region = config
            .base_url
            .as_deref()
            .and_then(|url| extract_region(url))
            .unwrap_or_else(|| {
                std::env::var("AWS_REGION")
                    .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
                    .unwrap_or_else(|_| "us-east-1".into())
            });

        // Credentials: api_key can be "access_key:secret_key", else use env vars
        let (access_key, secret_key) = if let Some(key) = &config.api_key {
            if let Some((ak, sk)) = key.split_once(':') {
                (ak.to_string(), sk.to_string())
            } else {
                (
                    key.clone(),
                    std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default(),
                )
            }
        } else {
            (
                std::env::var("AWS_ACCESS_KEY_ID").unwrap_or_default(),
                std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default(),
            )
        };

        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();

        // Model ID from config.model; Bedrock models use format like
        // "anthropic.claude-3-5-sonnet-20241022-v2:0" or
        // "us.anthropic.claude-3-5-sonnet-20241022-v2:0"
        let model_id = config.model.clone();

        Self {
            region,
            access_key,
            secret_key,
            session_token,
            model_id,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        }
    }

    fn base_url(&self) -> String {
        format!("https://bedrock-runtime.{}.amazonaws.com", self.region)
    }

    fn host(&self) -> String {
        format!("bedrock-runtime.{}.amazonaws.com", self.region)
    }

    fn converse_path(&self) -> String {
        format!("/model/{}/converse", self.model_id)
    }

    fn converse_stream_path(&self) -> String {
        format!("/model/{}/converse-stream", self.model_id)
    }

    fn build_converse_body(&self, request: &ChatRequest) -> serde_json::Value {
        // Extract system messages
        let system_texts: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| serde_json::json!({ "text": m.content }))
            .collect();

        // Non-system messages → Bedrock format
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| {
                let role = if m.role == "assistant" || m.role == "user" {
                    m.role.clone()
                } else if m.role == "tool" {
                    // Bedrock doesn't have a "tool" role; map to "user"
                    "user".to_string()
                } else {
                    m.role.clone()
                };

                let mut content_blocks: Vec<serde_json::Value> = vec![];

                // Text content
                if !m.content.is_empty() {
                    content_blocks.push(serde_json::json!({ "text": m.content }));
                }

                // Tool results
                if m.role == "tool" {
                    // Tool results go as content blocks with toolResult
                    content_blocks = vec![serde_json::json!({
                        "toolResult": {
                            "toolUseId": m.tool_call_id.clone().unwrap_or_default(),
                            "content": [{ "text": m.content }]
                        }
                    })];
                }

                // Tool calls (from assistant)
                if let Some(ref tool_calls) = m.tool_calls {
                    for tc in tool_calls {
                        let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::json!({}));
                        content_blocks.push(serde_json::json!({
                            "toolUse": {
                                "toolUseId": tc.id,
                                "name": tc.function.name,
                                "input": input
                            }
                        }));
                    }
                }

                serde_json::json!({
                    "role": role,
                    "content": content_blocks
                })
            })
            .collect();

        // Tool definitions → Bedrock tool format
        let tools: Option<Vec<serde_json::Value>> = request.tools.as_ref().map(|tools| {
            tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "toolSpec": {
                            "name": t.function.name,
                            "description": t.function.description,
                            "inputSchema": t.function.parameters
                        }
                    })
                })
                .collect()
        });

        let mut body = serde_json::json!({
            "messages": messages,
            "inferenceConfig": {
                "maxTokens": self.max_tokens,
                "temperature": self.temperature
            }
        });

        if !system_texts.is_empty() {
            body["system"] = serde_json::json!(system_texts);
        }

        if let Some(tools) = tools {
            body["tools"] = serde_json::json!(tools);
        }

        body
    }

    async fn sign_and_send(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Result<(u16, bytes::Bytes), String> {
        let host = self.host();
        let (authorization, date) = sign_request(
            method,
            path,
            "",
            &host,
            &self.region,
            &self.access_key,
            &self.secret_key,
            self.session_token.as_deref(),
            body,
            SystemTime::now(),
        );

        let url = format!("{}{}", self.base_url(), path);
        let client = reqwest::Client::new();
        let http_method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|e| format!("Invalid HTTP method: {e}"))?;
        let mut req_builder = client
            .request(http_method, &url)
            .header("Authorization", &authorization)
            .header("x-amz-date", &date)
            .header("x-amz-content-sha256", sha256_hex(body))
            .header("content-type", "application/json");

        if let Some(token) = &self.session_token {
            req_builder = req_builder.header("x-amz-security-token", token);
        }

        let resp = if body.is_empty() {
            req_builder.send().await
        } else {
            req_builder.body(body.to_vec()).send().await
        };

        let resp = resp.map_err(|e| format!("request failed: {}", e))?;
        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("failed to read response: {}", e))?;

        if !status.is_success() {
            let text = String::from_utf8_lossy(&bytes);
            return Err(format!("API error {}: {}", status, text));
        }

        Ok((status.as_u16(), bytes))
    }
}

fn extract_region(url: &str) -> Option<String> {
    // URLs like "https://bedrock-runtime.us-west-2.amazonaws.com" → "us-west-2"
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = without_scheme.split('/').next()?;
    let prefix = "bedrock-runtime.";
    if host.starts_with(prefix) {
        let region_part = &host[prefix.len()..];
        let region = region_part.split('.').next()?;
        if !region.is_empty() {
            return Some(region.to_string());
        }
    }
    None
}

#[async_trait::async_trait]
impl LlmProvider for BedrockProvider {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, String> {
        let body = self.build_converse_body(&request);
        let body_bytes =
            serde_json::to_vec(&body).map_err(|e| format!("failed to serialize request: {}", e))?;

        let (_status, bytes) = self
            .sign_and_send("POST", &self.converse_path(), &body_bytes)
            .await?;

        // Parse Converse response
        let resp: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("failed to parse response: {}", e))?;

        let output = resp
            .get("output")
            .and_then(|o| o.get("message"))
            .ok_or_else(|| format!("unexpected response structure: {}", resp))?;

        let content = output
            .get("content")
            .and_then(|c| c.as_array())
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let model = resp
            .get("modelId")
            .and_then(|m| m.as_str())
            .unwrap_or(&self.model_id)
            .to_string();

        let usage = resp.get("usage").map(|u| Usage {
            input_tokens: u.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            output_tokens: u.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        });

        // Extract tool calls if present
        let tool_calls = output
            .get("content")
            .and_then(|c| c.as_array())
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| {
                        if let Some(tool_use) = b.get("toolUse") {
                            let id = tool_use
                                .get("toolUseId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let name = tool_use
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let args = tool_use
                                .get("input")
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "{}".into());
                            Some(crate::ToolCall {
                                id,
                                tool_type: "function".into(),
                                function: crate::ToolCallFunction {
                                    name,
                                    arguments: args,
                                },
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());

        Ok(ChatResponse {
            content,
            model,
            usage,
            tool_calls,
        })
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
        tx: tokio::sync::mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), String> {
        let body = self.build_converse_body(&request);
        let body_bytes =
            serde_json::to_vec(&body).map_err(|e| format!("failed to serialize request: {}", e))?;

        let path = self.converse_stream_path();
        let host = self.host();
        let (authorization, date) = sign_request(
            "POST",
            &path,
            "",
            &host,
            &self.region,
            &self.access_key,
            &self.secret_key,
            self.session_token.as_deref(),
            &body_bytes,
            SystemTime::now(),
        );

        let url = format!("{}{}", self.base_url(), path);
        let client = reqwest::Client::new();
        let http_method = reqwest::Method::from_bytes(b"POST")
            .map_err(|e| format!("Invalid HTTP method: {e}"))?;
        let mut req_builder = client
            .request(http_method, &url)
            .header("Authorization", &authorization)
            .header("x-amz-date", &date)
            .header("x-amz-content-sha256", sha256_hex(&body_bytes))
            .header("accept", "application/vnd.amazon.eventstream");

        if let Some(token) = &self.session_token {
            req_builder = req_builder.header("x-amz-security-token", token);
        }

        let resp = req_builder
            .body(body_bytes)
            .send()
            .await
            .map_err(|e| format!("stream request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("stream API error {}: {}", status, text));
        }

        // Bedrock streaming returns binary event-stream framing, not SSE.
        // Each frame has a header (with content-type, etc.) followed by a JSON payload.
        // The JSON payloads include:
        //   {"messageStart": {"role":"assistant"}}
        //   {"contentBlockStart": {"contentBlockIndex":0}}
        //   {"contentBlockDelta": {"contentBlockIndex":0, "delta":{"text":"..."}}}
        //   {"contentBlockStop": {"contentBlockIndex":0}}
        //   {"messageStop": {"stopReason":"end_turn"}}
        //   {"metadata": {"usage":{...}, "metrics":{...}}}
        //
        // The binary framing: 4-byte total length, 4-byte headers length, headers, payload.
        // Content-type header "application/vnd.amazon.eventstream" uses a TLV format.
        // We parse this manually to avoid pulling in the full `aws-smithy-eventstream` crate.

        use futures_util::StreamExt;
        let stream = resp.bytes_stream();
        tokio::pin!(stream);

        let mut buf = bytes::BytesMut::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("stream read error: {}", e))?;
            buf.extend_from_slice(&chunk);

            // Process complete event-stream frames
            loop {
                if buf.len() < 4 {
                    break;
                }
                let total_len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
                if total_len < 4 {
                    // Invalid frame, skip
                    buf.advance(4);
                    continue;
                }
                if buf.len() < total_len {
                    break; // Need more data
                }

                let frame = buf.split_to(total_len);
                // Frame layout: 4 bytes total_len, 4 bytes headers_len, headers, payload
                if frame.len() < 8 {
                    continue;
                }
                let headers_len =
                    u32::from_be_bytes([frame[4], frame[5], frame[6], frame[7]]) as usize;

                let payload_start = 8 + headers_len;
                if payload_start > frame.len() {
                    continue;
                }
                let payload = &frame[payload_start..];

                if let Ok(payload_str) = std::str::from_utf8(payload) {
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(payload_str) {
                        // contentBlockDelta carries text
                        if let Some(delta) = event.get("contentBlockDelta") {
                            if let Some(d) = delta.get("delta") {
                                if let Some(text) = d.get("text").and_then(|t| t.as_str()) {
                                    let _ = tx.send(StreamChunk {
                                        content: text.to_string(),
                                        thinking: String::new(),
                                        done: false,
                                        model: Some(self.model_id.clone()),
                                        usage: None,
                                        delta_tool_calls: None,
                                    });
                                }
                            }
                        }

                        // contentBlockStart may carry toolUse info
                        if let Some(start) = event.get("contentBlockStart") {
                            if let Some(tool_use) = start.get("toolUse") {
                                let id = tool_use
                                    .get("toolUseId")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let name = tool_use
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let _ = tx.send(StreamChunk {
                                    content: String::new(),
                                    thinking: String::new(),
                                    done: false,
                                    model: Some(self.model_id.clone()),
                                    usage: None,
                                    delta_tool_calls: Some(vec![crate::DeltaToolCall {
                                        index: 0,
                                        id: Some(id),
                                        tool_type: Some("function".into()),
                                        function: Some(crate::DeltaToolCallFunction {
                                            name: Some(name),
                                            arguments: None,
                                        }),
                                    }]),
                                });
                            }
                        }

                        // contentBlockDelta may carry toolUse input
                        if let Some(delta) = event.get("contentBlockDelta") {
                            if let Some(tool_delta) = delta.get("delta") {
                                if let Some(partial) =
                                    tool_delta.get("partialJson").and_then(|v| v.as_str())
                                {
                                    let _ = tx.send(StreamChunk {
                                        content: String::new(),
                                        thinking: String::new(),
                                        done: false,
                                        model: Some(self.model_id.clone()),
                                        usage: None,
                                        delta_tool_calls: Some(vec![crate::DeltaToolCall {
                                            index: 0,
                                            id: None,
                                            tool_type: None,
                                            function: Some(crate::DeltaToolCallFunction {
                                                name: None,
                                                arguments: Some(partial.to_string()),
                                            }),
                                        }]),
                                    });
                                }
                            }
                        }

                        // messageStop → done
                        if event.get("messageStop").is_some() {
                            let _ = tx.send(StreamChunk {
                                content: String::new(),
                                thinking: String::new(),
                                done: true,
                                model: Some(self.model_id.clone()),
                                usage: None,
                                delta_tool_calls: None,
                            });
                            return Ok(());
                        }

                        // metadata may carry usage
                        if let Some(metadata) = event.get("metadata") {
                            if let Some(usage) = metadata.get("usage") {
                                let input_tokens = usage
                                    .get("inputTokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32;
                                let output_tokens = usage
                                    .get("outputTokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32;
                                let _ = tx.send(StreamChunk {
                                    content: String::new(),
                                    thinking: String::new(),
                                    done: false,
                                    model: Some(self.model_id.clone()),
                                    usage: Some(Usage {
                                        input_tokens,
                                        output_tokens,
                                    }),
                                    delta_tool_calls: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Stream ended without explicit messageStop
        let _ = tx.send(StreamChunk {
            content: String::new(),
            thinking: String::new(),
            done: true,
            model: Some(self.model_id.clone()),
            usage: None,
            delta_tool_calls: None,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hex_empty() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_signing_key_derivation() {
        // AWS test vector: https://docs.aws.amazon.com/general/latest/gr/signature-v4-examples.html
        let key = signing_key(
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "20150830",
            "us-east-1",
            "service",
        );
        let hex_key = hex::encode(&key);
        // The derived key is a 32-byte HMAC; verify it's deterministic
        let key2 = signing_key(
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "20150830",
            "us-east-1",
            "service",
        );
        assert_eq!(hex_key, hex::encode(&key2));
    }

    #[test]
    fn test_extract_region_from_url() {
        assert_eq!(
            extract_region("https://bedrock-runtime.us-west-2.amazonaws.com"),
            Some("us-west-2".into())
        );
        assert_eq!(
            extract_region("https://bedrock-runtime.eu-central-1.amazonaws.com"),
            Some("eu-central-1".into())
        );
        assert_eq!(extract_region("https://api.openai.com/v1"), None);
    }
    #[test]
    fn test_format_date() {
        // Use a known timestamp: 2024-01-01T00:00:00Z = 1704067200
        let formatted = format_date(1704067200);
        assert_eq!(formatted, "20240101T000000Z");
    }

    #[test]
    fn test_canonical_request_hash_deterministic() {
        let h1 = canonical_request_hash(
            "POST",
            "/model/test/converse",
            "",
            &[("host", "example.com"), ("x-amz-date", "20150830T000000Z")],
            &["host", "x-amz-date"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        let h2 = canonical_request_hash(
            "POST",
            "/model/test/converse",
            "",
            &[("x-amz-date", "20150830T000000Z"), ("host", "example.com")],
            &["host", "x-amz-date"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        // Hash should be the same regardless of header insertion order
        assert_eq!(h1, h2);
    }
}
