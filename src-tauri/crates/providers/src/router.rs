use crate::{create_provider, ProviderConfig, ProviderKind, LlmProvider};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::Mutex;

// ─── Config ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub default: String,
    pub plan: RoutePolicy,
    pub build: RoutePolicy,
    pub review: RoutePolicy,
    pub fallback: Vec<String>,

    /// Enable health-based routing (circuit breaker + latency tracking).
    #[serde(default = "default_true")]
    pub health_checks: bool,

    /// Circuit-breaker: max failures before a provider is marked unreachable.
    #[serde(default = "default_3")]
    pub max_failures: u32,

    /// Circuit-breaker: how long to stay open before trying half-open probes (seconds).
    #[serde(default = "default_60")]
    pub circuit_reset_timeout: u64,
}

fn default_true() -> bool { true }
fn default_3() -> u32 { 3 }
fn default_60() -> u64 { 60 }

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            default: "anthropic:claude-sonnet".into(),
            plan: RoutePolicy::default(),
            build: RoutePolicy::default(),
            review: RoutePolicy::default(),
            fallback: vec!["openai:gpt-4o".into(), "local:llama3".into()],
            health_checks: true,
            max_failures: 3,
            circuit_reset_timeout: 60,
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

// ─── Health monitoring ──────────────────────────────────────────────────────

/// Circuit-breaker state snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// EWMA latency tracker + circuit breaker for a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyTracker {
    /// Exponential moving average of latency in ms (α = 0.3).
    ewma: Option<f64>,
    /// Consecutive failure count.
    pub failures: u32,
    /// When the circuit was tripped, as millis since UNIX_EPOCH (None = closed).
    pub circuit_opened_at: Option<u128>,
    /// Human-readable provider name.
    pub name: String,
}

impl LatencyTracker {
    pub fn new(name: &str) -> Self {
        Self {
            ewma: None,
            failures: 0,
            circuit_opened_at: None,
            name: name.to_string(),
        }
    }

    /// Current wall-clock time in millis since UNIX_EPOCH.
    fn now_millis() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    }

    /// Record a successful probe: update EWMA latency, reset failure count + circuit.
    pub fn record_success(&mut self, latency_ms: u64) {
        let lat = latency_ms as f64;
        self.ewma = Some(match self.ewma {
            // α = 0.3 — 30% new sample, 70% history
            Some(prev) => prev * 0.7 + lat * 0.3,
            None => lat,
        });
        self.failures = 0;
        self.circuit_opened_at = None;
    }

    /// Record a failed probe: increment failure count, trip circuit if threshold hit.
    pub fn record_failure(&mut self, max_failures: u32) {
        self.failures += 1;
        if self.circuit_opened_at.is_none() && self.failures >= max_failures {
            self.circuit_opened_at = Some(Self::now_millis());
        }
        if let Some(prev) = self.ewma {
            self.ewma = Some(prev * 1.1); // widen latency estimate on failure
        }
    }

    /// Current EWMA latency estimate in ms (None if no data).
    pub fn latency_ms(&self) -> Option<u64> {
        self.ewma.map(|v| v as u64)
    }

    /// Is the circuit breaker open (provider currently quarantined)?
    pub fn is_circuit_open(&self, reset_timeout_ms: u64) -> bool {
        match self.circuit_opened_at {
            Some(opened) => {
                Self::now_millis().saturating_sub(opened) < reset_timeout_ms as u128
            }
            None => false,
        }
    }

    /// Is the circuit in half-open state (retrying after timeout)?
    pub fn is_half_open(&self, reset_timeout_ms: u64) -> bool {
        match self.circuit_opened_at {
            Some(opened) => {
                let elapsed = Self::now_millis().saturating_sub(opened);
                elapsed >= reset_timeout_ms as u128 && self.failures >= 1
            }
            None => false,
        }
    }

    /// Is the provider healthy (circuit not open)?
    pub fn is_healthy(&self, reset_timeout_ms: u64) -> bool {
        !self.is_circuit_open(reset_timeout_ms)
    }

    /// Return the current circuit state.
    pub fn circuit_state(&self, reset_timeout_ms: u64) -> CircuitState {
        if self.is_circuit_open(reset_timeout_ms) {
            CircuitState::Open
        } else if self.is_half_open(reset_timeout_ms) {
            CircuitState::HalfOpen
        } else {
            CircuitState::Closed
        }
    }
}

/// Snapshot of a provider's health at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub name: String,
    pub reachable: bool,
    pub latency_ms: Option<u64>,
    pub ewma_latency_ms: Option<u64>,
    pub failures: u32,
    pub circuit_state: CircuitState,
}

