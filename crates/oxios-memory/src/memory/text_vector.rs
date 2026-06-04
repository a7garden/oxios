//! Simple TF-IDF text vector for language-agnostic semantic similarity.
//!
//! No external embedding model needed. Used by `TfIdfEmbeddingProvider`
//! and `MemoryManager`'s vector search.
//!
//! Moved from `oxios-kernel::memory::TextVector` in RFC-018 b.2 because
//! `TfIdfEmbeddingProvider` (also moved in b.2) depends on it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Simple TF-IDF vector for text similarity.
///
/// Tokenizes text into terms, computes normalized term frequency,
/// and supports cosine similarity comparison. No external embedding
/// model needed — language-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextVector {
    /// Term frequencies (normalized).
    tf: HashMap<String, f64>,
}

impl TextVector {
    /// Create a text vector from input text.
    pub fn from_text(text: &str) -> Self {
        let mut tf: HashMap<String, f64> = HashMap::new();
        let terms = Self::tokenize(text);
        let total = terms.len() as f64;

        for term in terms {
            *tf.entry(term).or_insert(0.0) += 1.0;
        }

        // Normalize by total term count
        if total > 0.0 {
            for v in tf.values_mut() {
                *v /= total;
            }
        }

        Self { tf }
    }

    /// Tokenize text into terms (language-agnostic).
    /// Splits on whitespace and punctuation, lowercases.
    /// Preserves non-ASCII alphanumeric runs (CJK, Hangul, etc.) within tokens.
    pub fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && !('\u{AC00}'..='\u{D7A3}').contains(&c))
            .filter(|s| !s.is_empty() && s.len() > 1)
            .map(|s| s.to_string())
            .collect()
    }

    /// Returns a reference to the term-frequency map.
    pub fn tf_map(&self) -> &HashMap<String, f64> {
        &self.tf
    }

    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(&self, other: &TextVector) -> f64 {
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for (term, &a) in &self.tf {
            norm_a += a * a;
            if let Some(&b) = other.tf.get(term) {
                dot += a * b;
            }
        }
        for &b in other.tf.values() {
            norm_b += b * b;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_vector_cosine_similarity() {
        let v1 = TextVector::from_text("fix the null pointer error in main.rs");
        let v2 = TextVector::from_text("null pointer error found in rust code");
        let v3 = TextVector::from_text("update the documentation for deployment");

        // Similar texts should have high similarity
        assert!(
            v1.cosine_similarity(&v2) > 0.3,
            "Similar texts should have > 0.3 similarity"
        );

        // Different texts should have low similarity
        assert!(
            v1.cosine_similarity(&v3) < 0.2,
            "Different texts should have < 0.2 similarity"
        );
    }

    #[test]
    fn test_text_vector_multilingual() {
        let v1 = TextVector::from_text("main.rs 파일의 null pointer 에러 수정");
        let v2 = TextVector::from_text("null pointer 오류를 수정했습니다");
        let v3 = TextVector::from_text("문서 업데이트 배포 가이드");

        assert!(v1.cosine_similarity(&v2) > 0.1, "Mixed script similarity");
        assert!(v1.cosine_similarity(&v3) < 0.1, "Different topics");
    }

    #[test]
    fn test_text_vector_empty() {
        let v1 = TextVector::from_text("");
        let v2 = TextVector::from_text("hello");
        assert_eq!(v1.cosine_similarity(&v2), 0.0);
    }

    #[test]
    fn test_text_vector_identical() {
        let v1 = TextVector::from_text("rust programming language");
        let v2 = TextVector::from_text("rust programming language");
        let sim = v1.cosine_similarity(&v2);
        assert!(
            (sim - 1.0).abs() < 1e-9,
            "Identical texts should have similarity ~1.0, got {}",
            sim
        );
    }

    #[test]
    fn test_tokenize_multilingual() {
        let terms = TextVector::tokenize("main.rs 파일의 버그를 수정");
        // Should contain at least some meaningful tokens
        assert!(!terms.is_empty(), "Non-ASCII text should produce tokens");
    }
}
