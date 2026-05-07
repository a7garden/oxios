//! Evaluation cache — avoids re-evaluating the same seed+output pair.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use parking_lot::RwLock;

use crate::evaluation::EvaluationResult;
use crate::protocol::ExecutionResult;
use crate::seed::Seed;

/// Cache key: hash of (seed_id, output_content).
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct EvalKey {
    seed_id: uuid::Uuid,
    output_hash: u64,
}

impl EvalKey {
    fn new(seed: &Seed, execution: &ExecutionResult) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        execution.output.hash(&mut hasher);
        Self {
            seed_id: seed.id,
            output_hash: hasher.finish(),
        }
    }
}

/// In-memory evaluation cache. Same seed + output → same result.
/// Uses RwLock for read-heavy workload.
pub struct EvalCache {
    cache: RwLock<HashMap<EvalKey, EvaluationResult>>,
    max_entries: usize,
}

impl EvalCache {
    /// Create a new cache with the given maximum entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            max_entries,
        }
    }

    /// Look up a cached evaluation.
    pub fn get(&self, seed: &Seed, execution: &ExecutionResult) -> Option<EvaluationResult> {
        let key = EvalKey::new(seed, execution);
        self.cache.read().get(&key).cloned()
    }

    /// Store an evaluation result.
    pub fn put(&self, seed: &Seed, execution: &ExecutionResult, result: EvaluationResult) {
        let key = EvalKey::new(seed, execution);
        let mut cache = self.cache.write();
        if cache.len() >= self.max_entries {
            // FIFO eviction: remove the first entry
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }
        cache.insert(key, result);
    }
}
