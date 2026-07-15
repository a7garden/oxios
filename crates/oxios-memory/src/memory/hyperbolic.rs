#![allow(missing_docs)]
//! Hyperbolic embeddings using the Poincaré ball model.
//!
//! The Poincaré ball model embeds hierarchical data (trees, taxonomies,
//! ontologies) in hyperbolic space where distances naturally encode
//! hierarchical relationships. Nodes close to the root are near the
//! origin; leaf nodes are near the boundary.
//!
//! Use cases in Oxios:
//! - Persona hierarchy (parent → child relationships)
//! - Skill graph (prerequisite chains)
//! - Memory category taxonomy
//!
//! Reference: "Poincaré Embeddings for Learning Hierarchical
//! Representations" (Nickel & Kiela, 2017)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for hyperbolic operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperbolicConfig {
    /// Curvature of the hyperbolic space.
    /// Must be negative. Default: -1.0 (standard Poincaré ball).
    pub curvature: f32,
    /// Embedding dimensionality.
    pub dimensions: usize,
    /// Numerical stability epsilon.
    pub epsilon: f32,
}

impl Default for HyperbolicConfig {
    fn default() -> Self {
        Self {
            curvature: -1.0,
            dimensions: 64,
            epsilon: 1e-5,
        }
    }
}

impl HyperbolicConfig {
    /// Create a new config with validation.
    pub fn new(curvature: f32, dimensions: usize) -> Self {
        assert!(
            curvature < 0.0,
            "Curvature must be negative for hyperbolic space"
        );
        Self {
            curvature,
            dimensions,
            epsilon: 1e-5,
        }
    }

    /// Returns the absolute value of curvature (c = |K|).
    ///
    /// All free functions in this module (e.g., `euclidean_to_poincare`,
    /// `hyperbolic_distance`) already compute `curvature.abs()` internally,
    /// so this accessor is not needed by callers of those functions.
    /// It remains available for code that needs the positive curvature
    /// value directly (e.g., computing ball radius `1/√c`).
    #[allow(dead_code)]
    fn c(&self) -> f32 {
        self.curvature.abs()
    }
}

// ---------------------------------------------------------------------------
// Poincaré ball operations
// ---------------------------------------------------------------------------

/// Convert a Euclidean vector to a point on the Poincaré ball.
///
/// Projects the vector onto the open unit ball with radius 1/√c.
/// Points are clipped to stay strictly inside the ball.
///
/// # Arguments
/// * `vector` - Euclidean vector
/// * `curvature` - Negative curvature K (e.g., -1.0)
///
/// # Returns
/// Point on the Poincaré ball
pub fn euclidean_to_poincare(vector: &[f32], curvature: f32) -> Vec<f32> {
    let c = curvature.abs();
    let max_norm = 1.0 / c.sqrt();

    // Compute Euclidean norm
    let norm_sq: f32 = vector.iter().map(|v| v * v).sum();
    let norm = norm_sq.sqrt();

    if norm == 0.0 {
        return vec![0.0; vector.len()];
    }

    // Map to ball: project and scale, keeping inside the boundary
    // Use tanh-based mapping for smooth bounded projection
    let scale = max_norm * norm.tanh() / norm;
    vector.iter().map(|&v| v * scale).collect()
}

/// Batch-convert Euclidean vectors to Poincaré ball points.
pub fn batch_euclidean_to_poincare(vectors: &[Vec<f32>], curvature: f32) -> Vec<Vec<f32>> {
    vectors
        .iter()
        .map(|v| euclidean_to_poincare(v, curvature))
        .collect()
}

