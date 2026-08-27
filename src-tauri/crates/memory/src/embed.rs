const NGRAM_DIMENSION: usize = 256;

/// Trait for embedding engines that produce fixed-dimension float vectors
/// and support cosine similarity.
pub trait Embedder: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>, String>;
    fn similarity(&self, a: &[f32], b: &[f32]) -> Result<f64, String>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
}

/// Character n-gram embedding engine (no dependencies, 256-dim).
/// Used as the default embedder unless the `onnx-embed` feature is enabled.
pub struct EmbeddingEngine;

impl EmbeddingEngine {
    pub fn new() -> Self {
        Self
    }

    /// Generate a fixed-dimension embedding vector from text using character n-grams.
    /// Maps n-gram hashes to a NGRAM_DIMENSION-sized vector for cosine similarity.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let mut vec = vec![0.0f32; NGRAM_DIMENSION];
        let lower = text.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();

        if chars.is_empty() {
            return Ok(vec);
        }

        // Bigrams
        for window in chars.windows(2) {
            let hash = Self::hash_ngram(&[window[0], window[1]]);
            let idx = (hash as usize) % NGRAM_DIMENSION;
            vec[idx] += 1.0;
        }

        // Trigrams
        for window in chars.windows(3) {
            let hash = Self::hash_ngram(&[window[0], window[1], window[2]]);
            let idx = (hash as usize) % NGRAM_DIMENSION;
            vec[idx] += 1.0;
        }

        // Whole-word unigrams for emphasis
        for word in lower.split_whitespace() {
            if !word.is_empty() {
                let hash = Self::hash_ngram(&word.chars().collect::<Vec<_>>());
                let idx = (hash as usize) % NGRAM_DIMENSION;
                vec[idx] += 1.0;
            }
        }

        // L2 normalize
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
            return Err(format!("Dimension mismatch: {} vs {}", a.len(), b.len()));
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

impl Default for EmbeddingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Embedder for EmbeddingEngine {
    fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        self.embed(text)
    }

    fn similarity(&self, a: &[f32], b: &[f32]) -> Result<f64, String> {
        self.similarity(a, b)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        self.embed_batch(texts)
    }
}

// ── Optional ONNX-based semantic embeddings ───────────────────────────────────

#[cfg(feature = "onnx-embed")]
pub struct ONNXEmbeddingEngine {
    session: std::sync::Mutex<ort::session::Session>,
    tokenizer: tokenizers::Tokenizer,
    max_length: usize,
}

#[cfg(feature = "onnx-embed")]
impl ONNXEmbeddingEngine {
    /// Create a new ONNX embedding engine from a model file and a HuggingFace
    /// tokenizer JSON file.
    ///
    /// # Arguments
    /// * `model_path` — Path to the ONNX model (e.g. `all-MiniLM-L6-v2.onnx`)
    /// * `tokenizer_path` — Path to a tokenizer JSON file (e.g. `tokenizer.json`)
    /// * `max_length` — Maximum token sequence length (typically 128 or 256)
    pub fn new(model_path: &str, tokenizer_path: &str, max_length: usize) -> Result<Self, String> {
        let session = ort::session::Session::builder()
            .map_err(|e| format!("Failed to create ONNX session builder: {}", e))?
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load ONNX model `{}`: {}", model_path, e))?;

        let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer from `{}`: {}", tokenizer_path, e))?;

        Ok(Self {
            session: std::sync::Mutex::new(session),
            tokenizer,
            max_length,
        })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let (ids, mask, seq_len) = self.tokenize(text)?;

        let input_ids = ort::value::Tensor::from_array((vec![1, seq_len], ids))
            .map_err(|e| format!("Failed to create input_ids tensor: {}", e))?;
        let attention_mask = ort::value::Tensor::from_array((vec![1, seq_len], mask))
            .map_err(|e| format!("Failed to create attention_mask tensor: {}", e))?;

        let inputs = ort::inputs![
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
        ];

        let mut session = self
            .session
            .lock()
            .map_err(|e| format!("ONNX session lock poisoned: {}", e))?;
        let outputs = session
            .run(inputs)
            .map_err(|e| format!("ONNX inference failed: {}", e))?;

        // all-MiniLM-L6-v2 output is a single float tensor: (1, 384)
        let output_tensor = outputs[0]
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract ONNX output: {}", e))?;

        let mut vec: Vec<f32> = output_tensor.iter().copied().collect();

        // L2 normalize
        let mag: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if mag > 0.0 {
            for v in &mut vec {
                *v /= mag;
            }
        }

        Ok(vec)
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Tokenize text using the real HuggingFace tokenizer.
    /// Returns (input_ids, attention_mask, seq_len) with padding/truncation
    /// to `max_length`.
    fn tokenize(&self, text: &str) -> Result<(Vec<i64>, Vec<i64>, usize), String> {
        let mut encoding = self
            .tokenizer
            .encode(text.to_string(), true)
            .map_err(|e| format!("Tokenization failed: {}", e))?;

        // Truncate if needed
        if encoding.len() > self.max_length {
            encoding.truncate(
                0,
                self.max_length,
                tokenizers::utils::truncation::TruncationDirection::Right,
            );
        }

        // Pad to max_length
        let seq_len = encoding.len();
        let pad_len = self.max_length.saturating_sub(seq_len);

        let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&i| i as i64).collect();
        let mut mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .collect();

        // Pad with zeros
        ids.extend(std::iter::repeat(0).take(pad_len));
        mask.extend(std::iter::repeat(0).take(pad_len));

        Ok((ids, mask, self.max_length))
    }

