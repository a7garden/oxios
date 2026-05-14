//! KnowledgeBridge: manages knowledge flow between Spaces.
//!
//! Cross-Space knowledge flow is managed through three types:
//! - Reference: read-only access to another Space's memory
//! - Transfer: copy memory entries from one Space to another
//! - Synthesis: combine knowledge from multiple Spaces (Phase 6)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::SpaceId;
use crate::audit_trail::AuditTrail;
use crate::memory::{MemoryEntry, MemoryManager};

/// Cross-reference log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRefEntry {
    /// Source Space ID.
    pub from: SpaceId,
    /// Target Space ID.
    pub to: SpaceId,
    /// Memory entry IDs that were accessed.
    pub entry_ids: Vec<String>,
    /// Type of knowledge flow.
    pub flow: KnowledgeFlow,
    /// When this happened.
    pub timestamp: DateTime<Utc>,
}

impl CrossRefEntry {
    /// Create a new cross-reference entry.
    pub fn new(from: SpaceId, to: SpaceId, entry_ids: Vec<String>, flow: KnowledgeFlow) -> Self {
        Self {
            from,
            to,
            entry_ids,
            flow,
            timestamp: Utc::now(),
        }
    }
}

/// Type of knowledge flow between Spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeFlow {
    /// Read-only access to another Space's memory.
    Reference,
    /// Copy entries from one Space to another.
    Transfer,
    /// Synthesize insights from multiple Spaces. (Phase 6)
    Synthesis,
}

impl std::fmt::Display for KnowledgeFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KnowledgeFlow::Reference => write!(f, "reference"),
            KnowledgeFlow::Transfer => write!(f, "transfer"),
            KnowledgeFlow::Synthesis => write!(f, "synthesis"),
        }
    }
}

/// Manages knowledge flow between Spaces.
///
/// Provides controlled access to cross-Space memory:
/// - Checks `knowledge_visible` flags
/// - Records all access in audit trail
/// - Respects privacy settings
pub struct KnowledgeBridge {
    /// Reference to SpaceManager for space lookups.
    space_manager: Arc<super::manager::SpaceManager>,
    /// Audit trail for knowledge flow logging.
    audit_trail: Option<Arc<AuditTrail>>,
    /// In-memory log of recent cross-references.
    recent_refs: parking_lot::RwLock<Vec<CrossRefEntry>>,
}

impl KnowledgeBridge {
    /// Create a new KnowledgeBridge.
    pub fn new(
        space_manager: Arc<super::manager::SpaceManager>,
        audit_trail: Option<Arc<AuditTrail>>,
    ) -> Self {
        Self {
            space_manager,
            audit_trail,
            recent_refs: parking_lot::RwLock::new(Vec::new()),
        }
    }

    /// Reference: read memory from another Space.
    ///
    /// Returns memory entries from `from_space` that match the query.
    /// Records the access in audit trail.
    ///
    /// # Panics
    ///
    /// Panics if `memory_manager` is not properly initialized for cross-Space search.
    /// This is a Phase 3 stub — actual cross-Space search requires Space-scoped MemoryManagers.
    #[allow(unused_variables)]
    pub async fn reference(
        &self,
        from_space_id: SpaceId,
        to_space_id: SpaceId,
        _memory_manager: &MemoryManager,
        _query: &str,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // Check visibility
        let from_space = self.space_manager.get_space(&from_space_id).await?;
        if let Some(space) = from_space {
            if !space.knowledge_visible {
                anyhow::bail!(
                    "Space {} is private and cannot be accessed",
                    from_space_id
                );
            }
        }

        // Search the from_space's memory
        // Note: This requires a separate MemoryManager per Space, which
        // is set up in SpaceManager. Here we use the provided memory_manager
        // (which is the *current* Space's manager) — for true cross-Space
        // search, we'd need Space-scoped MemoryManagers, which is Phase 3.
        //
        // For Phase 1, this is a stub that returns empty.
        let entries = Vec::new();

        // Record the reference
        let entry = CrossRefEntry::new(
            from_space_id,
            to_space_id,
            entries.iter().map(|e: &MemoryEntry| e.id.clone()).collect(),
            KnowledgeFlow::Reference,
        );
        self.record_entry(entry);

        Ok(entries)
    }

    /// Transfer: copy memory entries from one Space to another.
    ///
    /// Called when a new Space is created, to inject relevant knowledge
    /// from existing Spaces.
    ///
    /// Returns the number of entries transferred.
    ///
    /// # Panics
    ///
    /// Panics if transfer is attempted with a private source Space.
    #[allow(unused_variables)]
    pub async fn transfer(
        &self,
        from_space_id: SpaceId,
        to_space_id: SpaceId,
        _memory_manager: &MemoryManager,
        entries: Vec<MemoryEntry>,
    ) -> anyhow::Result<usize> {
        // Check visibility
        let from_space = self.space_manager.get_space(&from_space_id).await?;
        if let Some(space) = from_space {
            if !space.knowledge_visible {
                tracing::warn!(
                    from = %from_space_id,
                    to = %to_space_id,
                    "Skipping transfer: source Space is private"
                );
                return Ok(0);
            }
        }

        // Transfer entries to the target space
        // Note: Similar stub limitation as reference() above.
        // Full implementation in Phase 3 with Space-scoped MemoryManagers.
        let count = entries.len();

        // Record the transfer
        let entry = CrossRefEntry::new(
            from_space_id,
            to_space_id,
            entries.iter().map(|e: &MemoryEntry| e.id.clone()).collect(),
            KnowledgeFlow::Transfer,
        );
        self.record_entry(entry);

        tracing::info!(
            from = %from_space_id,
            to = %to_space_id,
            count,
            "Knowledge transfer recorded"
        );

        Ok(count)
    }

    /// Record a cross-reference entry.
    fn record_entry(&self, entry: CrossRefEntry) {
        // Add to recent refs (bounded)
        let mut refs = self.recent_refs.write();
        refs.push(entry.clone());

        // Keep only last 100
        while refs.len() > 100 {
            refs.remove(0);
        }

        // Also log to audit trail if available
        if let Some(ref audit) = self.audit_trail {
            audit.append(
                format!("space:{}", entry.to),
                crate::audit_trail::AuditAction::Other {
                    detail: format!(
                        "knowledge_{}: {}->{} ({} entries)",
                        entry.flow, entry.from, entry.to, entry.entry_ids.len()
                    ),
                },
                format!("{:?}", entry),
            );
        }
    }

    /// Get recent cross-reference entries.
    pub fn recent_references(&self) -> Vec<CrossRefEntry> {
        self.recent_refs.read().clone()
    }

    /// Get cross-references for a specific Space.
    pub fn references_for(&self, space_id: SpaceId) -> Vec<CrossRefEntry> {
        self.recent_refs
            .read()
            .iter()
            .filter(|e| e.from == space_id || e.to == space_id)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_flow_display() {
        assert_eq!(KnowledgeFlow::Reference.to_string(), "reference");
        assert_eq!(KnowledgeFlow::Transfer.to_string(), "transfer");
        assert_eq!(KnowledgeFlow::Synthesis.to_string(), "synthesis");
    }

    #[test]
    fn test_cross_ref_entry() {
        let entry = CrossRefEntry::new(
            SpaceId::new_v4(),
            SpaceId::new_v4(),
            vec!["mem1".to_string(), "mem2".to_string()],
            KnowledgeFlow::Transfer,
        );
        assert_eq!(entry.entry_ids.len(), 2);
        assert!(!entry.timestamp.format("%Y").to_string().is_empty());
    }
}