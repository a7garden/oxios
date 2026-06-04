//! Memory embedding visualization (RFC-T1-B).
//!
//! Projects high-dimensional memory embeddings into 2D coordinates for the
//! web UI Memory Map. We use truncated PCA via pure-Rust power iteration
//! (no external linear-algebra dependency) and cosine-similarity for
//! neighbor detection.
//!
//! ## Why not UMAP / t-SNE?
//!
//! The RFC recommends UMAP. We chose PCA for the MVP because:
//!
//! 1. **No external dep** — `linfa`/`linfa-umap` would add ~5MB of compile
//!    time and a non-trivial pure-Rust surface area. PCA via power
//!    iteration fits in ~80 lines.
//! 2. **Deterministic** — given the same input we always get the same
//!    output (UMAP/TSNE have stochastic init).
//! 3. **Cheap** — O(n · d · k) for k components; trivial at n=1000.
//! 4. **Caches well** — pure function of embeddings, perfect for the
//!    5-min epoch cache the RFC requires.
//!
//! PCA captures *global* structure (the principal axes of variance),
//! not local neighborhoods. That is sufficient for "is this cluster
//! of Hot memories visually distinct from Cold?" — the use case for
//! the MVP. UMAP can be added later behind the same interface.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One node on the memory map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMapEntry {
    /// Memory entry ID.
    pub id: String,
    /// Tier (hot/warm/cold).
    pub tier: String,
    /// Memory type label (fact/episode/...).
    pub mem_type: String,
    /// First ~120 chars of content for hover preview.
    pub content_preview: String,
    /// RFC3339 timestamp.
    pub created_at: String,
    /// Lifetime access count (proxy for importance).
    pub access_count: u32,
    /// 2D coordinates in canvas units (after normalization).
    pub coords_2d: (f32, f32),
    /// Top similar memories (cosine > threshold, max 5).
    pub top_neighbors: Vec<MemoryNeighbor>,
}

/// Edge from one memory to a similar neighbor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryNeighbor {
    /// Neighbor memory ID.
    pub id: String,
    /// Cosine similarity in 0.0..=1.0.
    pub similarity: f32,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Project a set of embeddings into 2D coordinates via PCA.
///
/// Inputs can be heterogeneous (sparse, dense, varying dimensions).
/// They are unified by their term-set union so the math stays valid.
///
/// Returns one (x, y) per input, in the same order. Returns an empty
/// vector when `embeddings` is empty. All-zero or single-entry inputs
/// are returned at the origin (0, 0).
pub fn compute_pca_2d(embeddings: &[Vec<f32>]) -> Vec<(f32, f32)> {
    let n = embeddings.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![(0.0, 0.0)];
    }

    // 1) Build dense matrix: n rows × d cols (d = vocab size).
    let vocab = build_vocab(embeddings);
    if vocab.is_empty() {
        return vec![(0.0, 0.0); n];
    }
    let matrix = densify(embeddings, &vocab);

    // 2) Center columns.
    let centered = center_columns(&matrix, n, vocab.len());

    // 3) Power iteration to get the top eigenvector, then deflate.
    let v1 = power_iteration(&centered, n, vocab.len(), 80);
    let p1 = project(&centered, &v1, n, vocab.len());
    let residuals = deflate(&centered, &v1, &p1, n, vocab.len());

    let v2 = power_iteration(&residuals, n, vocab.len(), 80);
    let p2 = project(&residuals, &v2, n, vocab.len());

    // 4) Normalize into [-1, 1] range for stable canvas rendering.
    let coords: Vec<(f32, f32)> = p1
        .iter()
        .zip(p2.iter())
        .map(|(a, b)| (*a as f32, *b as f32))
        .collect();
    normalize_to_unit_square(&coords)
}

