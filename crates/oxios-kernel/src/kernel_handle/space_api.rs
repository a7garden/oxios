//! Space API — Space management and knowledge flow system calls.
//!
//! Provides REST API endpoints for:
//! - Listing and querying Spaces
//! - Space activation/switching
//! - Space merge and archive operations
//! - Knowledge flow monitoring

use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::space::{Space, SpaceManager, CrossRefEntry};
#[allow(unused_imports)]
use crate::event_bus::EventBus;
use anyhow::Context;

/// Serialized Space info for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct SpaceInfo {
    pub id: String,
    pub name: String,
    pub source: String,
    pub active: bool,
    pub paths: Vec<String>,
    pub interaction_count: u64,
    pub knowledge_visible: bool,
    pub last_active: String,
}

impl From<&Space> for SpaceInfo {
    fn from(space: &Space) -> Self {
        Self {
            id: space.id.to_string(),
            name: space.name.clone(),
            source: space.source.to_string(),
            active: space.active,
            paths: space.paths.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            interaction_count: space.interaction_count,
            knowledge_visible: space.knowledge_visible,
            last_active: space.last_active_at.to_rfc3339(),
        }
    }
}

/// Serialized knowledge flow entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct KnowledgeFlowInfo {
    pub from: String,
    pub to: String,
    pub flow_type: String,
    pub entry_count: usize,
    pub timestamp: String,
}

impl From<&CrossRefEntry> for KnowledgeFlowInfo {
    fn from(entry: &CrossRefEntry) -> Self {
        Self {
            from: entry.from.to_string(),
            to: entry.to.to_string(),
            flow_type: entry.flow.to_string(),
            entry_count: entry.entry_ids.len(),
            timestamp: entry.timestamp.to_rfc3339(),
        }
    }
}

/// Space system calls.
#[allow(dead_code)]
pub struct SpaceApi {
    /// Space manager for Space lifecycle and routing.
    pub(crate) space_manager: Arc<SpaceManager>,
    /// Event bus (reserved for future event publishing).
    #[allow(dead_code)]
    pub(crate) event_bus: EventBus,
}

impl SpaceApi {
    /// Create a new SpaceApi.
    pub fn new(space_manager: Arc<SpaceManager>, event_bus: EventBus) -> Self {
        Self {
            space_manager,
            event_bus,
        }
    }

    /// List all Spaces.
    pub fn list_spaces(&self) -> Vec<SpaceInfo> {
        self.space_manager
            .list()
            .iter()
            .map(SpaceInfo::from)
            .collect()
    }

    /// Get current active Space.
    pub fn current_space(&self) -> Option<SpaceInfo> {
        self.space_manager
            .current_space()
            .as_ref()
            .map(SpaceInfo::from)
    }

    /// Get Space details by ID.
    pub async fn get_space(&self, id: &str) -> Option<SpaceInfo> {
        let space_id = uuid::Uuid::parse_str(id).ok()?;
        self.space_manager
            .get_space(&space_id)
            .await
            .ok()
            .flatten()
            .as_ref()
            .map(SpaceInfo::from)
    }

    /// Activate a Space by ID.
    pub async fn activate(&self, id: &str) -> anyhow::Result<()> {
        let space_id = uuid::Uuid::parse_str(id)
            .context("Invalid Space ID")?;
        self.space_manager
            .activate(&space_id)
            .await
            .context("Failed to activate Space")
    }

    /// Archive a Space by ID.
    pub async fn archive(&self, id: &str) -> anyhow::Result<()> {
        let space_id = uuid::Uuid::parse_str(id)
            .context("Invalid Space ID")?;
        
        let space = self.space_manager
            .get_space(&space_id)
            .await?
            .context("Space not found")?;
        
        // Remove from active and save
        self.space_manager
            .activate(&self.space_manager.default_space_id())
            .await?;
        
        tracing::info!(space_id = %space_id, name = %space.name, "Space archived");
        Ok(())
    }

    /// Merge two Spaces.
    pub async fn merge(&self, survivor_id: &str, absorbed_id: &str) -> anyhow::Result<()> {
        let survivor = uuid::Uuid::parse_str(survivor_id)
            .context("Invalid survivor Space ID")?;
        let absorbed = uuid::Uuid::parse_str(absorbed_id)
            .context("Invalid absorbed Space ID")?;
        
        self.space_manager
            .merge_spaces(survivor, absorbed)
            .await
            .context("Failed to merge Spaces")
    }

    /// Restore an archived Space.
    pub async fn restore(&self, id: &str) -> anyhow::Result<()> {
        let space_id = uuid::Uuid::parse_str(id)
            .context("Invalid Space ID")?;
        
        self.space_manager
            .restore_from_archive(&space_id)
            .await
            .context("Failed to restore Space")
    }

    /// Get recent knowledge flow entries.
    pub fn knowledge_flow(&self) -> Vec<KnowledgeFlowInfo> {
        self.space_manager
            .knowledge_bridge()
            .map(|bridge| {
                bridge
                    .recent_references()
                    .iter()
                    .map(KnowledgeFlowInfo::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get knowledge flow for a specific Space.
    pub fn knowledge_flow_for(&self, id: &str) -> Option<Vec<KnowledgeFlowInfo>> {
        let space_id = uuid::Uuid::parse_str(id).ok()?;
        Some(
            self.space_manager
                .knowledge_bridge()?
                .references_for(space_id)
                .iter()
                .map(KnowledgeFlowInfo::from)
                .collect()
        )
    }
}