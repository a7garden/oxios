//! Alarm dispatcher — stub for alarm ticking and dispatch.

use crate::types::AlarmEvent;

/// Dispatches alarm events to channels.
///
/// This is currently a stub that logs alarm events. In the future, it will
/// dispatch alarms via the gateway (push notification, Telegram, etc.).
pub struct AlarmDispatcher;

impl AlarmDispatcher {
    /// Create a new alarm dispatcher.
    pub fn new() -> Self {
        Self
    }

    /// Dispatch a single alarm event.
    ///
    /// Currently logs the alarm. Will be extended to send via gateway channels.
    pub async fn dispatch(&self, alarm: &AlarmEvent) -> anyhow::Result<()> {
        tracing::info!(
            "Alarm: '{}' — {} minutes before (UID: {})",
            alarm.event_title,
            alarm.minutes_before,
            alarm.event_uid
        );
        Ok(())
    }
}

impl Default for AlarmDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
