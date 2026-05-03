//! Filesystem-based state store.
//!
//! All state is persisted as markdown or JSON files organized
//! by category. This is the "filesystem" of Oxios.

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize, Deserializer, Serializer};
use std::path::PathBuf;
use tokio::fs;

/// Unique identifier for a session.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    /// Creates a new random session ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for SessionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

/// A user message in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    /// Message content.
    pub content: String,
    /// Timestamp when the message was sent.
    pub timestamp: DateTime<Utc>,
}

/// An agent response in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// Response content.
    pub content: String,
    /// Session ID associated with this response.
    pub session_id: Option<String>,
    /// Seed ID used for this response (if any).
    pub seed_id: Option<String>,
    /// Phase reached during orchestration.
    pub phase_reached: Option<String>,
    /// Whether evaluation passed.
    pub evaluation_passed: Option<bool>,
    /// Timestamp when the response was generated.
    pub timestamp: DateTime<Utc>,
}

/// Arbitrary key-value metadata for a session.
pub type SessionMetadata = std::collections::HashMap<String, serde_json::Value>;

/// A session represents a single user conversation.
///
/// Sessions track the full message history and metadata for
/// a user conversation. They are created per user interaction
/// and persisted for later retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: SessionId,
    /// User ID who owns this session.
    pub user_id: String,
    /// All user messages in this session.
    #[serde(default)]
    pub user_messages: Vec<UserMessage>,
    /// All agent responses in this session.
    #[serde(default)]
    pub agent_responses: Vec<AgentResponse>,
    /// Currently active seed ID (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_seed_id: Option<String>,
    /// Currently active persona ID (for future multi-persona support).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_persona_id: Option<String>,
    /// Timestamp when the session was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the session was last updated.
    pub updated_at: DateTime<Utc>,
    /// Arbitrary key-value metadata.
    #[serde(default)]
    pub metadata: SessionMetadata,
}

impl Session {
    /// Creates a new session for a user.
    pub fn new(user_id: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            user_id: user_id.into(),
            user_messages: Vec::new(),
            agent_responses: Vec::new(),
            active_seed_id: None,
            active_persona_id: None,
            created_at: now,
            updated_at: now,
            metadata: SessionMetadata::new(),
        }
    }

    /// Creates a session with a specific ID.
    pub fn with_id(user_id: impl Into<String>, session_id: SessionId) -> Self {
        let now = Utc::now();
        Self {
            id: session_id,
            user_id: user_id.into(),
            user_messages: Vec::new(),
            agent_responses: Vec::new(),
            active_seed_id: None,
            active_persona_id: None,
            created_at: now,
            updated_at: now,
            metadata: SessionMetadata::new(),
        }
    }

    /// Adds a user message to the session.
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.user_messages.push(UserMessage {
            content: content.into(),
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Adds an agent response to the session.
    pub fn add_agent_response(&mut self, response: AgentResponse) {
        self.agent_responses.push(response);
        self.updated_at = Utc::now();
    }

    /// Sets the active seed ID.
    pub fn set_active_seed(&mut self, seed_id: Option<String>) {
        self.active_seed_id = seed_id;
        self.updated_at = Utc::now();
    }

    /// Sets the active persona ID.
    pub fn set_active_persona(&mut self, persona_id: Option<String>) {
        self.active_persona_id = persona_id;
        self.updated_at = Utc::now();
    }

    /// Sets a metadata value.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata.insert(key.into(), value);
        self.updated_at = Utc::now();
    }

    /// Gets a metadata value.
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Returns the total number of exchanges in this session.
    pub fn exchange_count(&self) -> usize {
        self.user_messages.len().min(self.agent_responses.len())
    }

    /// Returns true if the session is empty (no messages).
    pub fn is_empty(&self) -> bool {
        self.user_messages.is_empty()
    }
}
/// A filesystem-based persistent state store.
///
/// Files are organized as `<base_path>/<category>/<name>.md` or
/// `<base_path>/<category>/<name>.json`.
#[derive(Clone)]
pub struct StateStore {
    /// Root directory for all state files.
    pub base_path: PathBuf,
}

impl StateStore {
    /// Creates a new state store, initializing the directory if needed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use oxios_kernel::StateStore;
    /// use std::path::PathBuf;
    ///
    /// let store = StateStore::new(PathBuf::from("/tmp/oxios-state")).unwrap();
    /// ```
    pub fn new(base_path: PathBuf) -> Result<Self> {
        Ok(Self { base_path })
    }

    /// Validate that a category name does not contain path traversal.
    fn validate_category(category: &str) -> Result<()> {
        if category.contains("..") || category.contains('/') || category.contains('\\') {
            bail!("invalid category name: '{}'", category);
        }
        Ok(())
    }

