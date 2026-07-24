use crate::{AppState, MutexExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RouterStatus {
    pub current_provider: String,
    pub fallback_providers: Vec<String>,
    pub health: Vec<providers::ProviderHealth>,
}

pub async fn get_router_status(state: &AppState) -> Result<RouterStatus, String> {
    let config = state.provider_config.lock_guard().clone();
    let routing = providers::RoutingConfig::default();

    let health = providers::provider_doctor(&config).await.unwrap_or_default();

    Ok(RouterStatus {
        current_provider: format!("{}:{}", config.kind, config.model),
        fallback_providers: routing.fallback,
        health,
    })
}

pub async fn run_provider_doctor(state: &AppState) -> Result<String, String> {
    let config = state.provider_config.lock_guard().clone();
    let health = providers::provider_doctor(&config).await?;

    let mut report = String::from("Provider Health Report:\n");
    for h in &health {
        let status = if h.reachable { "✓ reachable" } else { "✗ unreachable" };
        let latency = h.latency_ms.map(|ms| format!(" ({}ms)", ms)).unwrap_or_default();
        report.push_str(&format!("  {}: {}{}\n", h.name, status, latency));
    }

    Ok(report)
}
