//! MCP stdio transport — spec-compliant Content-Length framing.
//!
//! Spawns a subprocess and exchanges JSON-RPC messages over stdin/stdout
//! using the MCP stdio transport protocol:
//!
//!   Content-Length: <N>\r\n
//!   MimeType: application/json\r\n
//!   \r\n
//!   <JSON body, exactly N bytes>
//!
//! Not newline-delimited — frames are length-prefixed per the MCP spec.

use crate::{McpRequest, McpResponse};
use serde_json::Value;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Framing separator: end of headers + start of body
const HEADER_SEPARATOR: &str = "\r\n\r\n";

/// Maximum message body size we'll read in one frame (10 MB).
const MAX_BODY: usize = 10 * 1024 * 1024;

/// Frame a JSON string into MCP stdio wire format.
pub fn frame_message(json: &str) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", json.len());
    let mut buf = Vec::with_capacity(header.len() + json.len());
    buf.extend_from_slice(header.as_bytes());
    buf.extend_from_slice(json.as_bytes());
    buf
}

/// Parse one or more complete frames from a byte buffer.
/// Returns (parsed JSON strings, remaining unparsed bytes).
pub fn parse_frames(buf: &mut Vec<u8>) -> (Vec<String>, Vec<u8>) {
    let mut frames = Vec::new();
    let mut offset = 0usize;

    loop {
        let search_buf = &buf[offset..];

        let sep_rel = search_buf
            .windows(HEADER_SEPARATOR.len())
            .position(|w| w == HEADER_SEPARATOR.as_bytes());

        let sep = match sep_rel {
            Some(p) => p + offset + HEADER_SEPARATOR.len(),
            None => break,
        };

        let header_block = &buf[offset..sep - HEADER_SEPARATOR.len()];
        let header_str = std::str::from_utf8(header_block).unwrap_or("");
        let length = match parse_content_length(header_str) {
            Some(n) => n,
            None => break,
        };

        if length > MAX_BODY {
            return (frames, Vec::new());
        }

        let body_start = sep;
        let body_end = body_start + length;

        if buf.len() < body_end {
            break;
        }

        let body = match std::str::from_utf8(&buf[body_start..body_end]) {
            Ok(s) => s.to_string(),
            Err(_) => {
                offset = body_end;
                continue;
            }
        };

        frames.push(body);
        offset = body_end;
    }

    let remaining = if offset == 0 {
        std::mem::take(buf)
    } else {
        buf.split_off(offset)
    };

    (frames, remaining)
}

fn parse_content_length(header: &str) -> Option<usize> {
    for line in header.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            if let Ok(n) = rest.trim().parse::<usize>() {
                return Some(n);
            }
        }
    }
    None
}

