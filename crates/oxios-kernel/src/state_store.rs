//! Filesystem-based state store.
//!
//! All state is persisted as markdown or JSON files organized
//! by category. This is the "filesystem" of Oxios.

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
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
    /// Index range into `Session::trajectory_steps` for tool calls that
    /// occurred during this response. `None` when no tool calls were made.
    /// Used by the Web UI to render per-turn execution timelines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trajectory_range: Option<TrajectoryRange>,
}

/// Index range (exclusive end) into `Session::trajectory_steps`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryRange {
    /// Start index (inclusive).
    pub start: usize,
    /// End index (exclusive).
    pub end: usize,
}

/// A single tool execution step recorded in a session (RFC-015).
///
/// Persisted alongside the agent response so that the Web UI can render the
/// execution timeline (tool calls, durations, errors) when the user
/// re-opens the session later. Mirrors `memory::sona::TrajectoryStep` but
/// is duplicated here to avoid a kernel-state → memory dependency cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStepRecord {
    /// Name of the tool that was called.
    pub tool_name: String,
    /// Tool input arguments (JSON).
    pub tool_args: serde_json::Value,
    /// Truncated output (max ~500 chars).
    pub output_summary: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Whether the tool returned an error.
    pub is_error: bool,
    /// Provider-specific tool call ID (for start/end correlation).
    pub tool_call_id: String,
    /// Timestamp when the step started.
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
    /// RFC-015: tool execution trajectory accumulated for this session.
    /// Appended on each orchestrator run; consumed by the Web UI to render
    /// the execution timeline when the session is re-opened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trajectory_steps: Vec<TrajectoryStepRecord>,
    /// Currently active seed ID (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_seed_id: Option<String>,
    /// Currently active persona ID (for future multi-persona support).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_persona_id: Option<String>,
    /// RFC-025: Project this session belongs to (singular, grouping only).
    /// Set by the sidebar/drag-to-reparent; consumed for Project-tree view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
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
            trajectory_steps: Vec::new(),
            active_seed_id: None,
            active_persona_id: None,
            project_id: None,
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
            trajectory_steps: Vec::new(),
            active_seed_id: None,
            active_persona_id: None,
            project_id: None,
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

    /// Appends trajectory steps to the session (RFC-015).
    ///
    /// Called by the orchestrator after each run so the Web UI can
    /// re-render the execution timeline when the user re-opens the session.
    pub fn extend_trajectory(&mut self, steps: Vec<TrajectoryStepRecord>) {
        if steps.is_empty() {
            return;
        }
        self.trajectory_steps.extend(steps);
        self.updated_at = Utc::now();
    }

    /// Returns the trajectory steps recorded in this session.
    pub fn trajectory(&self) -> &[TrajectoryStepRecord] {
        &self.trajectory_steps
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
    /// ```no_run
    /// use oxios_kernel::state_store::StateStore;
    /// use std::path::PathBuf;
    ///
    /// let store = StateStore::new(PathBuf::from("/tmp/oxios-state")).unwrap();
    /// ```
    pub fn new(base_path: PathBuf) -> Result<Self> {
        Ok(Self { base_path })
    }

    /// Validate that a category name does not contain path traversal.
    fn validate_category(category: &str) -> Result<()> {
        if category.contains("..") || category.contains('\\') {
            bail!("invalid category name: '{category}'");
        }
        if category.is_empty()
            || category.starts_with('/')
            || category.ends_with('/')
            || category.contains("//")
        {
            bail!("invalid category name: '{category}'");
        }
        Ok(())
    }

    /// Validate that a file name does not contain path traversal.
    fn validate_name(name: &str) -> Result<()> {
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            bail!("invalid file name: '{name}'");
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

        // Write to temp file first, then atomic rename
        let temp_path = dir.join(format!(
            "{name}.{}.{}.tmp",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::write(&temp_path, content).await?;
        tokio::fs::rename(&temp_path, &path).await?;

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
            if let Some(ext) = path.extension()
                && (ext == "md" || ext == "json")
                && let Some(stem) = path.file_stem()
            {
                names.push(stem.to_string_lossy().into_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Save a serializable value as JSON under the given category.
    pub async fn save_json<T: Serialize>(
        &self,
        category: &str,
        name: &str,
        data: &T,
    ) -> Result<()> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let dir = self.base_path.join(category);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{name}.json"));

        let content = serde_json::to_string_pretty(data)?;

        // Write to temp file first, then atomic rename
        let temp_path = dir.join(format!(
            "{name}.{}.{}.tmp",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::write(&temp_path, &content).await?;
        tokio::fs::rename(&temp_path, &path).await?;

        Ok(())
    }

    /// Load a deserializable value from JSON in the given category.
    pub async fn load_json<T: DeserializeOwned>(
        &self,
        category: &str,
        name: &str,
    ) -> Result<Option<T>> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let path = self.base_path.join(category).join(format!("{name}.json"));
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(Some(serde_json::from_str(&content)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete a file from the given category.
    pub async fn delete_file(&self, category: &str, name: &str) -> Result<bool> {
        Self::validate_category(category)?;
        Self::validate_name(name)?;
        let path = self.base_path.join(category).join(format!("{name}.json"));
        if path.exists() {
            tokio::fs::remove_file(path).await?;
            Ok(true)
        } else {
            let path = self.base_path.join(category).join(format!("{name}.md"));
            if path.exists() {
                tokio::fs::remove_file(path).await?;
                Ok(true)
            } else {
                Ok(false)
            }
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

    /// Saves a session and then runs pruning if auto_prune is enabled.
    pub async fn save_session_with_prune(
        &self,
        session: &Session,
        prune_config: &PruneConfig,
    ) -> Result<()> {
        self.save_session(session).await?;
        // Prune in the background — don't block the response
        let store = self.clone();
        let config = prune_config.clone();
        tokio::spawn(async move {
            if let Err(e) = store.prune_sessions(&config).await {
                tracing::warn!(error = %e, "Background session pruning failed");
            }
        });
        Ok(())
    }

    /// Loads a session by ID.
    pub async fn load_session(&self, session_id: &SessionId) -> Result<Option<Session>> {
        self.load_json("sessions", &session_id.0).await
    }

    /// RFC-025 Phase 5: Load all sessions in full (messages + trajectories).
    ///
    /// Used by the Mount auto-promotion scanner, which needs trajectory
    /// tool_args to identify paths the agent actually worked on. Cheaper to
    /// call once per scan than `load_session` per id.
    pub async fn load_all_sessions(&self) -> Result<Vec<Session>> {
        let mut sessions = Vec::new();
        if let Ok(names) = self.list_category("sessions").await {
            for name in names {
                if let Ok(Some(session)) = self.load_json::<Session>("sessions", &name).await {
                    sessions.push(session);
                }
            }
        }
        Ok(sessions)
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
                        title: session
                            .metadata
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .or_else(|| {
                                // Auto-generate from first user message
                                session.user_messages.first().map(|m| {
                                    let s = m.content.lines().next().unwrap_or("");
                                    if s.len() > 60 {
                                        format!("{}…", &s[..s.ceil_char_boundary(59)])
                                    } else {
                                        s.to_string()
                                    }
                                })
                            }),
                        active_seed_id: session.active_seed_id.clone(),
                        project_id: session
                            .project_id
                            .clone()
                            // Backward-compat: fall back to legacy metadata keys.
                            .or_else(|| {
                                session
                                    .metadata
                                    .get("project_id")
                                    .and_then(|v| v.as_str())
                                    .map(String::from)
                            })
                            .or_else(|| {
                                session
                                    .metadata
                                    .get("project_ids")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| s.split(',').next().map(String::from))
                            }),
                        created_at: session.created_at,
                        updated_at: session.updated_at,
                    });
                }
            }
        }

        // Sort by updated_at descending (most recent first)
        sessions.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        Ok(sessions)
    }

    /// Deletes a session by ID.
    pub async fn delete_session(&self, session_id: &SessionId) -> Result<bool> {
        let path = self
            .base_path
            .join("sessions")
            .join(format!("{}.json", session_id.0));
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
        if let Some(sid) = session_id
            && let Some(existing) = self.load_session(sid).await?
        {
            return Ok(existing);
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

    /// RFC-025: Move a session to a different Project (drag-to-reparent).
    ///
    /// Pass `None` to unassign (move to "unfiled").
    pub async fn move_session_to_project(
        &self,
        session_id: &SessionId,
        project_id: Option<&str>,
    ) -> Result<bool> {
        match self.load_session(session_id).await? {
            Some(mut session) => {
                session.project_id = project_id.map(String::from);
                session.updated_at = Utc::now();
                self.save_session(&session).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Prune sessions based on configuration.
    ///
    /// Removes sessions that exceed TTL or exceed the maximum count.
    /// Returns the number of sessions pruned.
    pub async fn prune_sessions(&self, config: &PruneConfig) -> Result<usize> {
        let mut sessions = self.list_sessions().await?;
        let mut pruned = 0;

        // TTL-based pruning: remove sessions older than ttl_hours
        if config.ttl_hours > 0 {
            let cutoff = Utc::now() - chrono::Duration::hours(config.ttl_hours as i64);
            let to_prune_ttl: Vec<String> = sessions
                .iter()
                .filter(|s| s.updated_at < cutoff)
                .map(|s| s.id.clone())
                .collect();

            for id in &to_prune_ttl {
                let sid = SessionId(id.clone());
                if self.delete_session(&sid).await.is_ok() {
                    pruned += 1;
                }
            }

            // Remove pruned sessions from the list for count-based pruning
            sessions.retain(|s| !to_prune_ttl.contains(&s.id));
        }

        // Count-based pruning: keep only the most recent `max_sessions`
        if config.max_sessions > 0 && sessions.len() > config.max_sessions {
            // sessions are already sorted by updated_at descending
            let excess = sessions.len() - config.max_sessions;
            for session in sessions.into_iter().rev().take(excess) {
                let sid = SessionId(session.id);
                if self.delete_session(&sid).await.is_ok() {
                    pruned += 1;
                }
            }
        }

        if pruned > 0 {
            tracing::info!(pruned = pruned, "Session pruning completed");
        }

        Ok(pruned)
    }

    /// Prune agent records based on config.
    ///
    /// 1. TTL-based: delete agents with created_at older than ttl_hours.
    /// 2. Count-based: if still over max_entries, delete oldest.
    pub async fn prune_agents_by_config(
        &self,
        max_entries: usize,
        ttl_hours: u64,
        batch_size: usize,
    ) -> Result<usize> {
        let mut pruned = 0usize;

        let names = self.list_category("agents").await?;
        if names.is_empty() {
            return Ok(0);
        }

        let now = Utc::now();

        // 1. TTL-based pruning
        let mut remaining: Vec<(String, DateTime<Utc>)> = Vec::with_capacity(names.len());

        if ttl_hours > 0 {
            let cutoff = now - chrono::Duration::hours(ttl_hours as i64);
            for name in &names {
                // Load just enough to check created_at
                if let Ok(Some(info)) = self
                    .load_json::<crate::types::AgentInfo>("agents", name)
                    .await
                {
                    if info.created_at < cutoff {
                        if self.delete_file("agents", name).await.unwrap_or(false) {
                            pruned += 1;
                        }
                    } else {
                        remaining.push((name.clone(), info.created_at));
                    }
                }
            }
        } else {
            // Load all created_at for count-based pruning
            for name in &names {
                if let Ok(Some(info)) = self
                    .load_json::<crate::types::AgentInfo>("agents", name)
                    .await
                {
                    remaining.push((name.clone(), info.created_at));
                }
            }
        }

        // 2. Count-based pruning
        if max_entries > 0 && remaining.len() > max_entries {
            // Sort by created_at ascending (oldest first)
            remaining.sort_by_key(|a| a.1);

            let excess = remaining.len() - max_entries;
            let to_delete = excess.min(batch_size);

            for (name, _) in remaining.iter().take(to_delete) {
                if self.delete_file("agents", name).await.unwrap_or(false) {
                    pruned += 1;
                }
            }
        }

        if pruned > 0 {
            tracing::info!(pruned = pruned, "Agent filesystem pruning completed");
        }

        Ok(pruned)
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
    /// Auto-generated title for this session. Derived from the first user
    /// message (truncated to ~60 chars) when not explicitly set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Active seed ID if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_seed_id: Option<String>,
    /// Active project ID(s) this session belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Configuration for session pruning.
#[derive(Debug, Clone)]
pub struct PruneConfig {
    /// Maximum number of sessions to keep. 0 = unlimited.
    pub max_sessions: usize,
    /// TTL in hours. Sessions older than this are pruned. 0 = no TTL.
    pub ttl_hours: u64,
}

impl Default for PruneConfig {
    fn default() -> Self {
        Self {
            max_sessions: 100,
            ttl_hours: 168, // 7 days
        }
    }
}

/// Tracks the last time a prune was performed, enabling cooldown.
pub struct PruneThrottle {
    /// Instant of the last prune (monotonic).
    last_prune: std::sync::Mutex<Option<std::time::Instant>>,
    /// Minimum seconds between prune runs.
    cooldown_secs: u64,
}

impl PruneThrottle {
    /// Create a new throttle with the given cooldown.
    pub fn new(cooldown_secs: u64) -> Self {
        Self {
            last_prune: std::sync::Mutex::new(None),
            cooldown_secs,
        }
    }

    /// Check if enough time has elapsed since the last prune.
    /// Returns `true` if prune should proceed.
    pub fn should_prune(&self) -> bool {
        // SAFETY: parking_lot::Mutex never poisons, but std::sync::Mutex does.
        // Recover from poison by taking the inner value so pruning continues.
        let mut guard = self.last_prune.lock().unwrap_or_else(|e| {
            tracing::warn!("PruneThrottle mutex poisoned, recovering: {e}");
            e.into_inner()
        });
        let now = std::time::Instant::now();
        match *guard {
            Some(last) => {
                if now.duration_since(last).as_secs() >= self.cooldown_secs {
                    *guard = Some(now);
                    true
                } else {
                    false
                }
            }
            None => {
                *guard = Some(now);
                true
            }
        }
    }
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

        let session = Session::new("user-123");
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
        let session = store.get_or_create_session("user-456", None).await.unwrap();
        assert_eq!(session.user_id, "user-456");
        assert!(session.user_messages.is_empty());
    }

    #[tokio::test]
    async fn test_prune_sessions_by_count() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        // Create 5 sessions
        for i in 0..5 {
            let mut session = Session::new(&format!("user-{}", i));
            session.add_user_message(&format!("Message {}", i));
            store.save_session(&session).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // Prune to max 3
        let config = PruneConfig {
            max_sessions: 3,
            ttl_hours: 0,
        };
        let pruned = store.prune_sessions(&config).await.unwrap();
        assert_eq!(pruned, 2);

        let remaining = store.list_sessions().await.unwrap();
        assert_eq!(remaining.len(), 3);
        // Oldest sessions (user-0, user-1) should be pruned
        let remaining_ids: Vec<&str> = remaining.iter().map(|s| s.user_id.as_str()).collect();
        assert!(remaining_ids.contains(&"user-2"));
        assert!(remaining_ids.contains(&"user-3"));
        assert!(remaining_ids.contains(&"user-4"));
    }

    #[tokio::test]
    async fn test_prune_sessions_by_ttl() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        // Create a session and manually set updated_at to the past
        let mut old_session = Session::new("old-user");
        old_session.updated_at = Utc::now() - chrono::Duration::hours(48);
        store.save_session(&old_session).await.unwrap();

        // Create a recent session
        let mut recent_session = Session::new("recent-user");
        recent_session.add_user_message("Hello");
        store.save_session(&recent_session).await.unwrap();

        // Prune with 24-hour TTL
        let config = PruneConfig {
            max_sessions: 0,
            ttl_hours: 24,
        };
        let pruned = store.prune_sessions(&config).await.unwrap();
        assert_eq!(pruned, 1);

        let remaining = store.list_sessions().await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].user_id, "recent-user");
    }
}
