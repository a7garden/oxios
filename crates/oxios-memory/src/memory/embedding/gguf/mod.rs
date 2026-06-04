#![allow(missing_docs)]
//! GGUF-based embedding provider (cross-platform).
//!
//! Uses `llama-gguf` crate to load EmbeddingGemma-300m GGUF model
//! and extract dense embeddings for semantic search.
//!
//! ## Lifecycle
//! 1. First `embed()` call downloads model (~329MB) and loads it (~1-3s)
//! 2. Model stays in memory for subsequent calls (~5-15ms each)
//! 3. After `model_ttl_secs` of inactivity, model is automatically unloaded
//! 4. Next call reloads the model
//!
//! ## Feature flag
//! Requires `embedding-gguf` feature. Falls back to TF-IDF when disabled.

pub mod loader;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use llama_gguf::{
    backend::cpu::CpuBackend,
    gguf::GgufFile,
    model::{load_llama_model, EmbeddingConfig, EmbeddingExtractor, LlamaModel, PoolingStrategy},
    tokenizer::Tokenizer,
    HfClient,
};
use parking_lot::Mutex;

use super::{EmbeddingProvider, EmbeddingVector};

pub use self::loader::GgufModelLoader;
pub use self::loader::{MODEL_DISPLAY_NAME, MODEL_SIZE_MB};

/// Matryoshka dimension truncation.
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

/// Loaded model state (heavy — contains GGUF weights in memory).
struct LoadedModel {
    model: LlamaModel,
    tokenizer: Tokenizer,
    extractor: EmbeddingExtractor,
    loaded_at: Instant,
}

/// Lazy-loaded GGUF embedding provider.
///
/// Thread-safe: uses `Mutex` for the inner model state.
/// The model is loaded on first use and unloaded after TTL expires.
pub struct GgufEmbeddingProvider {
    /// Directory where model files are stored.
    model_dir: PathBuf,
    /// Output embedding dimension (Matryoshka truncation).
    dimension: EmbeddingDimension,
    /// Inner model state (None = not loaded).
    inner: Mutex<Option<LoadedModel>>,
    /// Time-to-live for the loaded model.
    model_ttl: Duration,
    /// Last time the model was used.
    last_used: Mutex<Instant>,
}

impl std::fmt::Debug for GgufEmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GgufEmbeddingProvider")
            .field("model_dir", &self.model_dir)
            .field("dimension", &self.dimension)
            .field("model_ttl", &self.model_ttl)
            .finish()
    }
}

impl GgufEmbeddingProvider {
    /// Create a new GGUF embedding provider.
    ///
    /// The model is NOT loaded until the first `embed()` call.
    pub fn new(model_dir: PathBuf, dimension: EmbeddingDimension, model_ttl_secs: u64) -> Self {
        Self {
            model_dir,
            dimension,
            inner: Mutex::new(None),
            model_ttl: Duration::from_secs(model_ttl_secs),
            last_used: Mutex::new(Instant::now()),
        }
    }

    /// Create with default settings (256 dimensions, 5-minute TTL).
    pub fn with_defaults(model_dir: PathBuf) -> Self {
        Self::new(model_dir, EmbeddingDimension::default(), 300)
    }

    /// Ensure the model is loaded. Downloads + loads on first call.
    fn ensure_loaded(&self) -> Result<()> {
        {
            let inner = self.inner.lock();
            if inner.is_some() {
                return Ok(());
            }
        }

        // Download model if needed
        let gguf_path = GgufModelLoader::ensure_model(&self.model_dir)
            .context("Failed to download EmbeddingGemma GGUF model")?;

        // Load model + tokenizer
        let gguf = GgufFile::open(&gguf_path)
            .with_context(|| format!("Failed to open GGUF file: {}", gguf_path.display()))?;
        let model = load_llama_model(&gguf_path)
            .with_context(|| format!("Failed to load model from: {}", gguf_path.display()))?;
        let tokenizer =
            Tokenizer::from_gguf(&gguf).context("Failed to load tokenizer from GGUF")?;

        // Build embedding extractor (mean pooling + L2 normalize)
        let embed_config = EmbeddingConfig {
            layer: -1, // last layer
            pooling: PoolingStrategy::Mean,
            normalize: true,
            max_length: 512,
            ..EmbeddingConfig::default()
        };
        let extractor = EmbeddingExtractor::new(embed_config, model.config());

        let mut inner = self.inner.lock();
        *inner = Some(LoadedModel {
            model,
            tokenizer,
            extractor,
            loaded_at: Instant::now(),
        });

        tracing::info!(
            dir = %self.model_dir.display(),
            dim = self.dimension.size(),
            "GGUF EmbeddingGemma model loaded"
        );
        Ok(())
    }

