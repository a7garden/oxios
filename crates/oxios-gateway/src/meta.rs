//! Metadata key constants shared across all channels.
//!
//! Standardises the keys used in `IncomingMessage.metadata` and
//! `OutgoingMessage.metadata` `HashMap<String, String>` to prevent typos
//! and improve discoverability.

/// Well-known metadata keys.
#[allow(clippy::module_inception)]
pub mod meta {
    /// Session ID for multi-turn conversations.
    pub const SESSION_ID: &str = "session_id";
    /// Space ID for space-scoped operations.
    pub const SPACE_ID: &str = "space_id";
    /// Chat ID (used by Telegram and similar chat channels).
    pub const CHAT_ID: &str = "chat_id";
    /// Message ID for reply correlation.
    pub const MESSAGE_ID: &str = "message_id";
    /// User ID for authentication context.
    pub const USER_ID: &str = "user_id";
}
