//! Message types for the gateway.
//!
//! Messages are channel-agnostic: they carry content and metadata
//! without depending on any specific channel implementation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A message arriving from a channel.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IncomingMessage {
    /// Unique message identifier.
    pub id: uuid::Uuid,
    /// Name of the source channel.
    pub channel: String,
    /// Identifier for the user who sent the message.
    pub user_id: String,
    /// Message content.
    pub content: String,
    /// Timestamp of message creation.
    pub timestamp: DateTime<Utc>,
    /// Optional metadata (e.g., session_id for multi-turn conversations).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl IncomingMessage {
    /// Creates a new incoming message with the current timestamp and empty metadata.
    pub fn new(channel: impl Into<String>, user_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            channel: channel.into(),
            user_id: user_id.into(),
            content: content.into(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }
}

/// A message being sent to a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingMessage {
    /// Unique message identifier.
    pub id: uuid::Uuid,
    /// Name of the target channel.
    pub channel: String,
    /// Identifier for the user who should receive the message.
    pub user_id: String,
    /// Message content.
    pub content: String,
    /// Timestamp of message creation.
    pub timestamp: DateTime<Utc>,
    /// Optional metadata (e.g., session_id, phase, evaluation_passed).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl OutgoingMessage {
    /// Creates a new outgoing message with the current timestamp.
    pub fn new(channel: impl Into<String>, user_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            channel: channel.into(),
            user_id: user_id.into(),
            content: content.into(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Creates a new outgoing message with metadata.
    pub fn with_metadata(
        channel: impl Into<String>,
        user_id: impl Into<String>,
        content: impl Into<String>,
        metadata: HashMap<String, String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            channel: channel.into(),
            user_id: user_id.into(),
            content: content.into(),
            timestamp: Utc::now(),
            metadata,
        }
    }
}