    /// Cosine similarity between two vectors.
    pub fn similarity(&self, a: &[f32], b: &[f32]) -> Result<f64, String> {
        if a.len() != b.len() {
            return Err(format!("Dimension mismatch: {} vs {}", a.len(), b.len()));
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            return Ok(0.0);
        }

        Ok((dot / (mag_a * mag_b)) as f64)
    }
}

#[cfg(feature = "onnx-embed")]
impl Embedder for ONNXEmbeddingEngine {
    fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        ONNXEmbeddingEngine::embed(self, text)
    }

    fn similarity(&self, a: &[f32], b: &[f32]) -> Result<f64, String> {
        ONNXEmbeddingEngine::similarity(self, a, b)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        ONNXEmbeddingEngine::embed_batch(self, texts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_returns_correct_dimension() -> Result<(), String> {
        let engine = EmbeddingEngine::new();
        let vec = engine.embed("hello world")?;
        assert_eq!(vec.len(), NGRAM_DIMENSION);
        Ok(())
    }

    #[test]
    fn test_similar_vectors_high_similarity() -> Result<(), String> {
        let engine = EmbeddingEngine::new();
        let a = engine.embed("create a new user")?;
        let b = engine.embed("create a new account")?;
        let sim = engine.similarity(&a, &b)?;
        assert!(sim > 0.3, "Similarity too low: {}", sim);
        Ok(())
    }

    #[test]
    fn test_different_vectors_low_similarity() -> Result<(), String> {
        let engine = EmbeddingEngine::new();
        let a = engine.embed("delete all files")?;
        let b = engine.embed("hello world this is a test")?;
        let sim = engine.similarity(&a, &b)?;
        assert!(
            sim < 0.3,
            "truly different texts should not be similar: {}",
            sim
        );
        Ok(())
    }

    #[test]
    fn test_empty_text() -> Result<(), String> {
        let engine = EmbeddingEngine::new();
        let vec = engine.embed("")?;
        assert_eq!(vec.len(), NGRAM_DIMENSION);
        assert!(vec.iter().all(|&x| x == 0.0));
        Ok(())
    }

    #[test]
    fn test_same_text_perfect_similarity() -> Result<(), String> {
        let engine = EmbeddingEngine::new();
        let a = engine.embed("the quick brown fox")?;
        let b = engine.embed("the quick brown fox")?;
        let sim = engine.similarity(&a, &b)?;
        assert!(
            (sim - 1.0).abs() < 0.001,
            "Self-similarity should be ~1.0, got {}",
            sim
        );
        Ok(())
    }
}