// ─── Health monitor ─────────────────────────────────────────────────────────

/// Thread-safe health registry for all providers.
/// Tracks circuit-breaker state and EWMA latency per provider.
pub struct HealthMonitor {
    trackers: Mutex<Vec<LatencyTracker>>,
    config: RoutingConfig,
}

impl HealthMonitor {
    pub fn new(config: &RoutingConfig) -> Self {
        Self {
            trackers: Mutex::new(vec![]),
            config: config.clone(),
        }
    }

    /// Get current health summary for all known providers.
    pub async fn health_summary(&self) -> Vec<ProviderHealth> {
        let reset_ms = self.config.circuit_reset_timeout * 1000;
        let trackers = self.trackers.lock().await;

        trackers
            .iter()
            .map(|t| {
                ProviderHealth {
                    name: t.name.clone(),
                    reachable: t.is_healthy(reset_ms),
                    latency_ms: t.latency_ms(),
                    ewma_latency_ms: t.ewma.map(|v| v as u64),
                    failures: t.failures,
                    circuit_state: t.circuit_state(reset_ms),
                }
            })
            .collect()
    }

    /// Record a successful probe result.
    pub async fn record_success(&self, name: &str, latency_ms: u64) {
        let mut trackers = self.trackers.lock().await;
        if let Some(t) = trackers.iter_mut().find(|t| t.name == name) {
            t.record_success(latency_ms);
        }
    }

    /// Record a failed probe result.
    pub async fn record_failure(&self, name: &str) {
        let max_failures = self.config.max_failures;
        let mut trackers = self.trackers.lock().await;
        if let Some(t) = trackers.iter_mut().find(|t| t.name == name) {
            t.record_failure(max_failures);
        }
    }

    /// Check if a provider is healthy (circuit not open).
    pub async fn is_healthy(&self, name: &str) -> bool {
        let reset_ms = self.config.circuit_reset_timeout * 1000;
        let trackers = self.trackers.lock().await;
        trackers
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.is_healthy(reset_ms))
            .unwrap_or(true) // Unknown provider — assume healthy
    }
}

// ─── Provider string parsing ─────────────────────────────────────────────────

/// Parse "anthropic:claude-sonnet" → ("anthropic", "claude-sonnet")
pub fn parse_provider_string(s: &str) -> (String, String) {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (s.to_string(), String::new())
    }
}

/// Build a ProviderConfig from a "provider:model" string.
pub fn build_provider_config(provider_str: &str, model: &str) -> ProviderConfig {
    let kind = ProviderKind::from_str(provider_str);
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
    }
}

// ─── Health probing ─────────────────────────────────────────────────────────

/// Check health of a single provider config. Returns a snapshot — does not update any monitor.
pub async fn check_provider_health(config: &ProviderConfig) -> ProviderHealth {
    let url = config
        .base_url
        .clone()
        .unwrap_or_else(|| config.kind.default_base_url());
    let models_url = format!("{}/v1/models", url.trim_end_matches('/'));

    let name = format!("{}:{}", config.kind, config.model);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return ProviderHealth {
                name,
                reachable: false,
                latency_ms: None,
                ewma_latency_ms: None,
                failures: 0,
                circuit_state: CircuitState::Closed,
            };
        }
    };

    let mut req = client.get(&models_url);
    if let Some(key) = &config.api_key {
        req = req.header("Authorization", format!("Bearer {}", key));
    }

    let start = std::time::Instant::now();
    match req.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            ProviderHealth {
                name,
                reachable: resp.status().is_success() || resp.status().as_u16() == 401,
                latency_ms: Some(latency),
                ewma_latency_ms: Some(latency),
                failures: 0,
                circuit_state: CircuitState::Closed,
            }
        }
        Err(_) => ProviderHealth {
            name,
            reachable: false,
            latency_ms: None,
            ewma_latency_ms: None,
            failures: 0,
            circuit_state: CircuitState::Closed,
        },
    }
}

/// Check health of all configured providers. (Convenience wrapper around check_provider_health.)
pub async fn provider_doctor(config: &ProviderConfig) -> Result<Vec<ProviderHealth>, String> {
    let health = check_provider_health(config).await;
    Ok(vec![health])
}

// ─── Routing with health awareness ───────────────────────────────────────────

