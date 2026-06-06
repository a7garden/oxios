//! Agent memory system.
//!
//! Core logic lives in [`oxios_memory`]. This module provides:
//!
//! 1. **Trait bridge** — `impl MemoryStorage for StateStore`, `impl MemoryGit for GitLayer`
//! 2. **Config bridge** — `From<&ConsolidationConfig> for DreamConfig`
//! 3. **Re-exports** — types used by kernel internals and the web surface
//! 4. **Sub-modules** — `markdown_bridge` (orphan-rule wrapper),
//!    `auto_memory_bridge` (oxios_memory re-export)

// ── Trait: StateStore → MemoryStorage ───────────────────────────────

use async_trait::async_trait;
use oxios_memory::memory::storage::{MemoryGit, MemoryStorage};
use serde_json::Value;

#[async_trait]
impl MemoryStorage for crate::state_store::StateStore {
    async fn save_json_value(&self, c: &str, k: &str, v: &Value) -> anyhow::Result<()> {
        self.save_json(c, k, v).await
    }
    async fn load_json_value(&self, c: &str, k: &str) -> anyhow::Result<Option<Value>> {
        self.load_json::<Value>(c, k).await
    }
    async fn list_category(&self, category: &str) -> anyhow::Result<Vec<String>> {
        crate::state_store::StateStore::list_category(self, category).await
    }
    async fn delete_file(&self, category: &str, key: &str) -> anyhow::Result<bool> {
        crate::state_store::StateStore::delete_file(self, category, key).await
    }
}

// ── Trait: GitLayer → MemoryGit ─────────────────────────────────────

#[async_trait]
impl MemoryGit for crate::git_layer::GitLayer {
    async fn commit_file(&self, path: &str, message: &str) -> anyhow::Result<()> {
        crate::git_layer::GitLayer::commit_file(self, path, message)?;
        Ok(())
    }
    fn is_enabled(&self) -> bool {
        crate::git_layer::GitLayer::is_enabled(self)
    }
}

// ── Config bridge: ConsolidationConfig → DreamConfig ────────────────

impl From<&crate::config::ConsolidationConfig> for oxios_memory::memory::dream::DreamConfig {
    fn from(c: &crate::config::ConsolidationConfig) -> Self {
        Self {
            dream_enabled: c.dream_enabled,
            dream_interval_hours: c.dream_interval_hours,
            dream_min_sessions: c.dream_min_sessions,
            hot_max_entries: c.hot_max_entries,
            warm_max_entries: c.warm_max_entries,
            cold_max_entries: c.cold_max_entries,
            hot_token_budget: c.hot_token_budget,
            decay_threshold: c.decay_threshold,
            retention_days: c.retention_days,
            decay_multiplier: c.decay_multiplier,
            auto_protection: c.auto_protection,
            protection_low_access: c.protection_low_access,
            protection_medium_access: c.protection_medium_access,
            protection_high_access: c.protection_high_access,
            protection_medium_sessions: c.protection_medium_sessions,
            protection_high_sessions: c.protection_high_sessions,
            protection_demotion_enabled: c.protection_demotion_enabled,
            protection_demotion_stale_days: c.protection_demotion_stale_days,
            auto_classification: c.auto_classification,
            type_promotion_repetitions: c.type_promotion_repetitions,
            compaction_line_threshold: c.compaction_line_threshold,
            proactive_recall_limit: c.proactive_recall_limit,
            proactive_recall_threshold: c.proactive_recall_threshold,
            pagerank_enabled: true,
            pagerank_damping: 0.85,
            pagerank_iterations: 30,
            pagerank_boost_factor: 0.3,
        }
    }
}

// ── Re-exports ──────────────────────────────────────────────────────
//
// Minimal set: only types used by kernel internals or re-exported
// through lib.rs for the web surface / binary crate.

// Core types (kernel internal consumers)
pub use oxios_memory::memory::manager::MemoryManager;
pub use oxios_memory::memory::types::{
    content_hash, MemoryEntry, MemoryTier, MemoryType, ProtectionLevel, TextVector,
};

// Dream + Proactive (binary crate)
pub use oxios_memory::memory::dream::{DreamCheckpoint, DreamConfig, DreamProcess, DreamReport};
pub use oxios_memory::memory::proactive::{ProactiveRecall, RecallTiming};

// Web surface consumers (embedding viz, HNSW, graph)
pub use oxios_memory::memory::embedding_viz::{
    compute_pca_2d, compute_top_neighbors, MemoryMapEntry, MemoryNeighbor,
};
pub use oxios_memory::memory::hnsw::HnswIndex;
pub use oxios_memory::memory::hnsw_memory_index::{HnswMemoryIndex, SemanticHit};

// SQLite backend (feature-gated) — re-exported through lib.rs
// to avoid duplicate re-exports. Don't re-export here.

// ── Sub-modules ─────────────────────────────────────────────────────

/// Orphan-rule wrapper implementing `MarkdownSource` for `KnowledgeBase`.
pub mod markdown_bridge;

/// Re-export of `oxios_memory::memory::auto_bridge` under the
/// kernel's `memory::` namespace for back-compat.
pub mod auto_memory_bridge {
    pub use oxios_memory::memory::auto_bridge::*;
}
