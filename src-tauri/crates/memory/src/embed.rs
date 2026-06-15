pub struct EmbeddingEngine;

impl EmbeddingEngine {
    pub fn new() -> Self {
        Self
    }

    pub async fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
        Ok(vec![0.0f32; 384])
    }

    pub async fn similarity(&self, _a: &[f32], _b: &[f32]) -> Result<f64, String> {
        Ok(0.0)
    }
}