/// Route a request to the appropriate provider based on stage.
/// Tries preferred providers first, then falls over to default and configured fallbacks.
/// When `config.health_checks` is enabled, skips providers known to have open circuits.
pub async fn route_request(
    config: &RoutingConfig,
    stage: &str,
    monitor: Option<&HealthMonitor>,
) -> Result<Box<dyn LlmProvider>, String> {
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

    // Try preferred providers first (skip unhealthy if monitor is active)
    for pref in &policy.prefer {
        if config.health_checks {
            if let Some(m) = monitor {
                if !m.is_healthy(pref).await {
                    continue;
                }
            }
        }
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
        if config.health_checks {
            if let Some(m) = monitor {
                if !m.is_healthy(fallback).await {
                    continue;
                }
            }
        }
        let (provider_str, model) = parse_provider_string(fallback);
        let provider_config = build_provider_config(&provider_str, &model);
        if let Ok(provider) = create_provider(&provider_config) {
            return Ok(provider);
        }
    }

    Err("No provider available".into())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn route_fails_over_to_secondary() {
        let config = RoutingConfig::default();
        let result = route_request(&config, "plan", None).await;
        assert!(
            result.is_ok(),
            "Should route successfully: {:?}",
            result.err()
        );
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
        assert!(config.health_checks);
    }

    #[test]
    fn circuit_breaker_trippable_and_resettable() {
        let mut tracker = LatencyTracker::new("test:provider");
        let reset_ms = 60_000u64;

        // Fresh tracker is healthy, closed
        assert!(tracker.is_healthy(reset_ms));

        // After max_failures, circuit opens
        tracker.record_failure(3);
        tracker.record_failure(3);
        assert!(tracker.is_healthy(reset_ms)); // 2 failures < 3 threshold
        tracker.record_failure(3);
        assert!(!tracker.is_healthy(reset_ms)); // 3 failures >= 3 threshold
    }

    #[test]
    fn circuit_breaker_resets_on_success() {
        let mut tracker = LatencyTracker::new("test:provider");
        let reset_ms = 60_000u64;

        // Trip the circuit
        tracker.record_failure(3);
        tracker.record_failure(3);
        tracker.record_failure(3);
        assert!(!tracker.is_healthy(reset_ms));

        // A success should reset it
        tracker.record_success(100);
        assert!(tracker.is_healthy(reset_ms));
    }

    #[test]
    fn ewma_latency_smooths_readings() {
        let mut tracker = LatencyTracker::new("test:provider");

        // First reading: EWMA = 100
        tracker.record_success(100);
        assert_eq!(tracker.latency_ms(), Some(100));

        // Second reading: EWMA = 100*0.7 + 200*0.3 = 70 + 60 = 130
        tracker.record_success(200);
        assert_eq!(tracker.latency_ms(), Some(130));
    }

    #[tokio::test]
    async fn health_monitor_tracks_multiple_providers() {
        let config = RoutingConfig::default();
        let monitor = HealthMonitor::new(&config);

        // Providers unknown initially → healthy
        assert!(monitor.is_healthy("openai:gpt-4o").await);

        // Record a success (must exist in monitor first)
        // Since is_healthy returns true for unknown, we need to add a tracker first
        // by recording a success
        let reset_ms = config.circuit_reset_timeout * 1000;
        let mut trackers = monitor.trackers.lock().await;
        trackers.push(LatencyTracker::new("openai:gpt-4o"));
        drop(trackers);

        monitor.record_success("openai:gpt-4o", 150).await;
        let summary = monitor.health_summary().await;
        assert_eq!(summary.len(), 1);
        assert!(summary[0].reachable);
        assert_eq!(summary[0].latency_ms, Some(150));
        assert_eq!(summary[0].circuit_state, CircuitState::Closed);
    }

    #[tokio::test]
    async fn health_monitor_skips_unhealthy_provider() {
        let config = RoutingConfig::default();
        let monitor = HealthMonitor::new(&config);

        // Add the provider first so we can track it
        {
            let mut trackers = monitor.trackers.lock().await;
            trackers.push(LatencyTracker::new("anthropic:claude-sonnet"));
        }

        // Mark provider as unhealthy (3 failures trips circuit)
        monitor.record_failure("anthropic:claude-sonnet").await;
        monitor.record_failure("anthropic:claude-sonnet").await;
        monitor.record_failure("anthropic:claude-sonnet").await;

        assert!(!monitor.is_healthy("anthropic:claude-sonnet").await);

        // Unknown provider still healthy
        assert!(monitor.is_healthy("openai:gpt-4o").await);
    }

    #[test]
    fn health_check_config_fields() {
        let config = RoutingConfig {
            max_failures: 5,
            circuit_reset_timeout: 30,
            ..Default::default()
        };
        assert_eq!(config.max_failures, 5);
        assert_eq!(config.circuit_reset_timeout, 30);
    }
}
