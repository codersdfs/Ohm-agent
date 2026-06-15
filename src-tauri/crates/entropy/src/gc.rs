use chrono::Utc;

pub struct GarbageCollector;

impl GarbageCollector {
    pub fn new() -> Self {
        Self
    }

    pub async fn collect(&self) -> Result<String, String> {
        log::info!("GC running at {}", Utc::now());
        Ok("GC complete".into())
    }
}
