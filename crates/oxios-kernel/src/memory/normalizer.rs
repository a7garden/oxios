//! Embedding vector normalization utilities.
//!
//! Provides L2 normalization for dense vectors, ensuring they lie on the
//! unit hypersphere. Required for cosine similarity via dot product.

/// Normalize a dense vector in-place using L2 normalization.
///
/// After normalization, the vector has unit length (L2 norm = 1.0).
/// If the vector is zero, it is left unchanged.
pub fn l2_normalize_f32(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vector.iter_mut() {
            *v /= norm;
        }
    }
}

/// Normalize a dense f64 vector in-place using L2 normalization.
pub fn l2_normalize_f64(vector: &mut [f64]) {
    let norm: f64 = vector.iter().map(|v| v * v).sum::<f64>().sqrt();
    if norm > 0.0 {
        for v in vector.iter_mut() {
            *v /= norm;
        }
    }
}

/// Return the L2 norm of a vector.
pub fn l2_norm_f32(vector: &[f32]) -> f32 {
    vector.iter().map(|v| v * v).sum::<f32>().sqrt()
}

/// Return the L2 norm of an f64 vector.
pub fn l2_norm_f64(vector: &[f64]) -> f64 {
    vector.iter().map(|v| v * v).sum::<f64>().sqrt()
}

/// Compute dot product of two f32 vectors.
pub fn dot_product_f32(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Compute cosine similarity between two f32 vectors.
pub fn cosine_similarity_f32(a: &[f32], b: &[f32]) -> f32 {
    let dot = dot_product_f32(a, b);
    let na = l2_norm_f32(a);
    let nb = l2_norm_f32(b);
    if na == 0.0 || nb == 0.0 {
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
        let mut v = vec![1.0f32, 0.0, 0.0];
        l2_normalize_f32(&mut v);
        assert!((l2_norm_f32(&v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_general() {
        let mut v = vec![3.0f32, 4.0];
        l2_normalize_f32(&mut v);
        assert!((l2_norm_f32(&v) - 1.0).abs() < 1e-6);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_zero() {
        let mut v = vec![0.0f32, 0.0, 0.0];
        l2_normalize_f32(&mut v);
        assert_eq!(v, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0f32, 0.0, 0.0];
        let sim = cosine_similarity_f32(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![0.0f32, 1.0, 0.0];
        let sim = cosine_similarity_f32(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0f32, 0.0];
        let b = vec![-1.0f32, 0.0];
        let sim = cosine_similarity_f32(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0f32, 2.0, 3.0];
        let b = vec![4.0f32, 5.0, 6.0];
        assert_eq!(dot_product_f32(&a, &b), 32.0);
    }

    #[test]
    fn test_l2_normalize_f64() {
        let mut v = vec![3.0f64, 4.0];
        l2_normalize_f64(&mut v);
        assert!((l2_norm_f64(&v) - 1.0).abs() < 1e-12);
    }
}
