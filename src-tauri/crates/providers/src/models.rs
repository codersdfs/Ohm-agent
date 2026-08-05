//! Provider model fetching — moved out of `lib.rs` (P5 god-object split).

use crate::types::{ModelInfo, ProviderConfig, ProviderKind};

// ─── Model fetching ─────────────────────────────────────────────────────────

pub async fn fetch_models(config: &ProviderConfig) -> Result<Vec<ModelInfo>, String> {
    let base_url = config
        .base_url
        .clone()
        .unwrap_or_else(|| config.kind.default_base_url());

    if config.kind.is_openai_compatible() {
        fetch_openai_compatible_models(&base_url, config.api_key.as_deref()).await
    } else {
        match config.kind {
            ProviderKind::Local => match fetch_local_models(&base_url).await {
                Ok(models) if !models.is_empty() => Ok(models),
                _ => {
                    fetch_openai_compatible_models(
                        &format!("{}/v1", base_url.trim_end_matches('/')),
                        None,
                    )
                    .await
                }
            },
            ProviderKind::Google => fetch_google_models(&base_url, config.api_key.as_deref()).await,
            ProviderKind::Anthropic => {
                fetch_anthropic_models(&base_url, config.api_key.as_deref()).await
            }
            _ => fetch_openai_compatible_models(&base_url, config.api_key.as_deref()).await,
        }
    }
}

async fn fetch_openai_compatible_models(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModelInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let urls = vec![
        format!("{}/models", base_url.trim_end_matches('/')),
        format!("{}/v1/models", base_url.trim_end_matches('/')),
    ];

    for url in urls {
        let mut req = client.get(&url);
        if let Some(key) = api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let data: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Parse error: {}", e))?;
                let provider_name = extract_provider_name(&url);

                if let Some(models) = data.get("data").and_then(|d| d.as_array()) {
                    let mut result: Vec<ModelInfo> = models
                        .iter()
                        .filter_map(|m| {
                            let id = m.get("id").and_then(|v| v.as_str())?.to_string();
                            let name = m
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            Some(ModelInfo {
                                id,
                                name,
                                provider: provider_name.clone(),
                            })
                        })
                        .collect();
                    if !result.is_empty() {
                        result.sort_by(|a, b| a.id.cmp(&b.id));
                        return Ok(dedup_models(result));
                    }
                }
            }
            _ => continue,
        }
    }

    Err("No models endpoint responded".into())
}

async fn fetch_local_models(base_url: &str) -> Result<Vec<ModelInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "API error {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;
    let mut result = Vec::new();

    if let Some(models) = data.get("models").and_then(|d| d.as_array()) {
        for m in models {
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let model_name = m.get("model").and_then(|v| v.as_str());
            result.push(ModelInfo {
                id: name.to_string(),
                name: model_name.map(|s| s.to_string()),
                provider: "local".into(),
            });
        }
    }

    if result.is_empty() {
        return Err("No models found in Ollama response".into());
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

async fn fetch_google_models(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModelInfo>, String> {
    let key = api_key.ok_or_else(|| "API key required for Google provider".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!(
        "{}/v1beta/models?key={}",
        base_url.trim_end_matches('/'),
        key
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "API error {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;
    let mut result = Vec::new();

    if let Some(models) = data.get("models").and_then(|d| d.as_array()) {
        for m in models {
            if let Some(id) = m.get("name").and_then(|v| v.as_str()) {
                let name = m
                    .get("displayName")
                    .and_then(|v| v.as_str())
                    .or_else(|| m.get("description").and_then(|v| v.as_str()));
                result.push(ModelInfo {
                    id: id.to_string(),
                    name: name.map(|s| s.to_string()),
                    provider: "google".into(),
                });
            }
        }
    }

    if result.is_empty() {
        return Err("No models found in Google response".into());
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

async fn fetch_anthropic_models(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModelInfo>, String> {
    let key = api_key.ok_or_else(|| "API key required for Anthropic provider".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "API error {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;
    let mut result = Vec::new();

    if let Some(models) = data.get("data").and_then(|d| d.as_array()) {
        for m in models {
            if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
                let name = m
                    .get("name")
                    .and_then(|v| v.as_str())
                    .or_else(|| m.get("display_name").and_then(|v| v.as_str()));
                result.push(ModelInfo {
                    id: id.to_string(),
                    name: name.map(|s| s.to_string()),
                    provider: "anthropic".into(),
                });
            }
        }
    }

    if result.is_empty() {
        return Err("No models found in Anthropic response".into());
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

fn dedup_models(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    let mut seen = std::collections::HashSet::new();
    models
        .into_iter()
        .filter(|m| seen.insert(m.id.clone()))
        .collect()
}

fn extract_provider_name(url: &str) -> String {
    if url.contains("openai") {
        "openai".into()
    } else if url.contains("anthropic") {
        "anthropic".into()
    } else if url.contains("x.ai") || url.contains("xai") {
        "xai".into()
    } else if url.contains("cerebras") {
        "cerebras".into()
    } else if url.contains("groq") {
        "groq".into()
    } else if url.contains("moonshot") || url.contains("kimi") {
        "kimi".into()
    } else if url.contains("minimax") {
        "minimax".into()
    } else if url.contains("openrouter") {
        "openrouter".into()
    } else if url.contains("azure") {
        "azure".into()
    } else if url.contains("bedrock") {
        "bedrock".into()
    } else if url.contains("huggingface") {
        "huggingface".into()
    } else if url.contains("mistral") {
        "mistral".into()
    } else if url.contains("google") || url.contains("generativelanguage") {
        "google".into()
    } else {
        "unknown".into()
    }
}
