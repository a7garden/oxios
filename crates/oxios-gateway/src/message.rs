//! Message types for the gateway.
//!
//! Messages are channel-agnostic: they carry content and metadata
//! without depending on any specific channel implementation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A message arriving from a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl IncomingMessage {
    /// Creates a new incoming message with the current timestamp.
    pub fn new(channel: impl Into<String>, user_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            channel: channel.into(),
            user_id: user_id.into(),
            content: content.into(),
            timestamp: Utc::now(),
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
        }
    }
}