    /// Encode a single text string into a dense embedding vector.
    ///
    /// Handles tokenization, forward pass, mean pooling, L2 normalization,
    /// and Matryoshka truncation.
    fn encode(&self, text: &str) -> Result<Vec<f32>> {
        let mut inner = self.inner.lock();
        let loaded = inner
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Model not loaded"))?;

        let backend = CpuBackend::new();
        let mut ctx = loaded.model.create_context(Arc::new(backend));

        let embedding = loaded
            .extractor
            .embed_text(&loaded.model, &loaded.tokenizer, &mut ctx, text)
            .map_err(|e| anyhow::anyhow!("Embedding extraction failed: {}", e))?;

        // Matryoshka truncation
        let dim = self.dimension.size();
        let truncated = if embedding.len() > dim {
            embedding[..dim].to_vec()
        } else {
            embedding
        };

        // Re-normalize after truncation
        let norm: f32 = truncated.iter().map(|x| x * x).sum::<f32>().sqrt();
        let result = if norm > 1e-10 {
            truncated.iter().map(|x| x / norm).collect()
        } else {
            truncated
        };

        Ok(result)
    }

    /// Unload the model if TTL has expired.
    pub fn maybe_unload(&self) {
        let mut inner = self.inner.lock();
        if let Some(ref loaded) = *inner {
            if loaded.loaded_at.elapsed() > self.model_ttl {
                *inner = None;
                tracing::debug!("GGUF embedding model unloaded (TTL expired)");
            }
        }
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
impl EmbeddingProvider for GgufEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<EmbeddingVector> {
        self.ensure_loaded()?;
        *self.last_used.lock() = Instant::now();

        let vec = self.encode(text)?;
        Ok(EmbeddingVector::DenseF32(vec))
    }

    fn name(&self) -> &str {
        "gguf-embeddinggemma-300m"
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
        let provider =
            GgufEmbeddingProvider::with_defaults(PathBuf::from("/tmp/test-models/embedding"));
        assert_eq!(provider.dimension(), 256);
        assert_eq!(provider.name(), "gguf-embeddinggemma-300m");
    }

    #[test]
    fn test_provider_debug() {
        let provider = GgufEmbeddingProvider::with_defaults(PathBuf::from("/tmp/test"));
        let debug_str = format!("{:?}", provider);
        assert!(debug_str.contains("GgufEmbeddingProvider"));
        assert!(debug_str.contains("Dim256"));
    }

    #[test]
    fn test_maybe_unload_noop_when_not_loaded() {
        let provider = GgufEmbeddingProvider::with_defaults(PathBuf::from("/tmp/test"));
        provider.maybe_unload(); // Should not panic
    }

    // Integration test — requires model download (~329MB)
    #[tokio::test]
    #[ignore = "requires model download (~329MB)"]
    async fn test_embed_produces_dense_vector() {
        let dir = dirs::home_dir()
            .unwrap()
            .join(".oxios")
            .join("models")
            .join("embeddinggemma-300m");
        let provider = GgufEmbeddingProvider::new(dir, EmbeddingDimension::Dim256, 300);

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
    #[ignore = "requires model download (~329MB)"]
    async fn test_embed_korean() {
        let dir = dirs::home_dir()
            .unwrap()
            .join(".oxios")
            .join("models")
            .join("embeddinggemma-300m");
        let provider = GgufEmbeddingProvider::new(dir, EmbeddingDimension::Dim256, 300);

        let vec = provider.embed("한국어 임베딩 테스트").await.unwrap();
        if let EmbeddingVector::DenseF32(v) = vec {
            assert_eq!(v.len(), 256);
            assert!(v.iter().any(|&x| x != 0.0), "Should not be all zeros");
        }
    }
}
