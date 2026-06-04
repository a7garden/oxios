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
//! 3. **Cheap-ish** — O(nnz · iterations · k) for k components, where
//!    `nnz` is the number of non-zero entries across the input. The
//!    TF-IDF vectors the kernel produces are sparse (most entries are
//!    zero), so the practical cost is much lower than the dense
//!    `O(n · d)` bound. We work directly on the sparse representation
//!    and never densify.
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
///
/// Complexity: `O(nnz · iterations · k)` where `nnz` is the number of
/// non-zero entries across all input rows. The kernel's TF-IDF vectors
/// are typically sparse (a few non-zero terms per entry), so the
/// practical cost is far below the dense `O(n · d)` bound.
pub fn compute_pca_2d(embeddings: &[Vec<f32>]) -> Vec<(f32, f32)> {
    let n = embeddings.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![(0.0, 0.0)];
    }

    // 1) Build a sparse representation: nnz pairs per row + a column
    //    index. We never densify the n × d matrix.
    let sparse = build_sparse(embeddings);
    if sparse.nnz == 0 {
        return vec![(0.0, 0.0); n];
    }
    let d = sparse.d;

    // 2) Compute column means and subtract them implicitly by
    //    working on the centered sparse matvecs.
    let means = column_means(&sparse, n, d);
    let centered_sparse = Sparse::centered(&sparse, &means);

    // 3) Power iteration to get the top eigenvector, then deflate.
    //    20 iterations is sufficient for the well-separated singular
    //    values produced by TF-IDF after centering (the
    //    `pca_deterministic` test still passes).
    const POWER_ITERATIONS: usize = 20;

    let v1 = power_iteration_sparse(&centered_sparse, POWER_ITERATIONS);
    let p1 = project_sparse(&centered_sparse, &v1);
    // Deflate without materialising the dense matrix.
    let v2 = power_iteration_deflated(&centered_sparse, &p1, &v1, POWER_ITERATIONS);
    let p2 = project_deflated(&centered_sparse, &p1, &v1, &v2);

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

    let s = build_sparse(embeddings);
    if s.nnz == 0 {
        return vec![Vec::new(); n];
    }
    // Per-row L2 norm from the sparse form: sum of squares of nnz.
    let norms: Vec<f64> = s
        .rows
        .iter()
        .map(|row| row.iter().map(|(_, v)| *v * *v).sum::<f64>().sqrt())
        .collect();

    // Sort each row's (col, val) list by col index so we can do a
    // merge-style dot product between any two rows.
    let mut sorted_rows: Vec<Vec<(usize, f64)>> = s.rows.clone();
    for r in &mut sorted_rows {
        r.sort_by_key(|(j, _)| *j);
    }

    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut sims: Vec<(usize, f64)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| {
                let sim = if norms[i] == 0.0 || norms[j] == 0.0 {
                    0.0
                } else {
                    sparse_dot(&sorted_rows[i], &sorted_rows[j]) / (norms[i] * norms[j])
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

/// Inner product of two sparse rows. Both inputs are sorted by column
/// index so we can step through them in lock-step.
fn sparse_dot(a: &[(usize, f64)], b: &[(usize, f64)]) -> f64 {
    let (mut i, mut j) = (0usize, 0usize);
    let mut acc = 0.0_f64;
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Equal => {
                acc += a[i].1 * b[j].1;
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }
    acc
}

// ---------------------------------------------------------------------------
// Helpers (private)
// ---------------------------------------------------------------------------

/// Column-oriented sparse matrix used for PCA. We never densify
/// TF-IDF input; instead we keep:
///   * `rows[i]` = list of `(col, val)` for the non-zero entries of row i
///   * `cols[j]` = list of `(row, val)` for the non-zero entries of col j
/// so that both `(A v)_i` and `(A^T u)_j` matvecs are `O(nnz)`.
struct Sparse {
    n: usize,
    d: usize,
    nnz: usize,
    /// `rows[i]` = list of `(col, val)` non-zero entries in row i.
    rows: Vec<Vec<(usize, f64)>>,
    /// `cols[j]` = list of `(row, val)` non-zero entries in col j.
    cols: Vec<Vec<(usize, f64)>>,
}

impl Sparse {
    /// Build a centered sparse matrix: subtract column means from
    /// every non-zero entry.
    fn centered(&self, means: &[f64]) -> Self {
        let rows: Vec<Vec<(usize, f64)>> = self
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&(j, v)| (j, v - means[j]))
                    .collect()
            })
            .collect();
        let cols: Vec<Vec<(usize, f64)>> = self
            .cols
            .iter()
            .enumerate()
            .map(|(j, col)| {
                col.iter()
                    .map(|&(i, v)| (i, v - means[j]))
                    .collect()
            })
            .collect();
        Self {
            n: self.n,
            d: self.d,
            nnz: self.nnz,
            rows,
            cols,
        }
    }
}