/// Compute the hyperbolic distance between two points on the Poincaré ball.
///
/// d(x, y) = (1/√c) * arcosh(1 + 2c * δ(x, y) / ((1 - c||x||²)(1 - c||y||²)))
///
/// where δ(x, y) = ||x - y||²
pub fn hyperbolic_distance(a: &[f32], b: &[f32], curvature: f32) -> f32 {
    let c = curvature.abs();

    let norm_a_sq: f32 = a.iter().map(|v| v * v).sum();
    let norm_b_sq: f32 = b.iter().map(|v| v * v).sum();

    let diff_sq: f32 = a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum();

    let denominator = (1.0 - c * norm_a_sq) * (1.0 - c * norm_b_sq);

    if denominator <= 0.0 {
        // Points on or beyond the boundary — return max distance
        return f32::MAX;
    }

    let arg = 1.0 + 2.0 * c * diff_sq / denominator;

    if arg <= 1.0 {
        // Same point or very close
        return 0.0;
    }

    // arcosh(arg) = ln(arg + sqrt(arg² − 1)). The previous formula computed
    // sqrt(ln(arg)), which is wrong (e.g. arg=2 → 0.833 instead of arcosh(2)
    // ≈ 1.317), corrupting depth/hierarchy estimates in Poincaré embeddings.
    (1.0 / c.sqrt()) * (arg + (arg * arg - 1.0).sqrt()).ln()
}

/// Möbius addition: the hyperbolic analog of vector addition.
///
/// a ⊕_c b = ((1 + 2c⟨a,b⟩ + c||b||²)a + (1 - c||a||²)b) /
///           (1 + 2c⟨a,b⟩ + c²||a||²||b||²)
pub fn mobius_add(a: &[f32], b: &[f32], curvature: f32) -> Vec<f32> {
    let c = curvature.abs();

    let norm_a_sq: f32 = a.iter().map(|v| v * v).sum();
    let norm_b_sq: f32 = b.iter().map(|v| v * v).sum();
    let dot_ab: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();

    let denominator = 1.0 + 2.0 * c * dot_ab + c * c * norm_a_sq * norm_b_sq;

    if denominator.abs() < 1e-10 {
        // Degenerate case: return origin (neutral element)
        return vec![0.0; a.len()];
    }

    let scale_a = 1.0 + 2.0 * c * dot_ab + c * norm_b_sq;
    let scale_b = 1.0 - c * norm_a_sq;

    a.iter()
        .zip(b)
        .map(|(&ai, &bi)| (scale_a * ai + scale_b * bi) / denominator)
        .collect()
}

/// Möbius scalar multiplication: scaling in hyperbolic space.
///
/// s ⊗_c v = (1/√c) * tanh(s * arctanh(√c * ||v||)) * v / ||v||
///
/// # Arguments
/// * `scalar` - Multiplication factor
/// * `v` - Vector on the Poincaré ball
/// * `curvature` - Negative curvature K
/// * `epsilon` - Numerical stability margin (e.g. 1e-5)
pub fn mobius_scalar_mul(scalar: f32, v: &[f32], curvature: f32, epsilon: f32) -> Vec<f32> {
    let c = curvature.abs();
    let norm_sq: f32 = v.iter().map(|x| x * x).sum();
    let norm = norm_sq.sqrt();

    if norm < epsilon {
        return vec![0.0; v.len()];
    }

    let c_sqrt = c.sqrt();
    let w = c_sqrt * norm;

    // Clamp w to strictly less than 1 for numerical stability
    let w = w.min(1.0 - epsilon);
    let result_norm = (1.0 / c_sqrt) * (scalar * w.atanh()).tanh();

    let scale = result_norm / norm;
    v.iter().map(|&vi| vi * scale).collect()
}

// ---------------------------------------------------------------------------
// HyperbolicEmbedding — higher-level interface
// ---------------------------------------------------------------------------

/// Hyperbolic embedding manager for hierarchical data.
///
/// Provides a convenient interface for storing and querying
/// hierarchical embeddings in Poincaré ball space.
pub struct HyperbolicEmbedding {
    config: HyperbolicConfig,
    /// Named embeddings: id → Poincaré ball point.
    embeddings: Vec<(String, Vec<f32>)>,
}

