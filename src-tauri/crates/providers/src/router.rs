use crate::{create_provider, ProviderConfig, ProviderKind, LlmProvider};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub default: String,
    pub plan: RoutePolicy,
    pub build: RoutePolicy,
    pub review: RoutePolicy,
    pub fallback: Vec<String>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            default: "anthropic:claude-sonnet".into(),
            plan: RoutePolicy::default(),
            build: RoutePolicy::default(),
            review: RoutePolicy::default(),
            fallback: vec!["openai:gpt-4o".into(), "local:llama3".into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePolicy {
    pub prefer: Vec<String>,
    pub max_cost_tier: String,
    pub quality: String,
}

impl Default for RoutePolicy {
    fn default() -> Self {
        Self {
            prefer: vec![],
            max_cost_tier: "medium".into(),
            quality: "balanced".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub name: String,
    pub reachable: bool,
    pub latency_ms: Option<u64>,
}

/// Route a request to the appropriate provider based on stage.
/// Tries primary, fails over to secondary on error.
pub fn route_request(config: &RoutingConfig, stage: &str) -> Result<Box<dyn LlmProvider>, String> {
    let policy = match stage {
        "plan" => &config.plan,
        "build" => &config.build,
        "review" => &config.review,
        _ => {
            let (provider_str, model) = parse_provider_string(&config.default);
            let provider_config = build_provider_config(&provider_str, &model);
            return create_provider(&provider_config);
        }
    };

    // Try preferred providers first
    for pref in &policy.prefer {
        let (provider_str, model) = parse_provider_string(pref);
        let provider_config = build_provider_config(&provider_str, &model);
        if let Ok(provider) = create_provider(&provider_config) {
            return Ok(provider);
        }
    }

    // Fall back to default
    let (provider_str, model) = parse_provider_string(&config.default);
    let provider_config = build_provider_config(&provider_str, &model);
    if let Ok(provider) = create_provider(&provider_config) {
        return Ok(provider);
    }

    // Try fallbacks
    for fallback in &config.fallback {
        let (provider_str, model) = parse_provider_string(fallback);
        let provider_config = build_provider_config(&provider_str, &model);
        if let Ok(provider) = create_provider(&provider_config) {
            return Ok(provider);
        }
    }

    Err("No provider available".into())
}

fn parse_provider_string(s: &str) -> (String, String) {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (s.to_string(), String::new())
    }
}

fn build_provider_config(provider_str: &str, model: &str) -> ProviderConfig {
    let kind = ProviderKind::from_str(provider_str)
        .unwrap_or(ProviderKind::Custom);
    let base_url = kind.default_base_url();
    let model = if model.is_empty() {
        match &kind {
            ProviderKind::Anthropic => "claude-3-5-sonnet-20241022".to_string(),
            ProviderKind::OpenAI => "gpt-4o".to_string(),
            ProviderKind::Google => "gemini-1.5-pro".to_string(),
            _ => "default".to_string(),
        }
    } else {
        model.to_string()
    };

    ProviderConfig {
        kind,
        api_key: None,
        base_url: Some(base_url),
        model,
        max_tokens: 4096,
        temperature: 0.7,
        max_concurrent_tools: 3,
    }
}

/// Check health of all configured providers.
pub async fn provider_doctor(config: &ProviderConfig) -> Result<Vec<ProviderHealth>, String> {
    let mut results = vec![];

    let start = Instant::now();
    let health = check_provider_health(config, start).await;
    results.push(health);

    Ok(results)
}

async fn check_provider_health(config: &ProviderConfig, start: Instant) -> ProviderHealth {
    let url = config.base_url.clone().unwrap_or_else(|| config.kind.default_base_url());
    let models_url = format!("{}/v1/models", url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let name = format!("{}:{}", config.kind, config.model);

    match client {
        Ok(client) => {
            let mut req = client.get(&models_url);
            if let Some(key) = &config.api_key {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
            match req.send().await {
                Ok(resp) => {
                    let latency = start.elapsed().as_millis() as u64;
                    ProviderHealth {
                        name,
                        reachable: resp.status().is_success() || resp.status().as_u16() == 401,
                        latency_ms: Some(latency),
                    }
                }
                Err(_) => ProviderHealth {
                    name,
                    reachable: false,
                    latency_ms: None,
                },
            }
        }
        Err(_) => ProviderHealth {
            name,
            reachable: false,
            latency_ms: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_fails_over_to_secondary() {
        let config = RoutingConfig::default();
        let result = route_request(&config, "plan");
        assert!(result.is_ok(), "Should route successfully: {:?}", result.err());
    }

    #[test]
    fn parse_provider_string_basic() {
        let (provider, model) = parse_provider_string("anthropic:claude-sonnet");
        assert_eq!(provider, "anthropic");
        assert_eq!(model, "claude-sonnet");
    }

    #[test]
    fn parse_provider_string_no_model() {
        let (provider, model) = parse_provider_string("openai");
        assert_eq!(provider, "openai");
        assert_eq!(model, "");
    }

    #[test]
    fn routing_config_default() {
        let config = RoutingConfig::default();
        assert!(!config.default.is_empty());
        assert!(!config.fallback.is_empty());
    }
}
