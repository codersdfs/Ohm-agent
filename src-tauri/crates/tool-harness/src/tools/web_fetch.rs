// Web fetch tool — HTTP GET with SSRF protection

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;

pub struct WebFetchTool;

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }
    fn description(&self) -> &str {
        "Fetch content from a URL via HTTP GET. Returns HTML stripped to text."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": string_param("The URL to fetch"),
                "maxChars": {
                    "type": "number",
                    "description": "Maximum characters of content to return (default: 100000)",
                    "default": 100000
                }
            },
            "required": ["url"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "web_fetch".into(),
            label: "Web Fetch".into(),
            description: "Fetch content from a URL via HTTP GET. Returns HTML stripped to text.".into(),
            doc: Some("Fetches content from a URL via HTTP GET.
- Blocks private IP ranges (SSRF protection): localhost, 127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16
- Strips HTML tags to return readable text
- Size cap: 100k chars by default (configurable via maxChars)
- Follows redirects (max 5)".into()),
            category: ToolCategory::WebNetwork,
            subcategory: Some("fetch".into()),
            tags: vec!["web".into(), "fetch".into(), "http".into(), "url".into(), "scrape".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: true,
            concurrency_safe: true,
            latency_hint: LatencyHint::Slow,
            supports_streaming: false,
            max_result_chars: 100_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "ssrf_blocked".into(),
                    description: "URL points to a private/internal IP address (SSRF protection)".into(),
                    recoverable: true,
                    retry_advice: Some("Use a public URL instead".into()),
                },
                ToolErrorSpec {
                    kind: "invalid_url".into(),
                    description: "The URL is malformed or uses an unsupported scheme".into(),
                    recoverable: true,
                    retry_advice: Some("Use a valid http:// or https:// URL".into()),
                },
                ToolErrorSpec {
                    kind: "request_failed".into(),
                    description: "HTTP request failed (network error, timeout, non-200 status)".into(),
                    recoverable: true,
                    retry_advice: Some("Check the URL and try again".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Fetch a web page".into(),
                    description: "Get text content from a URL".into(),
                    arguments: serde_json::json!({ "url": "https://example.com" }),
                    expected_result: Some("Example Domain\nThis domain is for use in illustrative examples...".into()),
                },
                ToolExample {
                    title: "Fetch with size limit".into(),
                    description: "Fetch first 10000 chars of content".into(),
                    arguments: serde_json::json!({
                        "url": "https://example.com",
                        "maxChars": 10000
                    }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 500, category: CostCategory::Moderate }),
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let url_str = input
            .args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: url"))?;

        let max_chars = input
            .args
            .get("maxChars")
            .and_then(|v| v.as_u64())
            .unwrap_or(100_000) as usize;

        // Validate URL scheme
        let parsed = url::Url::parse(url_str).map_err(|e| {
            ToolError::with_kind(
                crate::ToolErrorKind::SchemaValidation,
                format!("Invalid URL: {}", e),
            )
        })?;

        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(ToolError::with_kind(
                crate::ToolErrorKind::SchemaValidation,
                format!("Unsupported URL scheme: {}. Only http and https are allowed.", scheme),
            ));
        }

        // SSRF protection: block private IP ranges
        if is_private_url(&parsed) {
            return Err(ToolError::with_kind(
                crate::ToolErrorKind::PermissionDenied,
                "URL points to a private/internal IP address (SSRF protection)".to_string(),
            ));
        }

        // Fetch content
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(5))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ToolError::new(format!("Failed to create HTTP client: {}", e)))?;

        let response = client
            .get(url_str)
            .header("User-Agent", "Omega-Agent/1.0")
            .send()
            .await
            .map_err(|e| ToolError::with_kind(crate::ToolErrorKind::Timeout, format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::with_kind(
                crate::ToolErrorKind::ExecutionFailed,
                format!("HTTP {}: {}", status, url_str),
            ));
        }

        let body = response
            .text()
            .await
            .map_err(|e| ToolError::new(format!("Failed to read response body: {}", e)))?;

        // Strip HTML to text
        let text = strip_html(&body);

        // Truncate to max_chars
        let truncated = if text.len() > max_chars {
            format!(
                "...[truncated: kept first {} chars of {}; content is {} chars total]",
                max_chars,
                url_str,
                text.len()
            )
        } else {
            String::new()
        };

        let result_text = if text.len() > max_chars {
            format!("{}{}", &text[..max_chars], truncated)
        } else {
            text
        };

        Ok(ToolResult::success(result_text))
    }
}

/// Check if a URL points to a private/internal IP address (SSRF protection).
fn is_private_url(url: &url::Url) -> bool {
    // Get the host
    let host = match url.host() {
        Some(h) => h,
        None => return true, // No host — block
    };

    match host {
        url::Host::Domain(domain) => {
            // Block localhost variants
            let domain_lower = domain.to_lowercase();
            if domain_lower == "localhost"
                || domain_lower.ends_with(".localhost")
                || domain_lower.ends_with(".internal")
                || domain_lower.ends_with(".local")
            {
                return true;
            }

            // Try to resolve and check if it's a private IP
            // For now, also block common internal TLDs
            false
        }
        url::Host::Ipv4(ip) => {
            is_private_ipv4(ip)
        }
        url::Host::Ipv6(ip) => {
            is_private_ipv6(ip)
        }
    }
}