impl HyperbolicEmbedding {
    /// Create a new hyperbolic embedding manager.
    pub fn new(config: HyperbolicConfig) -> Self {
        Self {
            config,
            embeddings: Vec::new(),
        }
    }

    /// Build a `HyperbolicEmbedding` from pre-existing (id, vector) pairs.
    ///
    /// Useful for restoring from a serialized source (e.g. SQLite `dream_state`).
    /// The vectors are stored as-is — call sites must ensure they were
    /// previously projected to the Poincaré ball.
    pub fn from_pairs(pairs: Vec<(String, Vec<f32>)>) -> Self {
        // Default config; the actual curvature is encoded in the vectors
        // themselves (they were projected to a specific ball).
        Self {
            config: HyperbolicConfig::default(),
            embeddings: pairs,
        }
    }

    /// Create with default configuration.
    pub fn with_dimensions(dimensions: usize) -> Self {
        let config = HyperbolicConfig {
            dimensions,
            ..Default::default()
        };
        Self::new(config)
    }

    /// Add a Euclidean vector as a named embedding.
    ///
    /// Converts to Poincaré ball coordinates.
    pub fn add(&mut self, id: &str, euclidean: &[f32]) {
        let poincare = euclidean_to_poincare(euclidean, self.config.curvature);
        // Replace if exists
        if let Some(pos) = self.embeddings.iter().position(|(name, _)| name == id) {
            self.embeddings[pos] = (id.to_string(), poincare);
        } else {
            self.embeddings.push((id.to_string(), poincare));
        }
    }

    /// Add a parent-child relationship using Möbius addition.
    ///
    /// The child is placed at `parent ⊕ child_euclidean`, which naturally
    /// positions it farther from the origin along the parent's direction.
    pub fn add_child(&mut self, parent_id: &str, child_id: &str, child_euclidean: &[f32]) {
        let child_on_ball = euclidean_to_poincare(child_euclidean, self.config.curvature);

        let child_point = if let Some((_, parent_vec)) =
            self.embeddings.iter().find(|(name, _)| name == parent_id)
        {
            // Use Möbius addition: child = parent ⊕ child_offset
            // This naturally places the child deeper in the hierarchy
            mobius_add(parent_vec, &child_on_ball, self.config.curvature)
        } else {
            child_on_ball
        };

        if let Some(pos) = self
            .embeddings
            .iter()
            .position(|(name, _)| name == child_id)
        {
            self.embeddings[pos] = (child_id.to_string(), child_point);
        } else {
            self.embeddings.push((child_id.to_string(), child_point));
        }
    }

    /// Get the hyperbolic embedding for a given id.
    pub fn get(&self, id: &str) -> Option<&[f32]> {
        self.embeddings
            .iter()
            .find(|(name, _)| name == id)
            .map(|(_, v)| v.as_slice())
    }

