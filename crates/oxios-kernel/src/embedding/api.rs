//! API-based embedding provider (OpenAI-compatible).
//!
//! Calls a remote embedding endpoint (e.g. `POST /v1/embeddings`) and returns
//! dense `f32` vectors suitable for sqlite-vec KNN and HNSW ANN search.
//!
//! ## Why this exists
//!
//! The default `TfIdfEmbeddingProvider` produces sparse vectors that
//! `EmbeddingVector::to_f32_dense()` cannot convert to f32 (`embedding.rs:99`
//! returns `None` for `Sparse`), so `SqliteMemoryStore::remember()` silently
//! skips the vector insert — `memory_vectors_rowids` stays empty.
//!
//! `GgufEmbeddingProvider` (feature `embedding-gguf`) is aarch64-only and
//! requires a 329MB model download.
//!
//! API embeddings: zero-dep (reqwest already in tree), cross-platform, and
//! the user already has API keys configured for LLM providers.
//!
//! ## Config
//!
//! ```toml
//! [embedding]
//! endpoint = "https://api.openai.com/v1/embeddings"
//! api_key  = ""               # empty → inherit from active LLM provider
//! model    = "text-embedding-3-small"
//! # dimensions = 1536         # optional; defaults per model
//! ```
//!
//! ## Failure handling
//!
//! Network errors / non-2xx responses return `Err`. Callers in the write path
//! (`SqliteMemoryStore::remember`) must treat this as non-fatal — store the
//! text+FTS5 row and skip the vector. See Phase 2b in the design doc.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::embedding::{EmbeddingProvider, EmbeddingVector};

/// Default well-known models and their output dimensions.
///
/// Used when the user does not specify `dimensions` in config.
pub fn default_dimensions(model: &str) -> Option<usize> {
    match model {
        "text-embedding-3-small" => Some(1536),
        "text-embedding-3-large" => Some(3072),
        "text-embedding-ada-002" => Some(1536),
        _ => None,
    }
}

/// HTTP client wrapper (single connection pool, configurable timeout).
#[derive(Debug)]
struct ApiClient {
    inner: reqwest::Client,
}

impl ApiClient {
    fn new() -> Result<Self> {
        let inner = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .context("Failed to construct reqwest client for embedding API")?;
        Ok(Self { inner })
    }
}

/// Request body for `POST /v1/embeddings` (OpenAI-compatible).
#[derive(Debug, Serialize)]
struct EmbeddingRequest<'a> {
    input: &'a str,
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

/// Response body for `POST /v1/embeddings`.
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    /// List of embeddings, one per input. We send a single input.
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

/// API-based embedding provider.
///
/// Created at boot from `[embedding]` config. If config is unset, this
/// provider is never instantiated and `TfIdfEmbeddingProvider` is used.
pub struct ApiEmbeddingProvider {
    /// HTTP endpoint, e.g. `https://api.openai.com/v1/embeddings`.
    endpoint: String,
    /// API key (Bearer token).
    api_key: String,
    /// Model name, e.g. `text-embedding-3-small`.
    model: String,
    /// Output dimensionality. Required for sqlite-vec table creation.
    /// Resolved from config or `default_dimensions()`.
    dimensions: usize,
    /// HTTP client (lazy-initialized via Mutex on first use to avoid
    /// propagating Client build errors through construction).
    client: Mutex<Option<Arc<ApiClient>>>,
}

impl std::fmt::Debug for ApiEmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiEmbeddingProvider")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("dimensions", &self.dimensions)
            .finish_non_exhaustive()
    }
}

impl ApiEmbeddingProvider {
    /// Construct a new provider from the resolved config fields.
    ///
    /// Returns `Err` if `dimensions` cannot be determined.
    pub fn new(
        endpoint: String,
        api_key: String,
        model: String,
        dimensions: Option<usize>,
    ) -> Result<Self> {
        let dimensions = match dimensions {
            Some(d) if d > 0 => d,
            _ => default_dimensions(&model)
                .ok_or_else(|| anyhow::anyhow!(
                    "Embedding model '{model}' has no known dimensionality; specify `dimensions` in [embedding] config"
                ))?,
        };
        Ok(Self {
            endpoint,
            api_key,
            model,
            dimensions,
            client: Mutex::new(None),
        })
    }

    /// Get or lazily build the HTTP client.
    fn http(&self) -> Result<Arc<ApiClient>> {
        let mut guard = self.client.lock();
        if let Some(c) = guard.as_ref() {
            return Ok(Arc::clone(c));
        }
        let c = Arc::new(ApiClient::new()?);
        *guard = Some(Arc::clone(&c));
        Ok(c)
    }
}

#[async_trait]
impl EmbeddingProvider for ApiEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<EmbeddingVector> {
        let client = self.http()?;
        let req = EmbeddingRequest {
            input: text,
            model: &self.model,
            dimensions: None, // most providers ignore or default to model size
        };
        let resp = client
            .inner
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .context("Embedding API request failed")?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .context("Failed to read embedding API response body")?;

        if !status.is_success() {
            anyhow::bail!(
                "Embedding API returned HTTP {status}: {}",
                truncate(&body, 200)
            );
        }

        let parsed: EmbeddingResponse = serde_json::from_str(&body)
            .context("Failed to parse embedding API response as JSON")?;

        let vec = parsed
            .data
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Embedding API returned no vectors in response"))?
            .embedding;

        if vec.len() != self.dimensions {
            anyhow::bail!(
                "Embedding dimension mismatch: configured {} got {}",
                self.dimensions,
                vec.len()
            );
        }

        Ok(EmbeddingVector::DenseF32(vec))
    }

    fn name(&self) -> &str {
        "api-embedding"
    }
}

/// Truncate a string for error messages without panicking on UTF-8 boundaries.
fn truncate(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_dimensions() {
        assert_eq!(default_dimensions("text-embedding-3-small"), Some(1536));
        assert_eq!(default_dimensions("text-embedding-3-large"), Some(3072));
        assert_eq!(default_dimensions("unknown-model"), None);
    }

    #[test]
    fn test_truncate_ascii() {
        assert_eq!(truncate("hello world", 5), "hello...");
    }

    #[test]
    fn test_truncate_korean_safe_boundary() {
        // "가나다라마" is 10 bytes; truncate at 7 should not panic
        let s = "가나다라마";
        let _ = truncate(s, 7);
    }

    #[test]
    fn test_constructor_requires_known_dim() {
        let p = ApiEmbeddingProvider::new(
            "https://example.com".into(),
            "k".into(),
            "text-embedding-3-small".into(),
            None,
        )
        .unwrap();
        assert_eq!(p.dimensions, 1536);
    }

    #[test]
    fn test_constructor_explicit_dim() {
        let p = ApiEmbeddingProvider::new(
            "https://example.com".into(),
            "k".into(),
            "custom-model".into(),
            Some(768),
        )
        .unwrap();
        assert_eq!(p.dimensions, 768);
    }

    #[test]
    fn test_constructor_unknown_model_no_dim_fails() {
        let r = ApiEmbeddingProvider::new(
            "https://example.com".into(),
            "k".into(),
            "custom-model".into(),
            None,
        );
        assert!(r.is_err());
    }
}
