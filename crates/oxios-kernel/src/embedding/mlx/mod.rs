//! MLX-based embedding provider (Apple Silicon only).
//!
//! Provides `MlxEmbeddingProvider` — a lazy-loaded dense embedding model
//! backed by EmbeddingGemma-300m running via mlx-rs on Apple Silicon GPU.
//!
//! ## Lifecycle
//! 1. First `embed()` call triggers model download + load (~1-2s)
//! 2. Model stays in memory for subsequent calls (~5-15ms each)
//! 3. After `model_ttl_secs` of inactivity, model is automatically unloaded
//! 4. Next call reloads the model
//!
//! ## Feature flag
//! Requires `embedding-mlx` feature. Falls back to TF-IDF when disabled.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use parking_lot::Mutex;

use crate::embedding::{EmbeddingProvider, EmbeddingVector};

pub use self::gemma::{GemmaEmbeddingModel, GemmaModelArgs};
pub use self::loader::MlxModelLoader;
pub use self::pooler::{l2_normalize, mean_pool};

/// Matryoshka dimension truncation.
///
/// EmbeddingGemma supports truncating the output vector to smaller dimensions
/// without re-running the model, which reduces storage and improves search speed.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingDimension {
    /// 128 dimensions — smallest, fastest, slightly lower quality.
    Dim128,
    /// 256 dimensions — recommended balance.
    Dim256,
    /// 512 dimensions — higher quality.
    Dim512,
    /// 768 dimensions — full quality.
    Dim768,
}

impl EmbeddingDimension {
    /// Number of output dimensions.
    pub fn size(&self) -> usize {
        match self {
            Self::Dim128 => 128,
            Self::Dim256 => 256,
            Self::Dim512 => 512,
            Self::Dim768 => 768,
        }
    }
}

impl Default for EmbeddingDimension {
    fn default() -> Self {
        Self::Dim256
    }
}

/// HuggingFace model identifier.
const MODEL_ID: &str = "mlx-community/embeddinggemma-300m-4bit";

/// Loaded model state.
struct LoadedModel {
    model: GemmaEmbeddingModel,
    tokenizer: tokenizers::Tokenizer,
    loaded_at: Instant,
}

/// Lazy-loaded MLX embedding provider.
///
/// Thread-safe: uses `Mutex` for the inner model state.
/// The model is loaded on first use and unloaded after TTL expires.
pub struct MlxEmbeddingProvider {
    /// Directory where model files are stored.
    model_dir: PathBuf,
    /// Output embedding dimension (Matryoshka truncation).
    dimension: EmbeddingDimension,
    /// Query prefix for search queries.
    query_prefix: String,
    /// Document prefix for content to be embedded.
    doc_prefix: String,
    /// Inner model state (None = not loaded).
    inner: Mutex<Option<LoadedModel>>,
    /// Time-to-live for the loaded model. Unloaded after this duration of inactivity.
    model_ttl: Duration,
    /// Last time the model was used.
    last_used: Mutex<Instant>,
}

impl std::fmt::Debug for MlxEmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MlxEmbeddingProvider")
            .field("model_dir", &self.model_dir)
            .field("dimension", &self.dimension)
            .field("model_ttl", &self.model_ttl)
            .finish()
    }
}

impl MlxEmbeddingProvider {
    /// Create a new MLX embedding provider.
    ///
    /// The model is NOT loaded until the first `embed()` call.
    ///
    /// # Arguments
    /// * `model_dir` — Directory to store/load model files. Will be created if needed.
    /// * `dimension` — Matryoshka output dimension.
    /// * `model_ttl_secs` — Seconds before the model is unloaded due to inactivity.
    pub fn new(model_dir: PathBuf, dimension: EmbeddingDimension, model_ttl_secs: u64) -> Self {
        Self {
            model_dir,
            dimension,
            query_prefix: "task: search result | query: ".to_string(),
            doc_prefix: "title: none | text: ".to_string(),
            inner: Mutex::new(None),
            model_ttl: Duration::from_secs(model_ttl_secs),
            last_used: Mutex::new(Instant::now()),
        }
    }

    /// Create with default settings (256 dimensions, 5-minute TTL).
    pub fn with_defaults(model_dir: PathBuf) -> Self {
        Self::new(model_dir, EmbeddingDimension::default(), 300)
    }

    /// Ensure the model is loaded. Loads on first call, reloads after unload.
    fn ensure_loaded(&self) -> Result<()> {
        let mut inner = self.inner.lock();
        if inner.is_some() {
            return Ok(());
        }

        // Download model if needed
        MlxModelLoader::ensure_model(&self.model_dir)
            .context("Failed to download EmbeddingGemma model")?;

        // Load model + tokenizer
        let model = GemmaEmbeddingModel::load(&self.model_dir)
            .context("Failed to load Gemma embedding model")?;
        let tokenizer = MlxModelLoader::load_tokenizer(&self.model_dir)
            .context("Failed to load tokenizer")?;

        *inner = Some(LoadedModel {
            model,
            tokenizer,
            loaded_at: Instant::now(),
        });

        tracing::info!(
            dir = %self.model_dir.display(),
            dim = self.dimension.size(),
            "MLX EmbeddingGemma model loaded"
        );
        Ok(())
    }