    /// Find the k nearest neighbors in hyperbolic space.
    ///
    /// Returns (id, distance) pairs sorted by distance.
    pub fn nearest_neighbors(&self, query_id: &str, k: usize) -> Vec<(String, f32)> {
        let query = match self.get(query_id) {
            Some(v) => v.to_vec(),
            None => return Vec::new(),
        };

        let mut results: Vec<(String, f32)> = self
            .embeddings
            .iter()
            .filter(|(name, _)| name != query_id)
            .map(|(name, vec)| {
                let dist = hyperbolic_distance(&query, vec, self.config.curvature);
                (name.clone(), dist)
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    /// Find nearest neighbors for an arbitrary Euclidean query.
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let query_poincare = euclidean_to_poincare(query, self.config.curvature);

        let mut results: Vec<(String, f32)> = self
            .embeddings
            .iter()
            .map(|(name, vec)| {
                let dist = hyperbolic_distance(&query_poincare, vec, self.config.curvature);
                (name.clone(), dist)
            })
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    /// Compute the hierarchical distance between two embeddings.
    ///
    /// In hierarchical data, nodes deeper in the tree are farther from
    /// the origin. This function returns the hyperbolic distance plus
    /// a depth penalty.
    pub fn hierarchical_distance(&self, id_a: &str, id_b: &str) -> f32 {
        let a = match self.get(id_a) {
            Some(v) => v,
            None => return f32::MAX,
        };
        let b = match self.get(id_b) {
            Some(v) => v,
            None => return f32::MAX,
        };

        hyperbolic_distance(a, b, self.config.curvature)
    }

    /// Returns the number of stored embeddings.
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Returns true if no embeddings stored.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Returns all embedding ids.
    pub fn ids(&self) -> Vec<&str> {
        self.embeddings
            .iter()
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Returns all embeddings as (id, vector) pairs.
    pub fn all_embeddings(&self) -> &[(String, Vec<f32>)] {
        &self.embeddings
    }

    // ── Phase 5: SQLite Persistence ────────────────────────────────
    // Moved to `crate::sqlite::hyperbolic_persist` (RFC-018 b.8).
    // The cfg-gated SQLite-specific methods live there as free functions
    // that take `SqliteMemoryStore` as a parameter. The pure-math core
    // remains here in `HyperbolicEmbedding`.

    /// Find memories near a query in hyperbolic space.
    ///
    /// Returns (memory_id, hyperbolic_distance) pairs.
    /// Useful for hierarchical navigation: memories close to the
    /// root are general; those near the boundary are specific.
    pub fn search_memories(&self, query_euclidean: &[f32], k: usize) -> Vec<(String, f32)> {
        self.search(query_euclidean, k)
    }

    /// Get the hierarchical depth rank of all memories.
    ///
    /// Memories closer to the origin are more general/root-level.
    /// Memories farther from origin are more specific/leaf-level.
    ///
    /// Returns (id, depth) pairs sorted by depth ascending.
    pub fn hierarchical_rank(&self) -> Vec<(String, f32)> {
        self.rank_by_depth()
    }

    /// Get the hyperbolic distance of a point from the origin.
    ///
    /// Points closer to the origin are "higher" in the hierarchy.
    pub fn depth(&self, id: &str) -> f32 {
        match self.get(id) {
            Some(v) => hyperbolic_distance(&vec![0.0; v.len()], v, self.config.curvature),
            None => f32::MAX,
        }
    }

    /// Rank all embeddings by depth (origin distance).
    ///
    /// Returns (id, depth) pairs sorted by depth ascending.
    /// Items with lower depth are closer to the root of the hierarchy.
    pub fn rank_by_depth(&self) -> Vec<(String, f32)> {
        let mut ranked: Vec<(String, f32)> = self
            .embeddings
            .iter()
            .map(|(name, vec)| {
                let origin = vec![0.0; vec.len()];
                let d = hyperbolic_distance(&origin, vec, self.config.curvature);
                (name.clone(), d)
            })
            .collect();

        ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // -----------------------------------------------------------------------
    // Property tests — invariant-style checks proptest is built for.
    // The inline #[test] cases above pin specific shapes; these exercise
    // a wider input space and catch subtle floating-point regressions.
    // -----------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// euclidean_to_poincare preserves length: input and output have
        /// the same number of dimensions.
        #[test]
        fn prop_euclidean_to_poincare_preserves_dim(
            v in proptest::collection::vec(-2.0_f32..2.0, 1..16),
        ) {
            let out = euclidean_to_poincare(&v, -1.0);
            prop_assert_eq!(out.len(), v.len());
        }

        /// euclidean_to_poincare(zero) == zero for any dimension.
        #[test]
        fn prop_euclidean_to_poincare_zero_is_zero(dim in 1_usize..16) {
            let v = vec![0.0_f32; dim];
            let out = euclidean_to_poincare(&v, -1.0);
            prop_assert_eq!(out, v);
        }

        /// Result always lies strictly inside the open ball: norm < 1/√c.
        /// (Note: the function uses tanh so the norm is actually bounded
        /// strictly less than max_norm for any non-zero input.)
        #[test]
        fn prop_euclidean_to_poincare_stays_in_ball(
            v in proptest::collection::vec(-1.0_f32..1.0, 1..8),
        ) {
            let out = euclidean_to_poincare(&v, -1.0);
            let norm_sq: f32 = out.iter().map(|x| x * x).sum();
            let norm = norm_sq.sqrt();
            let max_norm = 1.0 / 1.0_f32.sqrt(); // c = |-1.0| = 1.0
            prop_assert!(norm <= max_norm + 1e-5, "norm {} > max {}", norm, max_norm);
        }

        /// hyperbolic_distance is non-negative for any two inputs.
        #[test]
        fn prop_distance_is_nonneg(
            a in proptest::collection::vec(-0.5_f32..0.5, 1..8),
            b in proptest::collection::vec(-0.5_f32..0.5, 1..8),
        ) {
            let d = hyperbolic_distance(&a, &b, -1.0);
            prop_assert!(d >= 0.0, "distance must be non-negative, got {}", d);
        }

        /// hyperbolic_distance(a, a) == 0 (identity of indiscernibles).
        #[test]
        fn prop_distance_self_is_zero(
            a in proptest::collection::vec(-0.5_f32..0.5, 1..8),
        ) {
            let d = hyperbolic_distance(&a, &a, -1.0);
            // Allow tiny epsilon for floating-point noise near boundary.
            prop_assert!(d < 1e-4, "distance(a, a) should be 0, got {}", d);
        }

        /// hyperbolic_distance is symmetric: d(a, b) == d(b, a).
        #[test]
        fn prop_distance_is_symmetric(
            a in proptest::collection::vec(-0.5_f32..0.5, 1..8),
            b in proptest::collection::vec(-0.5_f32..0.5, 1..8),
        ) {
            let d_ab = hyperbolic_distance(&a, &b, -1.0);
            let d_ba = hyperbolic_distance(&b, &a, -1.0);
            prop_assert!(
                (d_ab - d_ba).abs() < 1e-4,
                "d(a,b)={} d(b,a)={}",
                d_ab,
                d_ba
            );
        }

        /// Triangle inequality: d(a, c) ≤ d(a, b) + d(b, c).
        #[test]
        fn prop_distance_triangle_inequality(
            (a, b, c) in (1usize..6).prop_flat_map(|n| (
                proptest::collection::vec(-0.4_f32..0.4, n),
                proptest::collection::vec(-0.4_f32..0.4, n),
                proptest::collection::vec(-0.4_f32..0.4, n),
            )),
        ) {
            let d_ab = hyperbolic_distance(&a, &b, -1.0);
            let d_bc = hyperbolic_distance(&b, &c, -1.0);
            let d_ac = hyperbolic_distance(&a, &c, -1.0);
            // f32::MAX sentinel for boundary points can break the inequality.
            if d_ab < f32::MAX && d_bc < f32::MAX && d_ac < f32::MAX {
                prop_assert!(
                    d_ac <= d_ab + d_bc + 1e-2,
                    "triangle inequality violated: d(a,c)={} d(a,b)={} d(b,c)={}",
                    d_ac,
                    d_ab,
                    d_bc
                );
            }
        }

        /// mobius_add preserves dimension.
        #[test]
        fn prop_mobius_add_preserves_dim(
            (a, b) in (1usize..8).prop_flat_map(|n| (
                proptest::collection::vec(-0.4_f32..0.4, n),
                proptest::collection::vec(-0.4_f32..0.4, n),
            )),
        ) {
            let sum = mobius_add(&a, &b, -1.0);
            prop_assert_eq!(sum.len(), a.len());
        }

        /// mobius_add(a, 0) == a (zero is the additive identity).
        #[test]
        fn prop_mobius_add_zero_identity(
            a in proptest::collection::vec(-0.4_f32..0.4, 1..8),
        ) {
            let zero = vec![0.0_f32; a.len()];
            let sum = mobius_add(&a, &zero, -1.0);
            for (got, want) in sum.iter().zip(a.iter()) {
                prop_assert!((got - want).abs() < 1e-4, "{} vs {}", got, want);
            }
        }

        /// mobius_scalar_mul(0, v) == 0 (zero scalar kills the vector).
        #[test]
        fn prop_mobius_scalar_mul_zero(
            v in proptest::collection::vec(-0.4_f32..0.4, 1..8),
        ) {
            let r = mobius_scalar_mul(0.0, &v, -1.0, 1e-5);
            for x in &r {
                prop_assert!(x.abs() < 1e-4, "expected 0, got {}", x);
            }
        }
    }
    #[test]
    fn test_euclidean_to_poincare_zero() {
        let result = euclidean_to_poincare(&[0.0, 0.0, 0.0], -1.0);
        assert_eq!(result, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_euclidean_to_poincare_bounded() {
        let c = -1.0;
        // Large vector should be projected inside the ball
        let result = euclidean_to_poincare(&[100.0, 100.0, 100.0], c);
        let norm: f32 = result.iter().map(|v| v * v).sum::<f32>().sqrt();
        let max_norm = 1.0 / c.abs().sqrt();
        assert!(
            norm < max_norm,
            "Result should be inside the ball: norm={}, max={}",
            norm,
            max_norm
        );
    }

    #[test]
    fn test_hyperbolic_distance_same_point() {
        let point = euclidean_to_poincare(&[0.5, 0.3], -1.0);
        let dist = hyperbolic_distance(&point, &point, -1.0);
        assert!(dist < 1e-5, "Distance from self should be ~0, got {}", dist);
    }

    #[test]
    fn test_hyperbolic_distance_symmetry() {
        let a = euclidean_to_poincare(&[1.0, 2.0], -1.0);
        let b = euclidean_to_poincare(&[3.0, 1.0], -1.0);
        let d_ab = hyperbolic_distance(&a, &b, -1.0);
        let d_ba = hyperbolic_distance(&b, &a, -1.0);
        assert!(
            (d_ab - d_ba).abs() < 1e-4,
            "Distance should be symmetric: {} vs {}",
            d_ab,
            d_ba
        );
    }

    #[test]
    fn test_hyperbolic_distance_triangle_inequality() {
        let a = euclidean_to_poincare(&[1.0, 0.0], -1.0);
        let b = euclidean_to_poincare(&[0.0, 1.0], -1.0);
        let c = euclidean_to_poincare(&[2.0, 2.0], -1.0);

        let d_ab = hyperbolic_distance(&a, &b, -1.0);
        let d_bc = hyperbolic_distance(&b, &c, -1.0);
        let d_ac = hyperbolic_distance(&a, &c, -1.0);

        assert!(
            d_ac <= d_ab + d_bc + 1e-4,
            "Triangle inequality: d(a,c)={} should be <= d(a,b)+d(b,c)={}",
            d_ac,
            d_ab + d_bc
        );
    }

    #[test]
    fn test_mobius_add_identity() {
        let a = euclidean_to_poincare(&[0.5, 0.3], -1.0);
        let zero = vec![0.0, 0.0];
        let result = mobius_add(&a, &zero, -1.0);
        for (r, expected) in result.iter().zip(a.iter()) {
            assert!((r - expected).abs() < 1e-4, "a ⊕ 0 should equal a");
        }
    }

    #[test]
    fn test_mobius_scalar_mul_zero() {
        let v = euclidean_to_poincare(&[1.0, 2.0], -1.0);
        let result = mobius_scalar_mul(0.0, &v, -1.0, 1e-5);
        for r in &result {
            assert!(r.abs() < 1e-4, "0 ⊗ v should be ~0, got {}", r);
        }
    }

    #[test]
    fn test_mobius_scalar_mul_one() {
        let v = euclidean_to_poincare(&[1.0, 2.0], -1.0);
        let result = mobius_scalar_mul(1.0, &v, -1.0, 1e-5);
        for (r, expected) in result.iter().zip(v.iter()) {
            assert!((r - expected).abs() < 1e-4, "1 ⊗ v should equal v");
        }
    }

    #[test]
    fn test_hyperbolic_embedding_add_and_search() {
        let mut he = HyperbolicEmbedding::with_dimensions(3);

        he.add("root", &[0.0, 0.0, 0.0]);
        he.add("child_a", &[1.0, 0.0, 0.0]);
        he.add("child_b", &[0.0, 1.0, 0.0]);
        he.add("grandchild", &[1.0, 1.0, 0.0]);

        assert_eq!(he.len(), 4);

        // Nearest neighbor of child_a should be grandchild (closer in hierarchy)
        let nn = he.nearest_neighbors("child_a", 2);
        assert_eq!(nn.len(), 2);
        // grandchild should be closer to child_a than child_b
        let gc_dist = nn
            .iter()
            .find(|(name, _)| name == "grandchild")
            .map(|(_, d)| *d);
        let cb_dist = nn
            .iter()
            .find(|(name, _)| name == "child_b")
            .map(|(_, d)| *d);
        if let (Some(gc), Some(cb)) = (gc_dist, cb_dist) {
            assert!(
                gc < cb,
                "grandchild should be closer to child_a than child_b"
            );
        }
    }

    #[test]
    fn test_hyperbolic_embedding_depth() {
        let mut he = HyperbolicEmbedding::with_dimensions(2);

        he.add("root", &[0.0, 0.0]);
        he.add("level1", &[0.5, 0.0]);
        he.add("level2", &[1.0, 0.0]);

        let root_depth = he.depth("root");
        let l1_depth = he.depth("level1");
        let l2_depth = he.depth("level2");

        assert!(
            root_depth < l1_depth,
            "Root should be shallower: root={}, l1={}",
            root_depth,
            l1_depth
        );
        assert!(
            l1_depth < l2_depth,
            "Level1 should be shallower: l1={}, l2={}",
            l1_depth,
            l2_depth
        );
    }

    #[test]
    fn test_rank_by_depth() {
        let mut he = HyperbolicEmbedding::with_dimensions(2);

        he.add("leaf", &[2.0, 2.0]);
        he.add("root", &[0.0, 0.0]);
        he.add("mid", &[0.5, 0.5]);

        let ranked = he.rank_by_depth();
        assert_eq!(ranked[0].0, "root");
        assert_eq!(ranked[1].0, "mid");
        assert_eq!(ranked[2].0, "leaf");
    }

    #[test]
    fn test_batch_conversion() {
        let vectors = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![0.0, 0.0]];
        let results = batch_euclidean_to_poincare(&vectors, -1.0);
        assert_eq!(results.len(), 3);
        // Last should be zero
        assert_eq!(results[2], vec![0.0, 0.0]);
    }

    #[test]
    fn test_curvature_effect() {
        let v = [1.0, 1.0];

        let p1 = euclidean_to_poincare(&v, -1.0);
        let p2 = euclidean_to_poincare(&v, -2.0);

        let norm1: f32 = p1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm2: f32 = p2.iter().map(|x| x * x).sum::<f32>().sqrt();

        // Higher curvature magnitude → smaller ball → smaller norm
        assert!(
            norm2 < norm1,
            "Higher curvature should produce smaller ball: {} vs {}",
            norm2,
            norm1
        );
    }

    #[test]
    fn test_add_child_hierarchy() {
        let mut he = HyperbolicEmbedding::with_dimensions(3);

        // Create a simple hierarchy
        he.add("parent", &[1.0, 0.0, 0.0]);
        he.add_child("parent", "child", &[0.5, 0.5, 0.0]);

        assert_eq!(he.len(), 2);

        // Both should exist
        assert!(he.get("parent").is_some());
        assert!(he.get("child").is_some());
    }
}