fn is_private_ipv4(ip: std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    // 127.0.0.0/8 — loopback
    if octets[0] == 127 {
        return true;
    }
    // 10.0.0.0/8 — private
    if octets[0] == 10 {
        return true;
    }
    // 172.16.0.0/12 — private
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }
    // 192.168.0.0/16 — private
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }
    // 169.254.0.0/16 — link-local
    if octets[0] == 169 && octets[1] == 254 {
        return true;
    }
    // 0.0.0.0/8 — current network
    if octets[0] == 0 {
        return true;
    }
    // 100.64.0.0/10 — carrier-grade NAT
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        return true;
    }
    false
}

fn is_private_ipv6(ip: std::net::Ipv6Addr) -> bool {
    let segments = ip.segments();
    // ::1 — loopback
    if ip == std::net::Ipv6Addr::LOCALHOST {
        return true;
    }
    // fe80::/10 — link-local (first 10 bits are 1111111010)
    if (segments[0] & 0xffc0) == 0xfe80 {
        return true;
    }
    // fc00::/7 — unique local (first 7 bits are 1111110)
    if (segments[0] & 0xfe00) == 0xfc00 {
        return true;
    }
    false
}

/// Strip HTML tags to produce readable text.
fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut in_script_or_style = false;
    let mut tag_name = String::new();

    for ch in html.chars() {
        if in_script_or_style {
            if ch == '>' {
                in_script_or_style = false;
                in_tag = false;
            }
            continue;
        }

        if in_tag {
            if ch == '>' {
                in_tag = false;
                let name_lower = tag_name.to_lowercase();
                if name_lower == "script" || name_lower == "style" {
                    in_script_or_style = true;
                }
                tag_name.clear();
            } else if ch.is_alphabetic() {
                tag_name.push(ch);
            }
            continue;
        }

        if ch == '<' {
            in_tag = true;
            tag_name.clear();
            continue;
        }

        if ch == '&' {
            // Handle basic HTML entities
            result.push(ch);
            continue;
        }

        result.push(ch);
    }

    // Decode basic HTML entities
    let decoded = result
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");

    // Collapse multiple whitespace
    let mut collapsed = String::new();
    let mut prev_ws = false;
    for line in decoded.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_ws {
                collapsed.push('\n');
                prev_ws = true;
            }
        } else {
            collapsed.push_str(trimmed);
            collapsed.push('\n');
            prev_ws = false;
        }
    }

    collapsed.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_private_ipv4() {
        assert!(is_private_ipv4("127.0.0.1".parse().unwrap()));
        assert!(is_private_ipv4("10.0.0.1".parse().unwrap()));
        assert!(is_private_ipv4("172.16.0.1".parse().unwrap()));
        assert!(is_private_ipv4("192.168.1.1".parse().unwrap()));
        assert!(is_private_ipv4("169.254.1.1".parse().unwrap()));
        assert!(!is_private_ipv4("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ipv4("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ipv6() {
        assert!(is_private_ipv6("::1".parse().unwrap()));
        assert!(is_private_ipv6("fe80::1".parse().unwrap()));
        assert!(is_private_ipv6("fc00::1".parse().unwrap()));
        assert!(!is_private_ipv6("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn test_is_private_url() {
        assert!(is_private_url(&url::Url::parse("http://localhost:8080").unwrap()));
        assert!(is_private_url(&url::Url::parse("http://127.0.0.1:8080").unwrap()));
        assert!(is_private_url(&url::Url::parse("http://10.0.0.1").unwrap()));
        assert!(is_private_url(&url::Url::parse("http://192.168.1.1").unwrap()));
        assert!(!is_private_url(&url::Url::parse("https://example.com").unwrap()));
        assert!(!is_private_url(&url::Url::parse("http://8.8.8.8").unwrap()));
    }

    #[test]
    fn test_strip_html() {
        let html = "<html><body><h1>Title</h1><p>Hello world</p></body></html>";
        let text = strip_html(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn test_strip_html_with_script() {
        let html = "<html><head><script>alert('xss')</script></head><body>Visible</body></html>";
        let text = strip_html(html);
        assert!(text.contains("Visible"));
        assert!(!text.contains("alert"));
    }

    #[test]
    fn test_strip_html_entities() {
        let html = "<p>Hello &amp; goodbye &lt;tag&gt;</p>";
        let text = strip_html(html);
        assert!(text.contains("Hello & goodbye <tag>"));
    }

    #[tokio::test]
    async fn test_invalid_url_scheme() {
        let tool = WebFetchTool::new();
        let input = ToolInput {
            tool: "web_fetch".into(),
            args: serde_json::json!({ "url": "ftp://example.com" }),
        };
        let ctx = ToolUseContext::new("test");

        // This will fail at URL parsing or scheme check
        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ssrf_blocked() {
        let tool = WebFetchTool::new();
        let input = ToolInput {
            tool: "web_fetch".into(),
            args: serde_json::json!({ "url": "http://127.0.0.1:8080" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("SSRF") || err.message.contains("private"));
    }
}
