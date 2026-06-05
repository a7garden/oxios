//! Agent memory system (kernel-side shim).
//!
//! Most memory logic now lives in `oxios-memory` (RFC-018 b.1-b.7).
//! This module is a back-compat shim that:
//! - Re-exports `oxios_memory::memory::*` types so existing
//!   `use oxios_kernel::memory::MemoryType;` paths still work.
//! - Keeps `auto_memory_bridge` and `dream` (these move in b.9).
//! - Re-exports `MemoryManager` from `oxios_memory` (moved in b.7).

// ─── Back-compat re-exports from oxios-memory ───────────────────────────
pub use oxios_memory::memory::{
    AutoClassifier, AutoProtector, CompactionTree, CurationCandidate, CurationReport,
    DecayEngine, HnswIndex, HnswMemoryIndex, MemoryBudget, MemoryEntry, MemoryManager, MemoryTier,
    MemoryType, ProtectionLevel, RecallTiming, RootEntry, RootIndex, SemanticHit, SonaEngine,
    TopicEntry,
};

// ─── Kernel-side modules (b.9 scope) ───────────────────────────────────
pub mod auto_memory_bridge;
pub mod dream;

pub use dream::{DreamCheckpoint, DreamProcess, DreamReport};

// ─── Test-only helper: content_hash used by `MemoryManager::is_duplicate`
//     (still in kernel's mod.rs for back-compat; the real impl lives in
//     oxios-memory::memory::helpers).
#[doc(hidden)]
pub fn content_hash(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}