    /// Unload the model if TTL has expired.
    pub fn maybe_unload(&self) {
        let mut inner = self.inner.lock();
        if let Some(ref loaded) = *inner {
            if loaded.loaded_at.elapsed() > self.model_ttl {
                *inner = None;
                tracing::debug!("MLX embedding model unloaded (TTL expired)");
            }
        }
    }

    /// Encode a single text string into a dense embedding vector.
    ///
    /// Handles tokenization, model forward pass, mean pooling,
    /// L2 normalization, and Matryoshka truncation.
    fn encode(&self, text: &str, prefix: &str) -> Result<Vec<f32>> {
        let mut inner = self.inner.lock();
        let loaded = inner
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Model not loaded"))?;

        let input = format!("{}{}", prefix, text);

        // Tokenize
        let encoding = loaded
            .tokenizer
            .encode(input, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let ids = encoding.get_ids();
        if ids.is_empty() {
            return Ok(vec![0.0; self.dimension.size()]);
        }

        // Forward pass
        let hidden = loaded.model.forward(ids)?;

        // Mean pooling + L2 normalize + Matryoshka truncation
        let dim = self.dimension.size();
        let pooled = mean_pool(&hidden, ids.len());
        let normalized = l2_normalize(&pooled);
        let result = normalized[..dim].to_vec();

        Ok(result)
    }

    /// Get the configured embedding dimension.
    pub fn dimension(&self) -> usize {
        self.dimension.size()
    }

    /// Get the model directory path.
    pub fn model_dir(&self) -> &PathBuf {
        &self.model_dir
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for MlxEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<EmbeddingVector> {
        self.ensure_loaded()?;
        *self.last_used.lock() = Instant::now();

        let vec = self.encode(text, &self.doc_prefix)?;
        Ok(EmbeddingVector::DenseF32(vec))
    }

    fn name(&self) -> &str {
        "mlx-embeddinggemma-300m"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_dimension_sizes() {
        assert_eq!(EmbeddingDimension::Dim128.size(), 128);
        assert_eq!(EmbeddingDimension::Dim256.size(), 256);
        assert_eq!(EmbeddingDimension::Dim512.size(), 512);
        assert_eq!(EmbeddingDimension::Dim768.size(), 768);
    }

    #[test]
    fn test_default_dimension() {
        assert_eq!(EmbeddingDimension::default().size(), 256);
    }

    #[test]
    fn test_provider_creation() {
        let provider = MlxEmbeddingProvider::with_defaults(
            PathBuf::from("/tmp/test-models/embedding"),
        );
        assert_eq!(provider.dimension(), 256);
        assert_eq!(provider.name(), "mlx-embeddinggemma-300m");
    }

    #[test]
    fn test_provider_debug() {
        let provider = MlxEmbeddingProvider::with_defaults(PathBuf::from("/tmp/test"));
        let debug_str = format!("{:?}", provider);
        assert!(debug_str.contains("MlxEmbeddingProvider"));
        assert!(debug_str.contains("Dim256"));
    }

    #[test]
    fn test_maybe_unload_noop_when_not_loaded() {
        let provider = MlxEmbeddingProvider::with_defaults(PathBuf::from("/tmp/test"));
        // Should not panic
        provider.maybe_unload();
    }

    // Integration test — requires model download (~173MB)
    #[tokio::test]
    #[ignore = "requires model download and Apple Silicon"]
    async fn test_embed_produces_dense_vector() {
        let dir = dirs::home_dir()
            .unwrap()
            .join(".oxios")
            .join("models")
            .join("embeddinggemma-300m-4bit");
        let provider = MlxEmbeddingProvider::new(dir, EmbeddingDimension::Dim256, 300);

        let vec = provider.embed("Rust programming language").await.unwrap();
        match vec {
            EmbeddingVector::DenseF32(v) => {
                assert_eq!(v.len(), 256, "Should produce 256-dim vector");
                // L2 normalized → should have unit norm
                let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                assert!((norm - 1.0).abs() < 0.01, "Should be L2 normalized");
            }
            _ => panic!("Expected DenseF32"),
        }
    }

    #[tokio::test]
    #[ignore = "requires model download and Apple Silicon"]
    async fn test_embed_korean() {
        let dir = dirs::home_dir()
            .unwrap()
            .join(".oxios")
            .join("models")
            .join("embeddinggemma-300m-4bit");
        let provider = MlxEmbeddingProvider::new(dir, EmbeddingDimension::Dim256, 300);

        let vec = provider.embed("한국어 임베딩 테스트").await.unwrap();
        if let EmbeddingVector::DenseF32(v) = vec {
            assert_eq!(v.len(), 256);
            assert!(v.iter().any(|&x| x != 0.0), "Should not be all zeros");
        }
    }
}