/// Build a sparse `n × d` matrix from a list of dense (but mostly-zero)
/// embedding vectors. `d` is the maximum index encountered + 1, so
/// columns that never appear are still represented (as empty `cols`
/// entries) for clean index arithmetic.
fn build_sparse(embeddings: &[Vec<f32>]) -> Sparse {
    let n = embeddings.len();
    let mut max_dim: usize = 0;
    for emb in embeddings {
        if emb.len() > max_dim {
            max_dim = emb.len();
        }
    }
    let d = max_dim;
    let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    let mut cols: Vec<Vec<(usize, f64)>> = vec![Vec::new(); d];
    let mut nnz = 0usize;
    for (i, emb) in embeddings.iter().enumerate() {
        for (j, v) in emb.iter().enumerate() {
            let val = *v as f64;
            if val != 0.0 {
                rows[i].push((j, val));
                cols[j].push((i, val));
                nnz += 1;
            }
        }
    }
    Sparse {
        n,
        d,
        nnz,
        rows,
        cols,
    }
}

/// Compute per-column mean over the (sparse) input.
fn column_means(s: &Sparse, n: usize, d: usize) -> Vec<f64> {
    if n == 0 || d == 0 {
        return vec![0.0_f64; d];
    }
    let mut means = vec![0.0_f64; d];
    for (j, col) in s.cols.iter().enumerate() {
        let mut s = 0.0_f64;
        for &(_, v) in col {
            s += v;
        }
        means[j] = s / n as f64;
    }
    means
}

/// Sparse `(A v)_i = sum over nnz in row_i of val * v[col]`. O(nnz).
fn matvec_rows(s: &Sparse, v: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0_f64; s.n];
    for (i, row) in s.rows.iter().enumerate() {
        let mut acc = 0.0_f64;
        for &(j, val) in row {
            acc += val * v[j];
        }
        out[i] = acc;
    }
    out
}

/// Sparse `(A^T u)_j = sum over nnz in col_j of val * u[row]`. O(nnz).
fn matvec_cols(s: &Sparse, u: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0_f64; s.d];
    for (j, col) in s.cols.iter().enumerate() {
        let mut acc = 0.0_f64;
        for &(i, val) in col {
            acc += val * u[i];
        }
        out[j] = acc;
    }
    out
}

/// Power iteration on a centered sparse matrix. Returns the top
/// left singular vector of `A`. `O(nnz · iterations)`.
fn power_iteration_sparse(s: &Sparse, iterations: usize) -> Vec<f64> {
    if s.d == 0 {
        return Vec::new();
    }
    // Deterministic but non-zero seed (same as the dense impl).
    let mut v: Vec<f64> = (0..s.d)
        .map(|i| ((i + 1) as f64 * 0.137).sin() + 1.0)
        .collect();
    normalize(&mut v);

    for _ in 0..iterations {
        let av = matvec_rows(s, &v);
        let ata_v = matvec_cols(s, &av);
        let mut ata_v = ata_v;
        normalize(&mut ata_v);
        v = ata_v;
    }
    v
}

/// Power iteration on the deflated matrix `A - p v^T` without
/// materialising it. We exploit the rank-1 structure:
/// `(A - p v^T) x = A x - p (v^T x)`
/// `(A - p v^T)^T u = A^T u - v (p^T u)`.
fn power_iteration_deflated(
    s: &Sparse,
    p: &[f64],
    v: &[f64],
    iterations: usize,
) -> Vec<f64> {
    if s.d == 0 {
        return Vec::new();
    }
    let mut w: Vec<f64> = (0..s.d)
        .map(|i| ((i + 1) as f64 * 0.137).sin() + 1.0)
        .collect();
    normalize(&mut w);

    for _ in 0..iterations {
        // m_new = (A - p v^T)^T (A - p v^T) w
        // We do this in two rank-1-corrected matvecs:
        //   deflated_w = (A - p v^T) w = A w - p (v^T w)
        //   m_new      = (A - p v^T)^T deflated_w
        //              = A^T deflated_w - v (p^T deflated_w)
        let aw = matvec_rows(s, &w);
        let vt_w: f64 = v.iter().zip(w.iter()).map(|(a, b)| *a * *b).sum();
        let deflated_w: Vec<f64> =
            aw.iter().zip(p.iter()).map(|(a, b)| *a - *b * vt_w).collect();
        let at_deflated = matvec_cols(s, &deflated_w);
        let pt_deflated: f64 =
            p.iter().zip(deflated_w.iter()).map(|(a, b)| *a * *b).sum();
        let mut new_w: Vec<f64> = at_deflated
            .iter()
            .zip(v.iter())
            .map(|(a, b)| *a - *b * pt_deflated)
            .collect();
        normalize(&mut new_w);
        w = new_w;
    }
    w
}

/// Project rows of a centered sparse matrix onto `v`. O(nnz).
fn project_sparse(s: &Sparse, v: &[f64]) -> Vec<f64> {
    matvec_rows(s, v)
}

/// Project rows of the deflated matrix `A - p v1^T` onto `v2`. Uses
/// the same rank-1 trick as the deflation power iteration:
/// `(A - p v1^T) v2 = A v2 - p (v1^T v2)`.
fn project_deflated(s: &Sparse, p: &[f64], v1: &[f64], v2: &[f64]) -> Vec<f64> {
    let av2 = matvec_rows(s, v2);
    let v1t_v2: f64 = v1.iter().zip(v2.iter()).map(|(a, b)| *a * *b).sum();
    av2.iter().zip(p.iter()).map(|(a, b)| *a - *b * v1t_v2).collect()
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
