//! Web response formatter.
//!
//! Web passes OutgoingMessage content through to route handlers as-is.
//! Formatting is the route handler's responsibility (ChatResponse JSON,
//! WebSocket JSON chunks, etc.).

use oxios_gateway::format::ChannelFormatter;
use oxios_gateway::message::OutgoingMessage;

/// Web channel formatter — identity pass-through.
///
/// The web channel uses OutgoingMessage directly (JSON serialized by route handlers).
/// This formatter exists to satisfy the [`ChannelFormatter`] trait but performs no
/// transformation.
#[allow(dead_code)]
pub struct WebFormatter;

impl ChannelFormatter for WebFormatter {
    fn format_success(&self, msg: &OutgoingMessage) -> String {
        msg.content.clone()
    }

    fn format_error(&self, msg: &OutgoingMessage) -> String {
        msg.content.clone()
    }

    fn format_progress(&self, _phase: &str) -> String {
        String::new()
    }
}
