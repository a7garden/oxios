//! SpaceManager: manages Space lifecycle and routing.
//!
//! The SpaceManager is the core component for Space system:
//! - Detects which Space a message belongs to (3-layer strategy)
//! - Creates/manages Spaces (auto + manual)
//! - Routes messages to the appropriate Space
//! - Manages memory flow via SpaceBridge

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use parking_lot::RwLock;
use tokio::sync::Mutex;

use super::conversation_buffer::{ConversationBuffer, ConversationTurn};
use super::space_bridge::SpaceBridge;
use super::{
    detection::{self, PathMatcher, Topic},
    Space, SpaceId, SpaceSource,
};
use crate::event_bus::{EventBus, KernelEvent};
use crate::state_store::StateStore;

const MAX_ARCHIVE_AGE_DAYS: i64 = 30;
#[allow(dead_code)]
const DEFAULT_WORKSPACE_DIR: &str = ".oxios/spaces";

/// Errors from SpaceManager operations.
#[derive(thiserror::Error, Debug)]
pub enum SpaceManagerError {
    /// Space not found.
    #[error("Space not found: {0}")]
    NotFound(SpaceId),
    /// Cannot merge a Space with itself.
    #[error("Cannot merge a Space with itself")]
    SelfMerge,
    /// Space is private and cannot be accessed.
    #[error("Space is private and cannot be accessed: {0}")]
    Private(SpaceId),
}

impl SpaceManagerError {
    /// Whether this error should abort the current operation gracefully.
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::SelfMerge)
    }
}

/// Manages Spaces and routes messages to appropriate Spaces.
pub struct SpaceManager {
    /// In-memory index of all Spaces.
    spaces: RwLock<HashMap<SpaceId, Space>>,
    /// The currently active Space ID.
    current_space_id: RwLock<SpaceId>,
    /// State store for persistence.
    state_store: Arc<StateStore>,
    /// Event bus for publishing Space events.
    /// Number of turns since last topic check.
    #[allow(dead_code)]
    event_bus: EventBus,
    /// Path matcher for Layer 1 detection.
    path_matcher: RwLock<PathMatcher>,
    /// Conversation buffer reference for detection.
    buffer: Arc<Mutex<ConversationBuffer>>,
    /// Memory bridge for cross-Space memory flow.
    memory_bridge: Option<Arc<SpaceBridge>>,
    /// Root directory for all Space data.
    root_dir: PathBuf,
    /// Number of turns since last topic check.
    /// Number of turns since last topic check.
    #[allow(dead_code)]
    turns_since_topic_check: Mutex<usize>,
}

/// Get the default Space ID.
fn default_space_id() -> SpaceId {
    *crate::space::DEFAULT_SPACE_ID
        .get_or_init(|| uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap())
}

impl SpaceManager {
    /// Get the default Space ID (for tests only).
    #[allow(missing_docs)]
    #[cfg(test)]
    pub fn default_space_id_for_tests() -> SpaceId {
        // Used only in tests to avoid OnceLock issues
        uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
    }
}

impl SpaceManager {
    /// Create a new SpaceManager.
    ///
    /// Initializes from state store, creates default Space if none exist.
    pub async fn new(state_store: Arc<StateStore>, event_bus: EventBus) -> Result<Self> {
        let root_dir = Self::default_root_dir();
        let this = Self {
            spaces: RwLock::new(HashMap::new()),
            current_space_id: RwLock::new(default_space_id()),
            state_store,
            event_bus,
            path_matcher: RwLock::new(PathMatcher::default()),
            buffer: Arc::new(Mutex::new(ConversationBuffer::default())),
            memory_bridge: None,
            root_dir,
            turns_since_topic_check: Mutex::new(0),
        };

        this.load_spaces().await?;
        this.ensure_default_space().await?;
        this.reindex_path_matcher();

        Ok(this)
    }

    /// Set the memory bridge (called after construction).
    pub fn set_memory_bridge(&mut self, bridge: Arc<SpaceBridge>) {
        self.memory_bridge = Some(bridge);
    }

