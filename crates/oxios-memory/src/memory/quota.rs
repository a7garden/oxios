#![allow(missing_docs)]
//! Memory budget and curation types.

use serde::{Deserialize, Serialize};

use super::types::MemoryType;

/// Budget for memory curation — limits per type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBudget {
    /// Maximum entries per memory type.
    pub max_per_type: usize,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self { max_per_type: 100 }
    }
}

/// A single candidate for removal during curation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationCandidate {
    /// Memory entry ID.
    pub id: String,
    /// Memory type.
    pub memory_type: MemoryType,
    /// Effective importance score (lower = more likely removed).
    pub effective_importance: f32,
}

/// Report from a curation run.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CurationReport {
    /// Total entries before curation.
    pub total_before: usize,
    /// Total entries after curation.
    pub total_after: usize,
    /// Number of entries actually removed.
    pub removed: usize,
    /// Candidates identified for removal.
    pub candidates_for_removal: Vec<CurationCandidate>,
}
