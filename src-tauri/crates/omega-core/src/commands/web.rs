//! Web commands — internet-referencing capabilities for the Omega TUI.
//!
//! Provides `/fetch`, `/status`, and `/search` slash command backends.

use serde::{Deserialize, Serialize};

/// Timeout for all HTTP requests (15 seconds).
const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Maximum response body length before truncation.
const MAX_BODY_LENGTH: usize = 10_000;

/// A single web search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Build a shared reqwest client with sensible defaults.
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent("Omega-Agent/0.1")
        .build()
        .expect("reqwest Client::builder() should never fail with these options")
}

/// Fetch the text content of a URL.
///
/// Returns the response body as a string, truncated at MAX_BODY_LENGTH.
/// Errors on non-2xx status codes, timeouts, or DNS failures.
pub async fn fetch_url(url: &str) -> Result<String, String> {
    if url.trim().is_empty() {
        return Err("URL must not be empty".into());
    }

    let client = http_client();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "HTTP {} {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown")
        ));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    // For binary content types, note it
    if content_type.starts_with("image/")
        || content_type.starts_with("audio/")
        || content_type.starts_with("video/")
        || content_type == "application/octet-stream"
    {
        return Err(format!("Unsupported content type: {content_type}"));
    }

    let truncated = if body.len() > MAX_BODY_LENGTH {
        let mut t = body.chars().take(MAX_BODY_LENGTH).collect::<String>();
        t.push_str(&format!(
            "\n\n[... truncated at {} characters; actual body is {} bytes]",
            MAX_BODY_LENGTH,
            body.len()
        ));
        t
    } else {
        body
    };

    Ok(truncated)
}

/// Check network connectivity and report status.
///
/// Attempts to reach multiple known endpoints and returns a JSON summary.
pub async fn check_status() -> Result<serde_json::Value, String> {
    let client = http_client();
    let mut results = serde_json::Map::new();

    // Check general internet connectivity
    let internet = match client
        .get("https://httpbin.org/get")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    };
    results.insert("internet_reachable".into(), serde_json::json!(internet));

    // Build reachable endpoints map
    let mut endpoints = serde_json::Map::new();

    // Check provider endpoints based on common defaults
    let checks = [
        ("openai", "https://api.openai.com/v1/models"),
        ("anthropic", "https://api.anthropic.com/v1/messages"),
        ("google", "https://generativelanguage.googleapis.com"),
    ];
    for (name, url) in &checks {
        let ok = match client.get(*url).timeout(std::time::Duration::from_secs(5)).send().await {
            Ok(resp) => !resp.status().is_server_error(), // 4xx is fine (auth needed)
            Err(_) => false,
        };
        endpoints.insert(name.to_string(), serde_json::json!(ok));
    }
    results.insert("provider_endpoints".into(), serde_json::Value::Object(endpoints));

    Ok(serde_json::Value::Object(results))
}

/// Search the web using a configurable search API.
///
/// Uses DuckDuckGo Lite API by default (no API key required).
/// The search endpoint can be overridden via `OMEGA_SEARCH_API` env var.
pub async fn search_web(query: &str) -> Result<Vec<WebSearchResult>, String> {
    if query.trim().is_empty() {
        return Err("Search query must not be empty".into());
    }

    let search_url = std::env::var("OMEGA_SEARCH_API")
        .unwrap_or_else(|_| "https://lite.duckduckgo.com/lite/".into());

    let client = http_client();
    let response = client
        .post(&search_url)
        .form(&[("q", query)])
        .send()
        .await
        .map_err(|e| format!("Search request failed: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Search API returned HTTP {}", status.as_u16()));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read search results: {e}"))?;

    // Parse DuckDuckGo Lite HTML response
    let results = parse_duckduckgo_lite(&body);

    if results.is_empty() {
        // Fallback: try to extract any meaningful text
        return Err("No search results found. The search endpoint may have changed.".into());
    }

    Ok(results)
}

/// Parse DuckDuckGo Lite's minimal HTML result format.
fn parse_duckduckgo_lite(html: &str) -> Vec<WebSearchResult> {
    let mut results = Vec::new();

    // Simple parser: look for result rows in the minimal HTML table
    // DuckDuckGo Lite returns: <a href="url" class="result-link">title</a> then <p class="result-snippet">snippet</p>
    for block in html.split("<a href=\"").skip(1) {
        let url = match block.split('\"').next() {
            Some(u) if !u.is_empty() && u.starts_with("http") => u.to_string(),
            _ => continue,
        };

        // Extract title: everything between '>' and '</a>'
        let rest_after_url = block.split('>').nth(1).unwrap_or("");
        let title = rest_after_url.split('<').next().unwrap_or("").to_string();

        // Extract snippet from the next result-snippet div
        let snippet = if let Some(snip_start) = block.find("class=\"result-snippet\"") {
            let after = &block[snip_start..];
            after
                .split('>')
                .nth(1)
                .and_then(|s| s.split('<').next())
                .map(|s| html_unescape(s))
                .unwrap_or_default()
        } else {
            String::new()
        };

        results.push(WebSearchResult {
            title: html_unescape(&title),
            url,
            snippet,
        });
    }

    results
}

/// Minimal HTML entity unescape for common entities.
fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_url_rejects_empty() {
        let result = fetch_url("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_url_rejects_bad_url() {
        let result = fetch_url("https://this-domain-definitely-does-not-exist-12345.com").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_status_returns_json() {
        let result = check_status().await;
        // This test may fail offline — it's an integration test
        if let Ok(status) = result {
            assert!(status.get("internet_reachable").is_some());
            assert!(status.get("provider_endpoints").is_some());
        }
    }

    #[tokio::test]
    async fn test_search_web_rejects_empty_query() {
        let result = search_web("").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_html_unescape() {
        assert_eq!(html_unescape("hello &amp; world"), "hello & world");
        assert_eq!(html_unescape("&lt;tag&gt;"), "<tag>");
        assert_eq!(html_unescape("&quot;quote&quot;"), "\"quote\"");
    }

    #[test]
    fn test_parse_duckduckgo_lite_empty() {
        let results = parse_duckduckgo_lite("<html></html>");
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_duckduckgo_lite_with_results() {
        let html = r#"<a href="https://example.com" class="result-link">Example</a>
                      <p class="result-snippet">An example site</p>"#;
        let results = parse_duckduckgo_lite(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example");
        assert_eq!(results[0].url, "https://example.com");
    }
}
