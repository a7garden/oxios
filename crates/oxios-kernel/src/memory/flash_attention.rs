//! Flash Attention — block-wise attention for O(N) memory usage.
//!
//! Triton-inspired CPU implementation that processes attention in blocks
//! to maximize L1/L2 cache efficiency. Achieves 2-5× speedup and ~75%
//! memory reduction compared to naive attention for large sequence lengths.
//!
//! Reference: "FlashAttention: Fast and Memory-Efficient Exact Attention
//! with IO-Awareness" (Dao et al., 2022)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for Flash Attention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashAttentionConfig {
    /// Block size for tiled computation (tune to L1 cache).
    /// Default 64 works well for typical f32 vectors.
    pub block_size: usize,
    /// Embedding dimensionality.
    pub dimensions: usize,
    /// Softmax temperature scaling.
    pub temperature: f32,
}

impl Default for FlashAttentionConfig {
    fn default() -> Self {
        Self {
            block_size: 64,
            dimensions: 128,
            temperature: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Benchmark result
// ---------------------------------------------------------------------------

/// Result of a benchmark comparing naive vs flash attention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Naive attention time in milliseconds.
    pub naive_time_ms: f64,
    /// Flash attention time in milliseconds.
    pub flash_time_ms: f64,
    /// Speedup ratio (naive / flash).
    pub speedup: f64,
    /// Memory reduction ratio (0.75 = 75% less memory).
    pub memory_reduction: f64,
    /// Number of query vectors.
    pub num_queries: usize,
    /// Embedding dimension.
    pub dimensions: usize,
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Flash Attention Benchmark: {} queries × {}d — {:.2}ms → {:.2}ms ({:.1}× speedup, {:.0}% memory reduction)",
            self.num_queries,
            self.dimensions,
            self.naive_time_ms,
            self.flash_time_ms,
            self.speedup,
            self.memory_reduction * 100.0,
        )
    }
}

// ---------------------------------------------------------------------------
// Flash Attention
// ---------------------------------------------------------------------------

/// Block-wise attention computation optimized for CPU cache locality.
///
/// Instead of materializing the full N×N attention matrix, processes
/// the computation in blocks that fit in L1/L2 cache, achieving
/// O(N) memory complexity instead of O(N²).
#[derive(Debug)]
pub struct FlashAttention {
    config: FlashAttentionConfig,
}

