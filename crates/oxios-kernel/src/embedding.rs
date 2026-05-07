//! Embedding abstraction for semantic similarity.

use std::collections::HashMap;

use anyhow::Result;

/// An embedding vector for semantic similarity comparison.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum EmbeddingVector {
    /// Dense vector from API-based embeddings.
    Dense(Vec<f64>),
    /// Sparse TF-IDF vector (term → weight).
    Sparse(HashMap<String, f64>),
}

impl EmbeddingVector {
    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(&self, other: &Self) -> f64 {
        match (self, other) {
            (EmbeddingVector::Dense(a), EmbeddingVector::Dense(b)) => {
                if a.len() != b.len() || a.is_empty() {
                    return 0.0;
                }
                let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
                let na: f64 = a.iter().map(|v| v * v).sum::<f64>().sqrt();
                let nb: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
                if na == 0.0 || nb == 0.0 {
                    return 0.0;
                }
                dot / (na * nb)
            }
            (EmbeddingVector::Sparse(a), EmbeddingVector::Sparse(b)) => {
                if a.is_empty() || b.is_empty() {
                    return 0.0;
                }
                let mut dot = 0.0;
                for (term, w) in a {
                    if let Some(w2) = b.get(term) {
                        dot += w * w2;
                    }
                }
                let na: f64 = a.values().map(|v| v * v).sum::<f64>().sqrt();
                let nb: f64 = b.values().map(|v| v * v).sum::<f64>().sqrt();
                if na == 0.0 || nb == 0.0 {
                    return 0.0;
                }
                dot / (na * nb)
            }
            _ => 0.0, // Cross-type comparison not supported
        }
    }

    /// Returns true if this vector is empty/zero.
    pub fn is_empty(&self) -> bool {
        match self {
            EmbeddingVector::Dense(v) => v.is_empty(),
            EmbeddingVector::Sparse(m) => m.is_empty(),
        }
    }
}

/// Provider for generating text embeddings.
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding vector for the given text.
    async fn embed(&self, text: &str) -> Result<EmbeddingVector>;
    /// Name of this provider.
    fn name(&self) -> &str;
}

/// TF-IDF based embedding provider (zero dependencies).
pub struct TfIdfEmbeddingProvider;

#[async_trait::async_trait]
impl EmbeddingProvider for TfIdfEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<EmbeddingVector> {
        let tv = crate::memory::TextVector::from_text(text);
        Ok(EmbeddingVector::Sparse(tv.tf_map().clone()))
    }
    fn name(&self) -> &str {
        "tfidf"
    }
}