    /// Get the default root directory for Space data.
    fn default_root_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".oxios")
            .join("spaces")
    }

    /// Load all Spaces from state store.
    async fn load_spaces(&self) -> Result<()> {
        let spaces_dir = &self.root_dir;

        if !spaces_dir.exists() {
            std::fs::create_dir_all(spaces_dir)?;
            return Ok(());
        }

        // Load index
        let index_path = spaces_dir.join("_index.json");
        if index_path.exists() {
            let ids: Vec<SpaceId> = match self.state_store.load_json("_spaces", "_index.json").await
            {
                Ok(Some(ids)) => ids,
                Ok(None) => Vec::new(),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to load Space index, starting fresh");
                    Vec::new()
                }
            };
            for id in ids {
                let path = spaces_dir.join(id.to_string()).join("space.json");
                if path.exists() {
                    if let Ok(space) = self.load_space_from_file(&path).await {
                        self.spaces.write().insert(space.id, space);
                    }
                }
            }
        }

        tracing::info!(count = self.spaces.read().len(), "Loaded Spaces from disk");
        Ok(())
    }

    /// Load a single Space from file.
    async fn load_space_from_file(&self, path: &PathBuf) -> Result<Space> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let space: Space = serde_json::from_str(&content)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(space)
    }

    /// Ensure the default Space exists.
    #[allow(clippy::await_holding_lock)]
    async fn ensure_default_space(&self) -> Result<()> {
        let spaces = self.spaces.read();
        if spaces.contains_key(&default_space_id()) {
            return Ok(());
        }
        drop(spaces);

        // Create default Space
        let default = Space {
            id: default_space_id(),
            name: String::new(), // Unnamed
            source: SpaceSource::Manual,
            paths: Vec::new(),
            workspace_dir: self.default_workspace_dir(&default_space_id()),
            tags: Vec::new(),
            active: true,
            created_at: Utc::now(),
            last_active_at: Utc::now(),
            interaction_count: 0,
            memory_visible: true,
        };

        self.add_space(default).await
    }

    /// Get the workspace directory for a Space.
    pub fn default_workspace_dir(&self, space_id: &SpaceId) -> PathBuf {
        self.root_dir.join(space_id.to_string()).join("workspace")
    }

    /// Add a new Space (creates workspace dir + saves).
    async fn add_space(&self, mut space: Space) -> Result<()> {
        // Create workspace directory
        let ws_dir = &space.workspace_dir;
        if !ws_dir.exists() {
            std::fs::create_dir_all(ws_dir)?;
        }

        // Save to state store
        self.save_space(&space).await?;

        // Add to memory index
        let mut spaces = self.spaces.write();
        if space.active {
            // Deactivate all others
            for s in spaces.values_mut() {
                s.deactivate();
            }
            space.activate();
        }
        spaces.insert(space.id, space);

        // Reindex path matcher
        drop(spaces);
        self.reindex_path_matcher();

        Ok(())
    }

    /// Save a Space to disk.
    async fn save_space(&self, space: &Space) -> Result<()> {
        let space_dir = self.root_dir.join(space.id.to_string());
        let space_file = space_dir.join("space.json");

        if !space_dir.exists() {
            std::fs::create_dir_all(&space_dir)?;
        }

        let json = serde_json::to_string_pretty(space)?;
        std::fs::write(&space_file, json)?;

        // Update index
        self.save_index().await?;

        Ok(())
    }

    /// Save the Space index to disk.
    async fn save_index(&self) -> Result<()> {
        let ids: Vec<SpaceId> = self.spaces.read().keys().cloned().collect();
        let index_path = self.root_dir.join("_index.json");
        let json = serde_json::to_string_pretty(&ids)?;
        std::fs::write(index_path, json)?;
        Ok(())
    }

    /// Reindex all Spaces into the path matcher.
    fn reindex_path_matcher(&self) {
        let spaces = self.spaces.read();
        let mut matcher = self.path_matcher.write();
        *matcher = PathMatcher::default();
        for space in spaces.values() {
            matcher.register(space);
        }
    }

    /// Detect or create the appropriate Space for a message.
    ///
    /// Implements the 3-layer detection strategy:
    /// 1. Filesystem path extraction (fast, free)
    /// 2. Keyword/tag matching (fast, free)
    /// 3. LLM topic classification (slow, only when needed)
    pub async fn detect_or_create(
        &self,
        message: &str,
        turns: &[ConversationTurn],
    ) -> Result<SpaceId> {
        let spaces = self.spaces.read().clone();
        let spaces_vec: Vec<_> = spaces.into_values().collect();

        // ── Layer 1: Filesystem path detection ──
        if let Some(path) = detection::extract_filesystem_path(message) {
            // Check if this path matches an existing Space (read lock, brief)
            let matched_space_id = {
                let matcher = self.path_matcher.read();
                matcher.find_space(&path)
            };

            if let Some(space_id) = matched_space_id {
                self.activate(&space_id).await?;
                return Ok(space_id);
            }

            // New path detected → create new Space
            let name = detection::path_name(&path);
            let mut space = Space::from_path(&path);
            space.name = name;
            space.workspace_dir = self.default_workspace_dir(&space.id);
            space.tags.push(path.to_string_lossy().to_string());

            self.add_space(space).await?;
            let space_id = self.current_space_id();

            self.event_bus.publish(KernelEvent::SpaceCreated {
                space_id,
                name: "unnamed".to_string(),
                source: "auto_resource".to_string(),
            })?;

            return Ok(space_id);
        }

        // ── Layer 2: Keyword/tag matching ──
        if let Some(space_id) = detection::match_keywords(message, &spaces_vec) {
            self.activate(&space_id).await?;
            return Ok(space_id);
        }

        // ── Layer 3: Topic shift detection (LLM-based) ──
        let should_check = ConversationBuffer::should_check_topic_from_messages(turns, 3);
        if should_check {
            let topic = detection::classify_topic_stub(message);

            if topic.is_clear() {
                // Check if we're in the default Space
                if self.is_in_default_space() {
                    // Promote: create new named Space from default
                    let new_space = self.promote_from_default(&topic.name).await?;
                    return Ok(new_space);
                }

                // Check if topic shifted from current Space
                if self.topic_shifted(&topic) {
                    if let Some(space_id) = self.find_by_topic(&topic.name) {
                        self.activate(&space_id).await?;
                        return Ok(space_id);
                    }

                    // Create new Space for this topic
                    let space = self.create_from_topic(&topic.name).await?;
                    return Ok(space.id);
                }
            }
        }

        // Default: stay in current Space
        Ok(self.current_space_id())
    }

    /// Check if the topic has shifted significantly.
    fn topic_shifted(&self, new_topic: &Topic) -> bool {
        let current = self.current_space();
        if let Some(space) = current {
            if space.is_default() {
                return true; // Always allow shift from default
            }
            // Compare topic name with current Space name
            let current_lower = space.name.to_lowercase();
            let new_lower = new_topic.name.to_lowercase();
            !current_lower.is_empty() && current_lower != new_lower
        } else {
            true
        }
    }

    /// Find a Space by topic name.
    fn find_by_topic(&self, topic: &str) -> Option<SpaceId> {
        let spaces = self.spaces.read();
        let topic_lower = topic.to_lowercase();

        // Exact match first
        for space in spaces.values() {
            if space.name.to_lowercase() == topic_lower {
                return Some(space.id);
            }
        }

        // Then tag match
        for space in spaces.values() {
            for tag in &space.tags {
                if tag.to_lowercase() == topic_lower {
                    return Some(space.id);
                }
            }
        }

        None
    }

    /// Promote from the default Space to a new named Space.
    ///
    /// Creates a new Space for the given topic and moves the default Space
    /// back to "unnamed" state.
    async fn promote_from_default(&self, topic: &str) -> Result<SpaceId> {
        let default_id = default_space_id();

        // Reset default Space
        {
            let mut spaces = self.spaces.write();
            if let Some(default) = spaces.get_mut(&default_id) {
                default.name = String::new();
                default.deactivate();
            }
        }

        // Create new named Space
        let mut new_space = Space::from_topic(topic);
        new_space.workspace_dir = self.default_workspace_dir(&new_space.id);
        new_space.active = true;

        let new_id = new_space.id;
        self.add_space(new_space).await?;

        // Update current
        *self.current_space_id.write() = new_id;

        self.event_bus.publish(KernelEvent::SpaceActivated {
            space_id: new_id,
            name: topic.to_string(),
        })?;

        tracing::info!(topic, "Promoted default Space to named Space");
        Ok(new_id)
    }

    /// Create a new Space from a detected topic.
    pub async fn create_from_topic(&self, topic: &str) -> Result<Space> {
        let mut space = Space::from_topic(topic);
        space.workspace_dir = self.default_workspace_dir(&space.id);

        // Add some default tags
        space.add_tag("topic");
        space.add_tag(topic);

        self.add_space(space.clone()).await?;

        self.event_bus.publish(KernelEvent::SpaceCreated {
            space_id: space.id,
            name: space.name.clone(),
            source: "auto_topic".to_string(),
        })?;

        Ok(space)
    }

    /// Create a Space from a filesystem path.
    pub async fn create_from_path(&self, name: &str, path: &Path) -> Result<Space> {
        let mut space = Space::from_path(path);
        if !name.is_empty() {
            space.name = name.to_string();
        }
        space.workspace_dir = self.default_workspace_dir(&space.id);

        self.add_space(space.clone()).await?;

        self.event_bus.publish(KernelEvent::SpaceCreated {
            space_id: space.id,
            name: space.name.clone(),
            source: "auto_resource".to_string(),
        })?;

        Ok(space)
    }

    /// Activate a Space (set as current).
    pub async fn activate(&self, space_id: &SpaceId) -> Result<()> {
        {
            let mut spaces = self.spaces.write();
            for (id, space) in spaces.iter_mut() {
                if *id == *space_id {
                    space.activate();
                    space.touch();
                } else {
                    space.deactivate();
                }
            }
        }
        *self.current_space_id.write() = *space_id;

        let space = self.current_space();
        let (id, name) = if let Some(s) = space {
            (s.id, s.name.clone())
        } else {
            (*space_id, String::new())
        };

        self.save_space(&Space {
            id,
            name: name.clone(),
            source: SpaceSource::Manual,
            paths: Vec::new(),
            workspace_dir: self.default_workspace_dir(&id),
            tags: Vec::new(),
            active: true,
            created_at: Utc::now(),
            last_active_at: Utc::now(),
            interaction_count: 1,
            memory_visible: true,
        })
        .await
        .ok(); // Ignore save errors here

        self.event_bus.publish(KernelEvent::SpaceActivated {
            space_id: *space_id,
            name,
        })?;

        Ok(())
    }

    /// Get a Space by ID.
    pub async fn get_space(&self, space_id: &SpaceId) -> Result<Option<Space>> {
        Ok(self.spaces.read().get(space_id).cloned())
    }

    /// List all Spaces.
    pub fn list(&self) -> Vec<Space> {
        self.spaces.read().values().cloned().collect()
    }

    /// Get the current Space ID.
    pub fn current_space_id(&self) -> SpaceId {
        *self.current_space_id.read()
    }

    /// Get the default Space ID.
    pub fn default_space_id(&self) -> SpaceId {
        default_space_id()
    }

    /// Get the current Space.
    pub fn current_space(&self) -> Option<Space> {
        let current_id = self.current_space_id();
        self.spaces.read().get(&current_id).cloned()
    }

    /// Check if currently in the default (unnamed) Space.
    pub fn is_in_default_space(&self) -> bool {
        let current = self.current_space();
        current.map(|s| s.is_default()).unwrap_or(true)
    }

    /// Merge two Spaces.
    ///
    /// The `absorbed` Space is merged into `survivor`.
    /// Memory entries from absorbed are transferred to survivor.
    pub async fn merge_spaces(&self, survivor_id: SpaceId, absorbed_id: SpaceId) -> Result<()> {
        if survivor_id == absorbed_id {
            bail!(SpaceManagerError::SelfMerge);
        }

        // Load both spaces
        let (mut survivor, absorbed) = {
            let spaces = self.spaces.read();
            let s = spaces.get(&survivor_id).cloned();
            let a = spaces.get(&absorbed_id).cloned();
            match (s, a) {
                (Some(sv), Some(av)) => (sv, av),
                _ => bail!(SpaceManagerError::NotFound(survivor_id)),
            }
        };

        // Transfer memory (Phase 3 will do actual transfer)
        let entries_migrated = 0; // Stub until Phase 3

        // Update survivor
        survivor.last_active_at = Utc::now();
        survivor.interaction_count += absorbed.interaction_count;

        // Merge tags
        for tag in absorbed.tags {
            survivor.add_tag(tag);
        }

        // Merge paths (deduplicate)
        for path in absorbed.paths {
            if !survivor.paths.contains(&path) {
                survivor.paths.push(path);
            }
        }

        // Save survivor and remove absorbed
        self.save_space(&survivor).await?;

        {
            let mut spaces = self.spaces.write();
            spaces.remove(&absorbed_id);
        }

        // Archive absorbed space directory
        let absorbed_dir = self.root_dir.join(absorbed_id.to_string());
        let archived_dir = self
            .root_dir
            .join("_archived")
            .join(absorbed_id.to_string());
        if absorbed_dir.exists() {
            if let Some(parent) = archived_dir.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::rename(&absorbed_dir, &archived_dir);
        }

        // Reindex
        self.reindex_path_matcher();
        self.save_index().await?;

        // Publish events
        self.event_bus.publish(KernelEvent::SpacesMerged {
            survivor: survivor_id,
            absorbed: absorbed_id,
            entries_migrated,
        })?;

        tracing::info!(
            survivor = %survivor_id,
            absorbed = %absorbed_id,
            "Spaces merged"
        );

        Ok(())
    }

    /// Check if two Spaces should be auto-merged.
    pub fn should_auto_merge(&self, a: &Space, b: &Space) -> bool {
        // Must share at least one path
        if a.paths.is_empty() || b.paths.is_empty() {
            return false;
        }

        let paths_overlap = a.paths.iter().any(|ap| {
            b.paths
                .iter()
                .any(|bp| ap == bp || ap.starts_with(bp) || bp.starts_with(ap))
        });

        if !paths_overlap {
            return false;
        }

        // Tag similarity must be high
        let a_tags: std::collections::HashSet<_> =
            a.tags.iter().map(|t| t.to_lowercase()).collect();
        let b_tags: std::collections::HashSet<_> =
            b.tags.iter().map(|t| t.to_lowercase()).collect();

        if a_tags.is_empty() && b_tags.is_empty() {
            // Both have no tags — could be candidates
        }

        // Both must have low interaction count (< 5)
        let both_low_activity = a.interaction_count < 5 && b.interaction_count < 5;

        paths_overlap && both_low_activity
    }

    /// Archive Spaces that haven't been active for MAX_ARCHIVE_AGE_DAYS.
    pub async fn archive_stale(&self) -> Result<Vec<SpaceId>> {
        let cutoff = Utc::now() - chrono::Duration::days(MAX_ARCHIVE_AGE_DAYS);
        let mut archived = Vec::new();

        let stale_ids: Vec<SpaceId> = {
            let spaces = self.spaces.read();
            spaces
                .values()
                .filter(|s| s.id != default_space_id() && s.last_active_at < cutoff)
                .map(|s| s.id)
                .collect()
        };

        for id in stale_ids {
            self.archive_space(&id).await?;
            archived.push(id);
        }

        if !archived.is_empty() {
            tracing::info!(count = archived.len(), "Archived stale Spaces");
        }

        Ok(archived)
    }

    /// Archive a single Space.
    async fn archive_space(&self, space_id: &SpaceId) -> Result<()> {
        let space = {
            let spaces = self.spaces.read();
            spaces.get(space_id).cloned()
        };

        let space = match space {
            Some(s) => s,
            None => return Ok(()),
        };

        // Move directory
        let src = self.root_dir.join(space_id.to_string());
        let dst = self.root_dir.join("_archived").join(space_id.to_string());
        if src.exists() {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&src, &dst)?;
        }

        // Remove from index
        {
            let mut spaces = self.spaces.write();
            spaces.remove(space_id);
        }

        self.save_index().await?;
        self.reindex_path_matcher();

        self.event_bus.publish(KernelEvent::SpaceArchived {
            space_id: *space_id,
            name: space.name,
        })?;

        Ok(())
    }

    /// Restore an archived Space.
    pub async fn restore_from_archive(&self, space_id: &SpaceId) -> Result<()> {
        let archived_dir = self.root_dir.join("_archived").join(space_id.to_string());

        if !archived_dir.exists() {
            bail!("Archived Space not found: {}", space_id);
        }

        // Load space data
        let space_file = archived_dir.join("space.json");
        let space: Space = if space_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&space_file)?)?
        } else {
            bail!("Space data not found for {}", space_id);
        };

        // Restore directory
        let dst = self.root_dir.join(space_id.to_string());
        std::fs::create_dir_all(&dst)?;
        for entry in std::fs::read_dir(&archived_dir)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let src_file = archived_dir.join(&file_name);
            let dst_file = dst.join(&file_name);
            if src_file.is_file() {
                std::fs::copy(&src_file, &dst_file)?;
            }
        }

        // Add back to index
        self.add_space(space).await?;

        // Remove from archived
        let _ = std::fs::remove_dir_all(&archived_dir);

        tracing::info!(space_id = %space_id, "Restored Space from archive");
        Ok(())
    }

    /// Get the memory bridge.
    pub fn memory_bridge(&self) -> Option<Arc<SpaceBridge>> {
        self.memory_bridge.clone()
    }

    /// Get the root directory.
    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }

    /// Get the conversation buffer.
    pub fn buffer(&self) -> Arc<Mutex<ConversationBuffer>> {
        self.buffer.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::SpaceSource;

    fn test_state_store() -> Arc<StateStore> {
        let dir = tempfile::tempdir().unwrap();
        Arc::new(StateStore::new(dir.path().to_path_buf()).unwrap())
    }

    fn test_event_bus() -> EventBus {
        EventBus::new(64)
    }

    #[tokio::test]
    async fn test_ensure_default_space() {
        let store = test_state_store();
        let bus = test_event_bus();
        let manager = SpaceManager::new(store, bus).await.unwrap();

        let default = manager.get_space(&default_space_id()).await.unwrap();
        assert!(default.is_some());
        assert!(default.unwrap().is_default());
    }

    #[tokio::test]
    async fn test_create_from_path() {
        let store = test_state_store();
        let bus = test_event_bus();
        let manager = SpaceManager::new(store, bus).await.unwrap();

        let path = PathBuf::from("/projects/oxios");
        let space = manager.create_from_path("oxios", &path).await.unwrap();

        assert_eq!(space.name, "oxios");
        assert_eq!(space.paths, vec![path]);
        assert_eq!(space.source, SpaceSource::AutoResource);
    }

    #[tokio::test]
    async fn test_activate() {
        let store = test_state_store();
        let bus = test_event_bus();
        let manager = SpaceManager::new(store, bus).await.unwrap();

        let path = PathBuf::from("/projects/oxios");
        let space = manager.create_from_path("oxios", &path).await.unwrap();

        assert_eq!(manager.current_space_id(), default_space_id());

        manager.activate(&space.id).await.unwrap();
        assert_eq!(manager.current_space_id(), space.id);
    }

    #[tokio::test]
    async fn test_is_in_default_space() {
        let store = test_state_store();
        let bus = test_event_bus();
        let manager = SpaceManager::new(store, bus).await.unwrap();

        assert!(manager.is_in_default_space());

        let path = PathBuf::from("/projects/oxios");
        let space = manager.create_from_path("oxios", &path).await.unwrap();
        manager.activate(&space.id).await.unwrap();

        assert!(!manager.is_in_default_space());
    }

    #[tokio::test]
    async fn test_list() {
        let store = test_state_store();
        let bus = test_event_bus();
        let manager = SpaceManager::new(store, bus).await.unwrap();

        assert_eq!(manager.list().len(), 1); // default only

        let path = PathBuf::from("/projects/oxios");
        manager.create_from_path("oxios", &path).await.unwrap();

        assert_eq!(manager.list().len(), 2);
    }

    #[tokio::test]
    async fn test_merge_spaces_self_error() {
        let store = test_state_store();
        let bus = test_event_bus();
        let manager = SpaceManager::new(store, bus).await.unwrap();

        let result = manager
            .merge_spaces(default_space_id(), default_space_id())
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().downcast_ref(),
            Some(SpaceManagerError::SelfMerge)
        ));
    }

    #[tokio::test]
    async fn test_should_auto_merge() {
        let store = test_state_store();
        let bus = test_event_bus();
        let manager = SpaceManager::new(store, bus).await.unwrap();

        let path = PathBuf::from("/projects/oxios");

        let mut space1 = Space::from_path(&path);
        space1.name = "oxios-dev".to_string();
        space1.interaction_count = 2;

        let mut space2 = Space::from_path(&path);
        space2.name = "oxios-bugfix".to_string();
        space2.interaction_count = 3;

        // Same path + low activity → should suggest merge
        assert!(manager.should_auto_merge(&space1, &space2));

        // High activity on one → should not auto-merge
        space1.interaction_count = 10;
        assert!(!manager.should_auto_merge(&space1, &space2));
    }
}
