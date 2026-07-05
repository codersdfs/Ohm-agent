const DIMENSION: usize = 256;

pub struct EmbeddingEngine;

impl EmbeddingEngine {
    pub fn new() -> Self {
        Self
    }

    /// Generate a fixed-dimension embedding vector from text using character n-grams.
    /// Maps n-gram hashes to a DIMENSION-sized vector for cosine similarity.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let mut vec = vec![0.0f32; DIMENSION];
        let lower = text.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();

        if chars.is_empty() {
            return Ok(vec);
        }

        // Bigrams
        for window in chars.windows(2) {
            let hash = Self::hash_ngram(&[window[0], window[1]]);
            let idx = (hash as usize) % DIMENSION;
            vec[idx] += 1.0;
        }

        // Trigrams
        for window in chars.windows(3) {
            let hash = Self::hash_ngram(&[window[0], window[1], window[2]]);
            let idx = (hash as usize) % DIMENSION;
            vec[idx] += 1.5;
        }

        // Unigrams
        for &c in &chars {
            if c.is_alphanumeric() {
                let hash = c as u64;
                let idx = (hash as usize) % DIMENSION;
                vec[idx] += 0.5;
            }
        }

        // Normalize
        let mag: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if mag > 0.0 {
            for v in &mut vec {
                *v /= mag;
            }
        }

        Ok(vec)
    }

    /// Cosine similarity between two vectors.
    pub fn similarity(&self, a: &[f32], b: &[f32]) -> Result<f64, String> {
        if a.len() != b.len() {
            return Err("Dimension mismatch".into());
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            return Ok(0.0);
        }

        Ok((dot / (mag_a * mag_b)) as f64)
    }

    /// Embed a batch of texts and return their vectors.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    fn hash_ngram(chars: &[char]) -> u64 {
        let mut hash: u64 = 5381;
        for &c in chars {
            hash = hash.wrapping_mul(33).wrapping_add(c as u64);
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_returns_correct_dimension() {
        let engine = EmbeddingEngine::new();
        let vec = engine.embed("hello world").unwrap();
        assert_eq!(vec.len(), DIMENSION);
    }

    #[test]
    fn test_similar_vectors_high_similarity() {
        let engine = EmbeddingEngine::new();
        let a = engine.embed("create a new user").unwrap();
        let b = engine.embed("create a new account").unwrap();
        let sim = engine.similarity(&a, &b).unwrap();
        assert!(sim > 0.3, "Similarity too low: {}", sim);
    }

    #[test]
    fn test_different_vectors_low_similarity() {
        let engine = EmbeddingEngine::new();
        let a = engine.embed("delete all files").unwrap();
        let b = engine.embed("hello world this is a test").unwrap();
        let sim = engine.similarity(&a, &b).unwrap();
        assert!(sim < 0.8, "Similarity too high: {}", sim);
    }

    #[test]
    fn test_empty_text() {
        let engine = EmbeddingEngine::new();
        let vec = engine.embed("").unwrap();
        assert_eq!(vec.len(), DIMENSION);
        assert!(vec.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_same_text_perfect_similarity() {
        let engine = EmbeddingEngine::new();
        let a = engine.embed("the quick brown fox").unwrap();
        let b = engine.embed("the quick brown fox").unwrap();
        let sim = engine.similarity(&a, &b).unwrap();
        assert!((sim - 1.0).abs() < 0.001, "Self-similarity should be ~1.0, got {}", sim);
    }
}
