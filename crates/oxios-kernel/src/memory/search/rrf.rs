//! Reciprocal Rank Fusion (RRF) for combining multiple search result sets.
//!
//! RRF is a simple yet effective method for merging ranked lists from
//! different retrieval systems. Each result's contribution is `1 / (k + rank)`,
//! where `k` is a constant (typically 60).

use std::collections::HashMap;

/// Fuse multiple ranked result sets using Reciprocal Rank Fusion.
///
/// # Arguments
/// * `results` — Vector of result sets. Each set is `(id, score)` pairs,
///   sorted by relevance (most relevant first).
/// * `k` — RRF constant. Standard value is 60.0. Higher values dampen
///   the effect of individual rank positions.
///
/// # Returns
/// A single merged list of `(id, rrf_score)`, sorted by score descending.
pub fn reciprocal_rank_fusion(results: Vec<Vec<(i64, f64)>>, k: f64) -> Vec<(i64, f64)> {
    let mut scores: HashMap<i64, f64> = HashMap::new();

    for tier_results in &results {
        for (rank, (id, _)) in tier_results.iter().enumerate() {
            *scores.entry(*id).or_default() += 1.0 / (k + rank as f64 + 1.0);
        }
    }

    let mut ranked: Vec<_> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_two_tiers() {
        let tier1 = vec![(1i64, 0.9f64), (2, 0.8), (3, 0.7)];
        let tier2 = vec![(2i64, 10.0f64), (4, 9.0), (1, 8.0)];

        let fused = reciprocal_rank_fusion(vec![tier1, tier2], 60.0);

        // Items appearing in both tiers should rank highest
        assert!(!fused.is_empty());
        // IDs 1 and 2 appear in both → higher score
        let score_1 = fused
            .iter()
            .find(|(id, _)| *id == 1)
            .map(|(_, s)| *s)
            .unwrap_or(0.0);
        let score_4 = fused
            .iter()
            .find(|(id, _)| *id == 4)
            .map(|(_, s)| *s)
            .unwrap_or(0.0);
        assert!(
            score_1 > score_4,
            "ID 1 (in both tiers) should outscore ID 4 (one tier)"
        );
    }

    #[test]
    fn test_rrf_empty_input() {
        let fused = reciprocal_rank_fusion(vec![], 60.0);
        assert!(fused.is_empty());
    }

    #[test]
    fn test_rrf_single_tier() {
        let tier = vec![(10i64, 0.9f64), (20, 0.5)];
        let fused = reciprocal_rank_fusion(vec![tier], 60.0);
        assert_eq!(fused.len(), 2);
        // Rank 0 should have higher score than rank 1
        assert!(fused[0].1 > fused[1].1);
        assert_eq!(fused[0].0, 10);
    }

    #[test]
    fn test_rrf_k_parameter() {
        let tier = vec![(1i64, 1.0f64), (2, 0.5)];
        // Small k → larger score differences
        let fused_small = reciprocal_rank_fusion(vec![tier.clone()], 1.0);
        // Large k → smaller score differences
        let fused_large = reciprocal_rank_fusion(vec![tier], 100.0);

        let diff_small = fused_small[0].1 - fused_small[1].1;
        let diff_large = fused_large[0].1 - fused_large[1].1;
        assert!(
            diff_small > diff_large,
            "Smaller k should produce larger rank differentiation"
        );
    }
}
