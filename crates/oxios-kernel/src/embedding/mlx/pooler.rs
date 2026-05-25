//! Pooling and normalization for embedding vectors.
//!
//! Provides mean pooling (over token hidden states) and L2 normalization
//! for converting transformer outputs into fixed-size embedding vectors.

use mlx_rs::error::Exception;
use mlx_rs::ops::indexing::IndexOp;
use mlx_rs::Array;

/// Mean pooling over the sequence dimension.
///
/// Takes hidden states of shape `[1, seq_len, hidden_size]` and
/// averages across the `seq_len` dimension to produce `[hidden_size]`.
///
/// # Arguments
/// * `hidden` — Output from the model forward pass, shape `[1, seq_len, hidden_size]`
/// * `seq_len` — Number of tokens (used for simple mean, no attention mask weighting)
///
/// # Returns
/// A `Vec<f32>` of length `hidden_size`.
pub fn mean_pool(hidden: &Array, _seq_len: usize) -> Vec<f32> {
    // hidden shape: [1, seq_len, hidden_size]
    // Mean over axis 1 (seq_len) → [1, hidden_size]
    let pooled = mlx_rs::ops::mean(hidden, Some(&[1]), None).unwrap();

    // Evaluate to materialize the lazy array
    mlx_rs::transforms::eval([&pooled]).unwrap();

    // Extract as f32 slice
    pooled.as_slice::<f32>().to_vec()
}

/// L2 normalize a vector in-place.
///
/// Divides each element by the L2 norm of the vector.
/// A zero vector is returned as-is.
pub fn l2_normalize(vec: &[f32]) -> Vec<f32> {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-10 {
        return vec.to_vec();
    }
    vec.iter().map(|x| x / norm).collect()
}

/// Compute cosine similarity between two f32 vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-10 || nb < 1e-10 {
        return 0.0;
    }
    dot / (na * nb)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_normalize_unit_vector() {
        let v = vec![3.0, 4.0]; // norm = 5
        let n = l2_normalize(&v);
        let norm: f32 = n.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
        assert!((n[0] - 0.6).abs() < 1e-6);
        assert!((n[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let v = vec![0.0, 0.0, 0.0];
        let n = l2_normalize(&v);
        assert_eq!(n, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }
}
