//! CLI session tracking.
//!
//! A session represents a single interactive conversation started
//! when the user launches the CLI.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An interactive CLI session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: Uuid,
    /// Optional human-readable label.
    pub label: Option<String>,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last active.
    pub last_active: DateTime<Utc>,
    /// Number of messages exchanged in this session.
    pub message_count: u64,
}

impl Session {
    /// Create a new session with the current timestamp.
    pub fn new(label: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            label,
            created_at: now,
            last_active: now,
            message_count: 0,
        }
    }

    /// Touch the session, updating `last_active` and incrementing the message count.
    pub fn touch(&mut self) {
        self.last_active = Utc::now();
        self.message_count += 1;
    }
}

impl std::fmt::Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = self.label.as_deref().unwrap_or("(untitled)");
        write!(
            f,
            "Session {} [{}] — {} messages, created {}",
            self.id,
            label,
            self.message_count,
            self.created_at.format("%Y-%m-%d %H:%M:%S")
        )
    }
}
