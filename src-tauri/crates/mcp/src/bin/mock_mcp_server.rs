//! Mock MCP server for integration testing the stdio transport.
//! Reads a framed JSON-RPC request from stdin, writes a framed response to stdout.
//! Uses Content-Length header framing per the MCP spec.

use std::io::{self, Read, Write};

const HEADER_SEPARATOR: &str = "\r\n\r\n";

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

fn read_frame(stdin: &mut impl Read) -> Option<String> {
    // Read headers until we find the separator
    let mut header_buf = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        if stdin.read(&mut byte).ok()? == 0 {
            return None; // EOF
        }
        header_buf.push(byte[0]);

        if header_buf.ends_with(HEADER_SEPARATOR.as_bytes()) {
            break;
        }
    }

    // Parse content-length from headers
    let header_str = String::from_utf8_lossy(&header_buf);
    let length = parse_content_length(&header_str)?;

    // Read exactly `length` bytes of body
    let mut body = vec![0u8; length];
    stdin.read_exact(&mut body).ok()?;

    Some(String::from_utf8_lossy(&body).to_string())
}

fn write_frame(stdout: &mut impl Write, json: &str) -> io::Result<()> {
    let header = format!("Content-Length: {}\r\n\r\n", json.len());
    stdout.write_all(header.as_bytes())?;
    stdout.write_all(json.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin_lock = stdin.lock();
    let mut stdout_lock = stdout.lock();

    // Read one request frame
    if let Some(request_json) = read_frame(&mut stdin_lock) {
        // Parse the JSON-RPC request
        let v: serde_json::Value = serde_json::from_str(&request_json).unwrap_or_default();
        let id = v["id"].clone();

        // Respond with a pong
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": "pong"
        });

        let response_str = response.to_string();
        if write_frame(&mut stdout_lock, &response_str).is_err() {
            eprintln!("Failed to write response frame");
        }
    }
}
