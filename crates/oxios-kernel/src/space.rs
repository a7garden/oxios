//! Space: logical work partition for context isolation.
//!
//! A Space provides isolated memory and workspace for different
//! contexts (projects, topics, domains). The OS automatically
//! routes user messages to the appropriate Space based on
//! filesystem paths, keywords, or LLM-based topic detection.

pub mod conversation_buffer;
pub mod detection;
pub mod knowledge_bridge;
pub mod manager;

pub use conversation_buffer::{ConversationBuffer, ConversationTurn};
pub use detection::{extract_filesystem_path, match_keywords, PathMatcher};
pub use knowledge_bridge::{CrossRefEntry, KnowledgeBridge, KnowledgeFlow};
pub use manager::{SpaceManager, SpaceManagerError};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Unique identifier for a Space.
pub type SpaceId = Uuid;

/// How a Space was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceSource {
    /// Auto-created from a detected filesystem path.
    AutoResource,
    /// Auto-created from a detected topic shift.
    AutoTopic,
    /// Explicitly created by the user.
    Manual,
}

impl std::fmt::Display for SpaceSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpaceSource::AutoResource => write!(f, "auto_resource"),
            SpaceSource::AutoTopic => write!(f, "auto_topic"),
            SpaceSource::Manual => write!(f, "manual"),
        }
    }
}

/// A logical work partition.
///
/// Each Space has its own scoped memory and workspace.
/// The OS automatically routes messages to the appropriate Space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    /// Unique identifier.
    pub id: SpaceId,
    /// Human-readable name.
    /// - AutoResource: derived from directory name (e.g. "oxios")
    /// - AutoTopic: estimated from LLM classification (e.g. "일상")
    /// - Default Space: empty string (named after topic forms)
    pub name: String,
    /// How this Space was created.
    pub source: SpaceSource,
    /// Actual filesystem paths bound to this Space.
    /// AgentRuntime sets CWD to paths[0] when executing.
    /// Empty for non-filesystem Spaces (일상, 요리 등).
    pub paths: Vec<PathBuf>,
    /// Scratch workspace directory for this Space.
    /// Temporary files, logs, build artifacts go here.
    pub workspace_dir: PathBuf,
    /// Tags for keyword matching (Layer 2 detection).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether this Space is currently active.
    #[serde(default)]
    pub active: bool,
    /// When this Space was created.
    pub created_at: DateTime<Utc>,
    /// When this Space was last active.
    pub last_active_at: DateTime<Utc>,
    /// Number of interactions in this Space.
    #[serde(default)]
    pub interaction_count: u64,
    /// Whether this Space allows cross-Space knowledge access.
    /// Default: true. Set to false for private Spaces.
    #[serde(default = "default_true")]
    pub knowledge_visible: bool,
}

fn default_true() -> bool {
    true
}

impl Space {
    /// Create a new Space with the given name and source.
    pub fn new(name: impl Into<String>, source: SpaceSource) -> Self {
        let now = Utc::now();
        Self {
            id: SpaceId::new_v4(),
            name: name.into(),
            source,
            paths: Vec::new(),
            workspace_dir: PathBuf::new(),
            tags: Vec::new(),
            active: false,
            created_at: now,
            last_active_at: now,
            interaction_count: 0,
            knowledge_visible: true,
        }
    }

    /// Create a Space from a detected filesystem path.
    pub fn from_path(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let mut space = Self::new(&name, SpaceSource::AutoResource);
        space.paths.push(path.to_path_buf());
        space
    }

    /// Create a Space from a detected topic.
    pub fn from_topic(topic: &str) -> Self {
        Self::new(topic, SpaceSource::AutoTopic)
    }

    /// Record that this Space was interacted with.
    pub fn touch(&mut self) {
        self.last_active_at = Utc::now();
        self.interaction_count += 1;
    }

    /// Mark this Space as active.
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Mark this Space as inactive.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Whether this Space has a name (non-empty).
    pub fn is_named(&self) -> bool {
        !self.name.is_empty()
    }

    /// Whether this is the default (unnamed) Space.
    pub fn is_default(&self) -> bool {
        self.name.is_empty()
    }

    /// Get the emoji indicator for this Space.
    pub fn emoji(&self) -> &'static str {
        if self.name.is_empty() {
            "⚪"
        } else {
            // Map common names to emojis
            match self.name.to_lowercase().as_str() {
                "oxios" | "dev" | "개발" => "🔧",
                "일상" | "daily" | "生活" => "🏠",
                "blog" | "블로그" => "📝",
                "docs" | "문서" => "📄",
                "study" | "공부" | "학습" => "📚",
                "cook" | "요리" | "recipe" | "레시피" => "🍳",
                "work" | "업무" => "💼",
                _ => "📦",
            }
        }
    }

    /// Add a tag for keyword matching.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }
}

#[allow(missing_docs)]
pub static DEFAULT_SPACE_ID: std::sync::OnceLock<uuid::Uuid> = std::sync::OnceLock::new();

/// Get the default Space ID.
pub fn default_space_id() -> SpaceId {
    *DEFAULT_SPACE_ID
        .get_or_init(|| uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_space_new() {
        let s = Space::new("oxios", SpaceSource::AutoResource);
        assert_eq!(s.name, "oxios");
        assert_eq!(s.source, SpaceSource::AutoResource);
        assert!(!s.active);
        assert_eq!(s.interaction_count, 0);
    }

    #[test]
    fn test_space_from_path() {
        let path = PathBuf::from("/projects/oxios");
        let s = Space::from_path(&path);
        assert_eq!(s.name, "oxios");
        assert_eq!(s.source, SpaceSource::AutoResource);
        assert_eq!(s.paths, vec![path]);
    }

    #[test]
    fn test_space_touch() {
        let mut s = Space::new("test", SpaceSource::Manual);
        assert_eq!(s.interaction_count, 0);
        s.touch();
        assert_eq!(s.interaction_count, 1);
    }

    #[test]
    fn test_space_emoji() {
        let s = Space::new("", SpaceSource::Manual);
        assert_eq!(s.emoji(), "⚪");

        let s = Space::new("oxios", SpaceSource::AutoResource);
        assert_eq!(s.emoji(), "🔧");

        let s = Space::new("일상", SpaceSource::AutoTopic);
        assert_eq!(s.emoji(), "🏠");

        let s = Space::new("random", SpaceSource::Manual);
        assert_eq!(s.emoji(), "📦");
    }

    #[test]
    fn test_space_default() {
        let s = Space::new("", SpaceSource::Manual);
        assert!(s.is_default());
        assert!(!s.is_named());

        let s = Space::new("oxios", SpaceSource::AutoResource);
        assert!(!s.is_default());
        assert!(s.is_named());
    }
}