/// Top-k most similar neighbors per embedding, cosine-similarity only.
///
/// Returns one `Vec<MemoryNeighbor>` per input, in the same order.
/// Each output is sorted by similarity desc, length ≤ `top_k`, and
/// filtered to `similarity >= threshold`. Self-edges are removed.
pub fn compute_top_neighbors(
    embeddings: &[Vec<f32>],
    ids: &[String],
    top_k: usize,
    threshold: f32,
) -> Vec<Vec<MemoryNeighbor>> {
    debug_assert_eq!(embeddings.len(), ids.len());
    let n = embeddings.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![Vec::new()];
    }

    let vocab = build_vocab(embeddings);
    if vocab.is_empty() {
        return vec![Vec::new(); n];
    }
    let matrix = densify(embeddings, &vocab);
    let norms: Vec<f64> = matrix.iter().map(|row| row_norm(row)).collect();

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut sims: Vec<(usize, f64)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| {
                let sim = if norms[i] == 0.0 || norms[j] == 0.0 {
                    0.0
                } else {
                    let dot: f64 = matrix[i]
                        .iter()
                        .zip(matrix[j].iter())
                        .map(|(a, b)| *a as f64 * *b as f64)
                        .sum();
                    dot / (norms[i] * norms[j])
                };
                (j, sim)
            })
            .filter(|(_, s)| *s >= threshold as f64)
            .collect();
        sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sims.truncate(top_k);
        out.push(
            sims
                .into_iter()
                .map(|(j, s)| MemoryNeighbor {
                    id: ids[j].clone(),
                    similarity: s as f32,
                })
                .collect(),
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Helpers (private)
// ---------------------------------------------------------------------------

/// Build a vocabulary of all term indices present in any embedding.
fn build_vocab(embeddings: &[Vec<f32>]) -> Vec<usize> {
    let mut vocab = Vec::new();
    for emb in embeddings {
        for (idx, v) in emb.iter().enumerate() {
            if *v != 0.0 && !vocab.contains(&idx) {
                vocab.push(idx);
            }
        }
    }
    vocab
}

/// Convert list of sparse embeddings into a dense n × d matrix.
fn densify(embeddings: &[Vec<f32>], vocab: &[usize]) -> Vec<Vec<f64>> {
    embeddings
        .iter()
        .map(|emb| {
            let mut row = vec![0.0_f64; vocab.len()];
            for (out_idx, vocab_idx) in vocab.iter().enumerate() {
                if let Some(v) = emb.get(*vocab_idx) {
                    row[out_idx] = *v as f64;
                }
            }
            row
        })
        .collect()
}

/// Subtract column means to center the data.
fn center_columns(matrix: &[Vec<f64>], n: usize, d: usize) -> Vec<Vec<f64>> {
    if d == 0 || n == 0 {
        return matrix.to_vec();
    }
    let mut means = vec![0.0_f64; d];
    for row in matrix {
        for (j, v) in row.iter().enumerate() {
            means[j] += *v;
        }
    }
    for m in &mut means {
        *m /= n as f64;
    }
    matrix
        .iter()
        .map(|row| row.iter().zip(means.iter()).map(|(v, m)| *v - *m).collect())
        .collect()
}

/// L2 norm of a row.
fn row_norm(row: &[f64]) -> f64 {
    row.iter().map(|v| *v * *v).sum::<f64>().sqrt()
}

/// Power iteration: returns the top eigenvector of `A^T A` (or equivalently
/// the top left singular vector of `A`).
///
/// Used twice: once on the centered data, then on the deflated residuals,
/// to recover the second principal component.
fn power_iteration(matrix: &[Vec<f64>], n: usize, d: usize, iterations: usize) -> Vec<f64> {
    if d == 0 {
        return Vec::new();
    }
    // Seed vector: deterministic but non-zero.
    let mut v: Vec<f64> = (0..d)
        .map(|i| ((i + 1) as f64 * 0.137).sin() + 1.0)
        .collect();
    normalize(&mut v);

    for _ in 0..iterations {
        // m_new = A^T (A v)
        let mut av = vec![0.0_f64; n];
        for i in 0..n {
            let mut s = 0.0;
            for j in 0..d {
                s += matrix[i][j] * v[j];
            }
            av[i] = s;
        }
        let mut ata_v = vec![0.0_f64; d];
        for i in 0..n {
            for j in 0..d {
                ata_v[j] += matrix[i][j] * av[i];
            }
        }
        normalize(&mut ata_v);
        // Convergence check (optional; we just iterate the fixed count).
        v = ata_v;
    }
    v
}

/// Project rows of `matrix` onto `v` (one scalar per row).
fn project(matrix: &[Vec<f64>], v: &[f64], n: usize, d: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut s = 0.0;
        for j in 0..d.min(v.len()) {
            s += matrix[i][j] * v[j];
        }
        out.push(s);
    }
    out
}