    /// Validate that a file name does not contain path traversal.
    fn validate_name(name: &str) -> Result<()> {
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            bail!("invalid file name: '{}'", name);
        }
        Ok(())
    }

    /// Save a markdown file under the given category.
    pub async fn save_markdown(&self, category: &str, name: &str, content: &str) -> Result<()> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let dir = self.base_path.join(category);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{name}.md"));
        fs::write(path, content).await?;
        Ok(())
    }

    /// Load a markdown file from the given category.
    pub async fn load_markdown(&self, category: &str, name: &str) -> Result<Option<String>> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let path = self.base_path.join(category).join(format!("{name}.md"));
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all markdown files in a category (names without extension).
    pub async fn list_category(&self, category: &str) -> Result<Vec<String>> {
        Self::validate_category(category)?;
        let dir = self.base_path.join(category);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&dir).await?;
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "md" || ext == "json" {
                    if let Some(stem) = path.file_stem() {
                        names.push(stem.to_string_lossy().into_owned());
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Save a serializable value as JSON under the given category.
    pub async fn save_json<T: Serialize>(&self, category: &str, name: &str, data: &T) -> Result<()> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let dir = self.base_path.join(category);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{name}.json"));
        let content = serde_json::to_string_pretty(data)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Load a deserializable value from JSON in the given category.
    pub async fn load_json<T: DeserializeOwned>(&self, category: &str, name: &str) -> Result<Option<T>> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let path = self.base_path.join(category).join(format!("{name}.json"));
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(Some(serde_json::from_str(&content)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

impl std::fmt::Debug for StateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateStore")
            .field("base_path", &self.base_path)
            .finish()
    }
}

impl StateStore {
    /// Saves a session to the sessions category.
    pub async fn save_session(&self, session: &Session) -> Result<()> {
        self.save_json("sessions", &session.id.0, session).await
    }

    /// Loads a session by ID.
    pub async fn load_session(&self, session_id: &SessionId) -> Result<Option<Session>> {
        self.load_json("sessions", &session_id.0).await
    }

    /// Lists all sessions (sorted by updated_at descending).
    pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let mut sessions = Vec::new();

        if let Ok(names) = self.list_category("sessions").await {
            for name in names {
                if let Ok(Some(session)) = self.load_json::<Session>("sessions", &name).await {
                    sessions.push(SessionSummary {
                        id: session.id.0.clone(),
                        user_id: session.user_id.clone(),
                        message_count: session.user_messages.len(),
                        active_seed_id: session.active_seed_id.clone(),
                        created_at: session.created_at,
                        updated_at: session.updated_at,
                    });
                }
            }
        }

        // Sort by updated_at descending (most recent first)
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// Deletes a session by ID.
    pub async fn delete_session(&self, session_id: &SessionId) -> Result<bool> {
        let path = self.base_path.join("sessions").join(format!("{}.json", session_id.0));
        match fs::remove_file(&path).await {
            Ok(()) => {
                tracing::info!(session_id = %session_id, "Session deleted");
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Gets or creates a session for a user, initializing with the given session ID.
    pub async fn get_or_create_session(
        &self,
        user_id: &str,
        session_id: Option<&SessionId>,
    ) -> Result<Session> {
        if let Some(sid) = session_id {
            if let Some(existing) = self.load_session(sid).await? {
                return Ok(existing);
            }
        }

        // Create new session
        let session = match session_id {
            Some(sid) => Session::with_id(user_id, sid.clone()),
            None => Session::new(user_id),
        };

        self.save_session(&session).await?;
        Ok(session)
    }

    /// Updates an existing session, saving it to disk.
    pub async fn update_session(&self, session: &Session) -> Result<()> {
        self.save_session(session).await
    }
}

/// Summary of a session for listing (without full message history).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Session ID.
    pub id: String,
    /// User ID who owns this session.
    pub user_id: String,
    /// Number of messages in this session.
    pub message_count: usize,
    /// Active seed ID if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_seed_id: Option<String>,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last updated.
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation_and_persistence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        // Create a session
        let mut session = Session::new("user-123");
        session.add_user_message("Hello");

        // Save and load
        store.save_session(&session).await.unwrap();
        let loaded = store.load_session(&session.id).await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.user_id, "user-123");
        assert_eq!(loaded.user_messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_list_sorts_by_updated() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        // Create multiple sessions
        for i in 0..3 {
            let mut session = Session::new(&format!("user-{}", i));
            session.add_user_message(&format!("Message {}", i));
            store.save_session(&session).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 3);
        // Most recently updated should be first
        assert_eq!(sessions[0].user_id, "user-2");
    }

    #[tokio::test]
    async fn test_delete_session() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        let mut session = Session::new("user-123");
        store.save_session(&session).await.unwrap();

        // Delete and verify
        let deleted = store.delete_session(&session.id).await.unwrap();
        assert!(deleted);

        let loaded = store.load_session(&session.id).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_get_or_create_session_existing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        let mut existing = Session::new("user-123");
        existing.add_user_message("Original message");
        store.save_session(&existing).await.unwrap();

        // Get or create with same ID should return existing
        let retrieved = store
            .get_or_create_session("user-123", Some(&existing.id))
            .await
            .unwrap();
        assert_eq!(retrieved.id, existing.id);
        assert_eq!(retrieved.user_messages.len(), 1);
    }

    #[tokio::test]
    async fn test_get_or_create_session_new() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        // Get or create without existing session should create new
        let session = store
            .get_or_create_session("user-456", None)
            .await
            .unwrap();
        assert_eq!(session.user_id, "user-456");
        assert!(session.user_messages.is_empty());
    }
}