/// Stdio transport — spawns a subprocess and exchanges framed JSON-RPC.
pub struct StdioTransport {
    stdin: Mutex<tokio::process::ChildStdin>,
    reader: Mutex<BufReader<tokio::process::ChildStdout>>,
    child: Mutex<Option<Child>>,
    stderr_task: Option<tokio::task::JoinHandle<()>>,
}
impl StdioTransport {
    pub fn spawn(program: &str, args: &[&str]) -> Result<Self, String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn MCP subprocess `{program}`: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Subprocess did not provide stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Subprocess did not provide stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Subprocess did not provide stderr".to_string())?;

        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => log::warn!("[mcp-stdio] {}", line.trim_end()),
                    Err(e) => {
                        log::warn!("[mcp-stdio] stderr read error: {e}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            stdin: Mutex::new(stdin),
            reader: Mutex::new(BufReader::new(stdout)),
            child: Mutex::new(Some(child)),
            stderr_task: Some(stderr_task),
        })
    }

    /// Send a JSON-RPC request and await the response.
    pub async fn send(&self, request: McpRequest) -> Result<McpResponse, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request.id,
            "method": request.method,
            "params": request.params,
        });

        let json_str = serde_json::to_string(&body)
            .map_err(|e| format!("Failed to serialize MCP request: {e}"))?;

        let framed = frame_message(&json_str);

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(&framed)
                .await
                .map_err(|e| format!("Failed to write to MCP subprocess stdin: {e}"))?;
            stdin
                .flush()
                .await
                .map_err(|e| format!("Failed to flush MCP subprocess stdin: {e}"))?;
        }

        self.read_response().await
    }

    async fn read_response(&self) -> Result<McpResponse, String> {
        let mut buf: Vec<u8> = Vec::new();

        loop {
            // Read raw bytes (not lines) — Content-Length framing does not
            // require newline delimiters, and JSON bodies may contain newlines.
            let mut chunk = vec![0u8; 8192];
            let bytes_read = {
                let mut reader = self.reader.lock().await;
                match reader.read(&mut chunk).await {
                    Ok(0) => return Err("MCP subprocess closed stdout unexpectedly".into()),
                    Ok(n) => n,
                    Err(e) => return Err(format!("Failed to read from MCP subprocess: {e}")),
                }
            };

            buf.extend_from_slice(&chunk[..bytes_read]);

            let (frames, remaining) = parse_frames(&mut buf);
            buf = remaining;

            if let Some(frame_json) = frames.into_iter().next() {
                let resp: McpResponse = serde_json::from_str(&frame_json)
                    .map_err(|e| format!("Failed to parse MCP response: {e}"))?;
                return Ok(resp);
            }
        }
    }

    /// Close the subprocess gracefully.
    pub async fn close(&self) -> Result<(), String> {
        let mut child_opt = self.child.lock().await;
        if let Some(mut child) = child_opt.take() {
            // Drop stdin so the subprocess sees EOF
            let stdin = child.stdin.take();
            drop(stdin);
            // Wait for child to exit (with a timeout)
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                child.wait(),
            )
            .await
            {
                Ok(result) => {
                    result.map_err(|e| format!("Failed waiting for MCP subprocess: {e}"))?;
                }
                Err(_) => {
                    let _ = child.kill().await;
                    return Err("Timed out waiting for MCP subprocess to exit".into());
                }
            }
        }
        if let Some(task) = self.stderr_task.as_ref() {
            task.abort();
        }
        Ok(())
    }

    /// Build a request with arbitrary params (convenience for tool calls).
    pub fn make_request(method: &str, params: HashMap<String, Value>, id: String) -> McpRequest {
        McpRequest {
            method: method.to_string(),
            params: Some(params),
            id,
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_message_correct_format() {
        let json = r#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#;
        let framed = frame_message(json);
        let expected = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
        assert_eq!(
            framed,
            expected.as_bytes(),
            "framed message should match expected format"
        );
    }

    #[test]
    fn parse_single_frame() {
        let json = r#"{"jsonrpc":"2.0","id":"1","result":"ok"}"#;
        let framed = frame_message(json);
        let mut buf = framed.clone();
        let (frames, remaining) = parse_frames(&mut buf);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], json);
        assert!(remaining.is_empty());
    }

    #[test]
    fn parse_multiple_frames_in_one_buffer() {
        let f1 = frame_message(r#"{"id":"1"}"#);
        let f2 = frame_message(r#"{"id":"2"}"#);
        let mut combined = f1.clone();
        combined.extend(f2.clone());

        let mut buf = combined;
        let (frames, remaining) = parse_frames(&mut buf);
        assert_eq!(frames.len(), 2);
        assert!(remaining.is_empty());
    }

    #[test]
    fn parse_partial_frame_waits_for_more_data() {
        let json = r#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#;
        let framed = frame_message(json);
        let partial = &framed[..framed.len() / 2];
        let mut buf = partial.to_vec();
        let (frames, remaining) = parse_frames(&mut buf);
        assert!(frames.is_empty());
        assert_eq!(remaining.len(), partial.len());
    }

    #[test]
    fn parse_large_payload() {
        let big_json = format!(r#"{{"data":"{}"}}"#, "x".repeat(100_000));
        let framed = frame_message(&big_json);
        let mut buf = framed.clone();
        let (frames, remaining) = parse_frames(&mut buf);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], big_json);
        assert!(remaining.is_empty());
    }
}