/// Subtract the rank-1 component along `v` (with score `p`) from `matrix`.
fn deflate(
    matrix: &[Vec<f64>],
    v: &[f64],
    p: &[f64],
    n: usize,
    d: usize,
) -> Vec<Vec<f64>> {
    let mut out = matrix.to_vec();
    for i in 0..n {
        for j in 0..d.min(v.len()) {
            out[i][j] -= p[i] * v[j];
        }
    }
    out
}

fn normalize(v: &mut [f64]) {
    let norm = v.iter().map(|x| *x * *x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Rescale coordinates to roughly [-1, 1] for stable canvas rendering.
fn normalize_to_unit_square(coords: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if coords.is_empty() {
        return Vec::new();
    }
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for &(x, y) in coords {
        if x < min_x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if y > max_y {
            max_y = y;
        }
    }
    let span_x = (max_x - min_x).max(f32::MIN_POSITIVE);
    let span_y = (max_y - min_y).max(f32::MIN_POSITIVE);
    let span = span_x.max(span_y);
    if span <= 0.0 {
        return coords.to_vec();
    }
    let cx = (min_x + max_x) / 2.0;
    let cy = (min_y + max_y) / 2.0;
    coords
        .iter()
        .map(|(x, y)| (((x - cx) / span) as f32, ((y - cy) / span) as f32))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: dense f32 vectors for the test matrix.
    fn v(values: &[f32]) -> Vec<f32> {
        values.to_vec()
    }

    #[test]
    fn pca_two_clear_clusters_along_axes() {
        // Two clusters: one along (1, 0, 0) and one along (0, 1, 0).
        // PCA must recover the two axes — cluster centers should
        // separate cleanly along x and y.
        let mut embs = Vec::new();
        for i in 0..5 {
            embs.push(v(&[10.0 + i as f32, 0.0, 0.0]));
        }
        for i in 0..5 {
            embs.push(v(&[0.0, 10.0 + i as f32, 0.0]));
        }
        let coords = compute_pca_2d(&embs);
        assert_eq!(coords.len(), 10);

        // The two clusters should have distinct centroids.
        let (cx1, cy1) = centroid(&coords[0..5]);
        let (cx2, cy2) = centroid(&coords[5..10]);
        let dist = ((cx1 - cx2).powi(2) + (cy1 - cy2).powi(2)).sqrt();
        assert!(
            dist > 0.5,
            "clusters should be visually separated, got {dist}"
        );
    }

    #[test]
    fn pca_empty_input() {
        let coords = compute_pca_2d(&[]);
        assert!(coords.is_empty());
    }

    #[test]
    fn pca_single_input_returns_origin() {
        let coords = compute_pca_2d(&[v(&[1.0, 2.0, 3.0])]);
        assert_eq!(coords, vec![(0.0, 0.0)]);
    }

    #[test]
    fn pca_deterministic() {
        // Same input must always produce the same output.
        let embs = vec![
            v(&[1.0, 0.0, 0.0, 0.0]),
            v(&[0.0, 1.0, 0.0, 0.0]),
            v(&[0.0, 0.0, 1.0, 0.0]),
            v(&[0.0, 0.0, 0.0, 1.0]),
            v(&[1.0, 1.0, 0.0, 0.0]),
            v(&[0.0, 0.0, 1.0, 1.0]),
        ];
        let a = compute_pca_2d(&embs);
        let b = compute_pca_2d(&embs);
        for (pa, pb) in a.iter().zip(b.iter()) {
            assert!((pa.0 - pb.0).abs() < 1e-5);
            assert!((pa.1 - pb.1).abs() < 1e-5);
        }
    }

    #[test]
    fn pca_handles_zero_vectors() {
        // A row of zeros must not poison the result.
        let embs = vec![
            v(&[0.0, 0.0, 0.0]),
            v(&[1.0, 0.0, 0.0]),
            v(&[0.0, 1.0, 0.0]),
        ];
        let coords = compute_pca_2d(&embs);
        assert_eq!(coords.len(), 3);
        for c in &coords {
            assert!(c.0.is_finite() && c.1.is_finite());
        }
    }

    #[test]
    fn pca_handles_sparse_input() {
        // Different vector lengths, mostly zeros.
        let embs = vec![
            v(&[1.0, 0.0, 0.0, 0.0, 0.0]),
            v(&[1.0, 0.0, 0.0, 0.0, 0.0]),
            v(&[0.0, 0.0, 0.0, 0.0, 1.0]),
            v(&[0.0, 0.0, 0.0, 0.0, 1.0]),
        ];
        let coords = compute_pca_2d(&embs);
        assert_eq!(coords.len(), 4);
        // Two pairs of identical vectors should collapse to one point.
        let d1 = dist(coords[0], coords[1]);
        let d2 = dist(coords[2], coords[3]);
        assert!(d1 < 1e-3, "identical pair must coincide, got {d1}");
        assert!(d2 < 1e-3, "identical pair must coincide, got {d2}");
    }

    #[test]
    fn neighbors_finds_nearest() {
        let embs = vec![
            v(&[1.0, 0.0, 0.0]), // 0
            v(&[1.0, 0.0, 0.0]), // 1 — same as 0
            v(&[0.0, 1.0, 0.0]), // 2 — orthogonal
            v(&[0.0, 0.0, 1.0]), // 3 — orthogonal
        ];
        let ids: Vec<String> = (0..4).map(|i| format!("id{i}")).collect();
        let nbrs = compute_top_neighbors(&embs, &ids, 2, 0.0);
        assert_eq!(nbrs.len(), 4);

        // For entry 0, the top neighbor should be entry 1 (cos = 1.0).
        let top0 = &nbrs[0];
        assert!(!top0.is_empty());
        assert_eq!(top0[0].id, "id1");
        assert!((top0[0].similarity - 1.0).abs() < 1e-4);
        // Self-edges must be removed.
        assert!(top0.iter().all(|n| n.id != "id0"));
    }

    #[test]
    fn neighbors_threshold_filters() {
        let embs = vec![
            v(&[1.0, 0.0]), // 0
            v(&[1.0, 0.0]), // 1 — same
            v(&[0.0, 1.0]), // 2 — orthogonal, sim 0
        ];
        let ids: Vec<String> = (0..3).map(|i| format!("id{i}")).collect();
        // With threshold 0.5, entry 0 should only see entry 1.
        let nbrs = compute_top_neighbors(&embs, &ids, 5, 0.5);
        assert_eq!(nbrs[0].len(), 1);
        assert_eq!(nbrs[0][0].id, "id1");
    }

    #[test]
    fn neighbors_empty_input() {
        let nbrs = compute_top_neighbors(&[], &[], 5, 0.0);
        assert!(nbrs.is_empty());
    }

    #[test]
    fn neighbors_single_input() {
        let embs = vec![v(&[1.0, 0.0])];
        let ids = vec!["only".to_string()];
        let nbrs = compute_top_neighbors(&embs, &ids, 5, 0.0);
        assert_eq!(nbrs, vec![Vec::<MemoryNeighbor>::new()]);
    }

    #[test]
    fn map_entry_serializes_json() {
        let entry = MemoryMapEntry {
            id: "abc".into(),
            tier: "hot".into(),
            mem_type: "fact".into(),
            content_preview: "hello world".into(),
            created_at: "2026-06-04T00:00:00Z".into(),
            access_count: 3,
            coords_2d: (0.5, -0.5),
            top_neighbors: vec![MemoryNeighbor {
                id: "def".into(),
                similarity: 0.87,
            }],
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"id\":\"abc\""));
        assert!(json.contains("\"coords_2d\":[0.5,-0.5]"));
        assert!(json.contains("\"similarity\":0.87"));
    }

    // ---- Helpers ----

    fn centroid(coords: &[(f32, f32)]) -> (f32, f32) {
        let sx: f32 = coords.iter().map(|c| c.0).sum();
        let sy: f32 = coords.iter().map(|c| c.1).sum();
        (sx / coords.len() as f32, sy / coords.len() as f32)
    }

    fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
    }
}
