use chrono::Utc;

pub struct DriftScanner;

impl DriftScanner {
    pub fn new() -> Self {
        Self
    }

    pub async fn scan(&self) -> Vec<String> {
        log::info!("Entropy scan starting at {}", Utc::now());
        vec![]
    }
}