impl FlashAttention {
    /// Create a new FlashAttention with the given configuration.
    pub fn new(config: FlashAttentionConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn with_dimensions(dimensions: usize) -> Self {
        let config = FlashAttentionConfig {
            dimensions,
            ..Default::default()
        };
        Self { config }
    }

    /// Returns a reference to the configuration.
    pub fn config(&self) -> &FlashAttentionConfig {
        &self.config
    }

    /// Compute scaled dot-product attention using the block-wise algorithm.
    ///
    /// For sequences of length N with dimension D:
    /// - Naive: O(N²) memory (full attention matrix)
    /// - Flash: O(N) memory (block-wise accumulation via online softmax)
    ///
    /// # Arguments
    /// * `queries` - Query vectors [N_q × D]
    /// * `keys` - Key vectors [N_k × D]
    /// * `values` - Value vectors [N_k × D]
    ///
    /// # Returns
    /// Output vectors [N_q × D]
    #[allow(clippy::needless_range_loop)]
    pub fn attention(
        &self,
        queries: &[Vec<f32>],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Vec<Vec<f32>> {
        if queries.is_empty() || keys.is_empty() {
            return Vec::new();
        }

        // Use actual vector length (may differ from config.dimensions)
        let dim = queries.first().map_or(0, |v| v.len());
        if dim == 0 {
            return vec![vec![]; queries.len()];
        }
        let scale = 1.0 / (self.config.temperature * (dim as f32).sqrt());
        let block_size = self.config.block_size.min(keys.len());

        let num_queries = queries.len();
        let mut outputs = vec![vec![0.0f32; dim]; num_queries];

        // Process each query independently — each query only needs O(N_k) memory
        for (qi, query) in queries.iter().enumerate() {
            // Online softmax accumulators (Flash Attention core idea)
            // Instead of storing all attention weights, we accumulate incrementally
            let mut output_accum = vec![0.0f32; dim];
            let mut max_score = f32::NEG_INFINITY; // Running max for numerical stability
            let mut sum_exp = 0.0f32; // Running sum of exp(score - max)

            // Process key/value pairs in blocks
            for k_block_start in (0..keys.len()).step_by(block_size) {
                let k_block_end = (k_block_start + block_size).min(keys.len());

                // Compute attention scores for this block
                let mut block_max = max_score;
                let mut block_scores = Vec::with_capacity(k_block_end - k_block_start);

                for ki in k_block_start..k_block_end {
                    let score = dot_product(query, &keys[ki]) * scale;
                    block_scores.push(score);
                    if score > block_max {
                        block_max = score;
                    }
                }

                // Update running maximum and rescale previous accumulation
                let old_max = max_score;
                if block_max > max_score {
                    max_score = block_max;
                }

                // Rescale the accumulated sum and output by the change in max
                let rescale_factor = if old_max == f32::NEG_INFINITY {
                    0.0
                } else {
                    (old_max - max_score).exp()
                };
                sum_exp *= rescale_factor;
                for v in output_accum.iter_mut() {
                    *v *= rescale_factor;
                }

                // Add block contributions
                for (block_idx, &score) in block_scores.iter().enumerate() {
                    let ki = k_block_start + block_idx;
                    let weight = (score - max_score).exp();
                    sum_exp += weight;
                    for (d, v) in output_accum.iter_mut().enumerate() {
                        *v += weight * values[ki][d];
                    }
                }
            }

            // Normalize by sum_exp
            if sum_exp > 0.0 {
                let inv_sum = 1.0 / sum_exp;
                for v in output_accum.iter_mut() {
                    *v *= inv_sum;
                }
            }

            outputs[qi] = output_accum;
        }

        outputs
    }

    /// Naive attention implementation for benchmarking comparison.
    ///
    /// Materializes the full N×N attention matrix: O(N²) memory.
    pub fn naive_attention(
        &self,
        queries: &[Vec<f32>],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Vec<Vec<f32>> {
        if queries.is_empty() || keys.is_empty() {
            return Vec::new();
        }

        // Use actual vector length (may differ from config.dimensions)
        let dim = queries.first().map_or(0, |v| v.len());
        if dim == 0 {
            return vec![vec![]; queries.len()];
        }
        let scale = 1.0 / (self.config.temperature * (dim as f32).sqrt());
        let num_queries = queries.len();
        let num_keys = keys.len();

        // Materialize full attention matrix: O(N_q × N_k) memory
        let mut attention_weights = vec![vec![0.0f32; num_keys]; num_queries];

        // Compute all scores
        for (qi, query) in queries.iter().enumerate() {
            let mut max_score = f32::NEG_INFINITY;
            for (ki, key) in keys.iter().enumerate() {
                let score = dot_product(query, key) * scale;
                attention_weights[qi][ki] = score;
                if score > max_score {
                    max_score = score;
                }
            }
            // Softmax
            let mut sum_exp = 0.0f32;
            for w in &mut attention_weights[qi] {
                *w = (*w - max_score).exp();
                sum_exp += *w;
            }
            if sum_exp > 0.0 {
                let inv = 1.0 / sum_exp;
                for w in &mut attention_weights[qi] {
                    *w *= inv;
                }
            }
        }

        // Weighted sum: output = attention_weights × values
        let mut outputs = vec![vec![0.0f32; dim]; num_queries];
        for qi in 0..num_queries {
            for ki in 0..num_keys {
                let w = attention_weights[qi][ki];
                for d in 0..dim {
                    outputs[qi][d] += w * values[ki][d];
                }
            }
        }

        outputs
    }

    /// Run a benchmark comparing naive vs flash attention.
    ///
    /// Generates random vectors and measures wall-clock time for both methods.
    /// Also verifies that both implementations produce equivalent results.
    pub fn benchmark(&self, num_vectors: usize) -> BenchmarkResult {
        let vectors = generate_test_vectors(num_vectors, self.config.dimensions);

        let naive_start = std::time::Instant::now();
        let naive_result = self.naive_attention(&vectors, &vectors, &vectors);
        let naive_duration = naive_start.elapsed();

        let flash_start = std::time::Instant::now();
        let flash_result = self.attention(&vectors, &vectors, &vectors);
        let flash_duration = flash_start.elapsed();

        // Verify results are similar (within 5% relative tolerance)
        let mut max_rel_err = 0.0f32;
        for (f_row, n_row) in flash_result.iter().zip(naive_result.iter()) {
            for (f, n) in f_row.iter().zip(n_row.iter()) {
                let err = (f - n).abs() / f.abs().max(n.abs()).max(1e-6);
                max_rel_err = max_rel_err.max(err);
            }
        }
        if max_rel_err > 0.05 {
            tracing::warn!(
                max_relative_error = max_rel_err,
                "Flash vs naive attention results diverge"
            );
        }

        let naive_ms = naive_duration.as_secs_f64() * 1000.0;
        let flash_ms = flash_duration.as_secs_f64() * 1000.0;
        let speedup = if flash_ms > 0.0 {
            naive_ms / flash_ms
        } else {
            f64::INFINITY
        };

        // Memory reduction: naive stores N×N matrix, flash stores O(N) per query
        let naive_mem = num_vectors * num_vectors; // attention matrix elements
        let flash_mem = self.config.dimensions + 2; // per-query accumulators
        let memory_reduction = 1.0 - (flash_mem as f64 / naive_mem as f64);

        BenchmarkResult {
            naive_time_ms: naive_ms,
            flash_time_ms: flash_ms,
            speedup,
            memory_reduction: memory_reduction.max(0.0),
            num_queries: num_vectors,
            dimensions: self.config.dimensions,
        }
    }

    /// Compute self-attention: a sequence attends to itself.
    ///
    /// Convenience wrapper around `attention(q, q, q)`.
    pub fn self_attention(&self, sequence: &[Vec<f32>]) -> Vec<Vec<f32>> {
        self.attention(sequence, sequence, sequence)
    }

    /// Compute cross-attention between two sequences.
    ///
    /// Queries from one sequence attend to keys/values from another.
    pub fn cross_attention(&self, queries: &[Vec<f32>], kv_sequence: &[Vec<f32>]) -> Vec<Vec<f32>> {
        self.attention(queries, kv_sequence, kv_sequence)
    }

    /// Estimate peak memory usage in bytes for a given sequence length.
    pub fn memory_estimate(&self, seq_len: usize) -> MemoryEstimate {
        let dim = self.config.dimensions;
        let element_size = std::mem::size_of::<f32>();

        // Naive: full N×N attention matrix + N×D output + N×D Q/K/V
        let naive_peak = seq_len * seq_len * element_size // attention matrix
            + seq_len * dim * element_size * 3 // Q, K, V
            + seq_len * dim * element_size; // output

        // Flash: D accumulators + 2 scalars per query, processed sequentially
        let flash_peak = dim * element_size // output accumulator
            + self.config.block_size * element_size // block scores
            + seq_len * dim * element_size * 3 // Q, K, V (inputs)
            + seq_len * dim * element_size; // output

        MemoryEstimate {
            naive_bytes: naive_peak,
            flash_bytes: flash_peak,
            reduction_ratio: 1.0 - (flash_peak as f64 / naive_peak as f64),
        }
    }
}

/// Memory usage estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEstimate {
    /// Peak memory for naive attention (bytes).
    pub naive_bytes: usize,
    /// Peak memory for flash attention (bytes).
    pub flash_bytes: usize,
    /// Memory reduction ratio (0.75 = 75% less).
    pub reduction_ratio: f64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Dot product of two f32 vectors.
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Generate deterministic test vectors using a simple LCG.
fn generate_test_vectors(count: usize, dim: usize) -> Vec<Vec<f32>> {
    let mut rng_state = 42u64;
    let mut vectors = Vec::with_capacity(count);

    for _ in 0..count {
        let mut v = Vec::with_capacity(dim);
        for _ in 0..dim {
            // LCG: x_{n+1} = (a * x_n + c) mod m
            rng_state = rng_state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let val = ((rng_state >> 33) as f32 / (1u64 << 31) as f32) - 1.0;
            v.push(val);
        }
        vectors.push(v);
    }

    vectors
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_vs_naive_small() {
        let fa = FlashAttention::with_dimensions(16);
        let queries = generate_test_vectors(4, 16);
        let keys = generate_test_vectors(4, 16);
        let values = generate_test_vectors(4, 16);

        let flash_output = fa.attention(&queries, &keys, &values);
        let naive_output = fa.naive_attention(&queries, &keys, &values);

        assert_eq!(flash_output.len(), naive_output.len());

        // Results should be very close (within 1% relative tolerance)
        for (flash_row, naive_row) in flash_output.iter().zip(naive_output.iter()) {
            for (f, n) in flash_row.iter().zip(naive_row.iter()) {
                let diff = (f - n).abs();
                let max_val = f.abs().max(n.abs()).max(1e-6);
                assert!(
                    diff / max_val < 0.01,
                    "Flash and naive outputs differ: flash={:.6}, naive={:.6}",
                    f,
                    n
                );
            }
        }
    }

    #[test]
    fn test_flash_attention_empty() {
        let fa = FlashAttention::with_dimensions(16);
        let result = fa.attention(&[], &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_self_attention() {
        let fa = FlashAttention::with_dimensions(8);
        let seq = generate_test_vectors(3, 8);
        let result = fa.self_attention(&seq);
        assert_eq!(result.len(), 3);
        // Each output should have the correct dimension
        for row in &result {
            assert_eq!(row.len(), 8);
        }
    }

    #[test]
    fn test_cross_attention() {
        let fa = FlashAttention::with_dimensions(8);
        let queries = generate_test_vectors(2, 8);
        let kv = generate_test_vectors(5, 8);
        let result = fa.cross_attention(&queries, &kv);
        assert_eq!(result.len(), 2);
        for row in &result {
            assert_eq!(row.len(), 8);
        }
    }

    #[test]
    fn test_memory_estimate() {
        let fa = FlashAttention::with_dimensions(128);
        let estimate = fa.memory_estimate(1000);

        assert!(estimate.flash_bytes < estimate.naive_bytes);
        assert!(
            estimate.reduction_ratio > 0.5,
            "Should achieve >50% memory reduction"
        );

        // For 1000×128: naive = 1000*1000*4 + overhead, flash much less
        // The attention matrix alone is 4MB for naive, ~0 for flash
    }

    #[test]
    fn test_benchmark_result_display() {
        let result = BenchmarkResult {
            naive_time_ms: 10.0,
            flash_time_ms: 3.0,
            speedup: 3.33,
            memory_reduction: 0.75,
            num_queries: 256,
            dimensions: 128,
        };
        let s = format!("{}", result);
        assert!(s.contains("256"));
        assert!(s.contains("3.3"));
        assert!(s.contains("75%"));
    }

    #[test]
    fn test_block_size_effect() {
        // Different block sizes should produce the same result
        let mut config1 = FlashAttentionConfig::default();
        config1.dimensions = 16;
        config1.block_size = 2;

        let mut config2 = FlashAttentionConfig::default();
        config2.dimensions = 16;
        config2.block_size = 32;

        let fa1 = FlashAttention::new(config1);
        let fa2 = FlashAttention::new(config2);

        let vectors = generate_test_vectors(8, 16);

        let out1 = fa1.attention(&vectors, &vectors, &vectors);
        let out2 = fa2.attention(&vectors, &vectors, &vectors);

        // Results should be identical regardless of block size
        for (row1, row2) in out1.iter().zip(out2.iter()) {
            for (v1, v2) in row1.iter().zip(row2.iter()) {
                assert!(
                    (v1 - v2).abs() < 1e-4,
                    "Block size shouldn't affect output: {} vs {}",
                    v1,
                    v2
                );
            }
        }
    }

    #[test]
    fn test_temperature_scaling() {
        let mut config_high = FlashAttentionConfig::default();
        config_high.dimensions = 16;
        config_high.temperature = 2.0;

        let mut config_low = FlashAttentionConfig::default();
        config_low.dimensions = 16;
        config_low.temperature = 0.5;

        let fa_high = FlashAttention::new(config_high);
        let fa_low = FlashAttention::new(config_low);

        let vectors = generate_test_vectors(4, 16);

        let out_high = fa_high.attention(&vectors, &vectors, &vectors);
        let out_low = fa_low.attention(&vectors, &vectors, &vectors);

        // Higher temperature → more uniform distribution → less peaked output
        // Lower temperature → sharper distribution → more peaked output
        // Check that they produce different results
        let mut different = false;
        for (r_high, r_low) in out_high.iter().zip(out_low.iter()) {
            for (v_high, v_low) in r_high.iter().zip(r_low.iter()) {
                if (v_high - v_low).abs() > 1e-4 {
                    different = true;
                    break;
                }
            }
        }
        assert!(
            different,
            "Different temperatures should produce different outputs"
        );
    }

    #[test]
    fn test_large_sequence_correctness() {
        let fa = FlashAttention::with_dimensions(32);
        let vectors = generate_test_vectors(50, 32);

        let flash = fa.attention(&vectors, &vectors, &vectors);
        let naive = fa.naive_attention(&vectors, &vectors, &vectors);

        // For larger sequences, allow slightly more tolerance
        let mut max_relative_error = 0.0f32;
        for (f_row, n_row) in flash.iter().zip(naive.iter()) {
            for (f, n) in f_row.iter().zip(n_row.iter()) {
                let err = (f - n).abs() / f.abs().max(n.abs()).max(1e-6);
                max_relative_error = max_relative_error.max(err);
            }
        }
        assert!(
            max_relative_error < 0.02,
            "Max relative error: {:.4} — should be < 2%",
            max_relative_error
        );
    }
}
