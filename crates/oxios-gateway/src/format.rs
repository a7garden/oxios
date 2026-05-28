//! Channel response formatting trait.
//!
//! Each channel implements `ChannelFormatter` to render `OutgoingMessage`
//! in a format appropriate for its output medium (terminal, Telegram, HTTP).
//!
//! The trait lives in the gateway crate; concrete implementations live in
//! each channel crate. Dependency direction: channel → gateway (no cycle).

use crate::message::OutgoingMessage;

/// Channel-specific response formatter.
///
/// Implementations format outgoing messages for display in their target medium.
/// The gateway does **not** call formatters — each channel's `send()` method
/// uses its own formatter internally.
pub trait ChannelFormatter: Send + Sync {
    /// Format a success response.
    fn format_success(&self, msg: &OutgoingMessage) -> String;

    /// Format an error response.
    fn format_error(&self, msg: &OutgoingMessage) -> String;

    /// Format an in-progress status indicator (for non-streaming channels).
    fn format_progress(&self, phase: &str) -> String;
}
