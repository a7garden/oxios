//! Core types for the calendar system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Source of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EventSource {
    /// Created by an agent.
    #[default]
    Agent,
    /// Created by the user.
    User,
    /// Created by a cron/scheduled task.
    Cron,
}

/// Simple repeat rule (agent-facing, converts to RRULE).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repeat {
    /// Frequency: daily, weekly, monthly, yearly.
    pub frequency: String,
    /// Days of the week (for weekly: mon, tue, wed, thu, fri, sat, sun).
    #[serde(default)]
    pub days: Vec<String>,
    /// Interval between recurrences (default: 1).
    #[serde(default = "default_interval")]
    pub interval: u32,
    /// Repeat until this ISO date.
    pub until: Option<String>,
    /// Maximum number of occurrences.
    pub count: Option<u32>,
}

fn default_interval() -> u32 {
    1
}

/// Draft for creating a new event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDraft {
    /// Event title / summary.
    pub title: String,
    /// Start time (UTC).
    pub start: DateTime<Utc>,
    /// End time (UTC).
    pub end: DateTime<Utc>,
    /// Whether this is an all-day event.
    #[serde(default)]
    pub all_day: bool,
    /// Optional description.
    pub description: Option<String>,
    /// Optional location.
    pub location: Option<String>,
    /// Optional repeat rule.
    pub repeat: Option<Repeat>,
    /// Optional path of a linked knowledge note (stored as `X-OXIOS-NOTE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_path: Option<String>,
    /// Reminder offsets in minutes before the event.
    #[serde(default)]
    pub reminder_minutes: Vec<u32>,
    /// Event source.
    #[serde(default)]
    pub source: EventSource,
}

/// Patch for updating an existing event.
///
/// All fields are optional. `None` means "don't change". Inner `Option` fields
/// (like `description`) use `Some(None)` to clear the value.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventPatch {
    /// New title.
    pub title: Option<String>,
    /// New start time.
    pub start: Option<DateTime<Utc>>,
    /// New end time.
    pub end: Option<DateTime<Utc>>,
    /// Toggle all-day.
    pub all_day: Option<bool>,
    /// Set or clear description. `Some(None)` clears it.
    pub description: Option<Option<String>>,
    /// Set or clear location. `Some(None)` clears it.
    pub location: Option<Option<String>>,
    /// Set or clear repeat rule. `Some(None)` clears it.
    pub repeat: Option<Option<Repeat>>,
    /// Set or clear linked knowledge note. `Some(None)` clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_path: Option<Option<String>>,
    /// Replace reminder minutes.
    pub reminder_minutes: Option<Vec<u32>>,
}

/// A calendar event (parsed from .ics).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique identifier (UID in iCalendar).
    pub uid: String,
    /// Event title / summary.
    pub title: String,
    /// Start time (UTC).
    pub start: DateTime<Utc>,
    /// End time (UTC).
    pub end: DateTime<Utc>,
    /// Whether this is an all-day event.
    pub all_day: bool,
    /// Optional description.
    pub description: Option<String>,
    /// Optional location.
    pub location: Option<String>,
    /// Raw RRULE string (e.g. `FREQ=WEEKLY;BYDAY=MO,WE,FR`).
    pub rrule: Option<String>,
    /// Event status (CONFIRMED, TENTATIVE, CANCELLED).
    pub status: String,
    /// Where the event came from.
    pub source: EventSource,
    /// Filename of the .ics file (e.g. `abc123.ics`).
    pub filename: String,
    /// Optional path of a linked knowledge note (stored as `X-OXIOS-NOTE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_path: Option<String>,
}

/// Index entry for fast lookup without parsing .ics files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// .ics filename.
    pub file: String,
    /// Event summary.
    pub summary: String,
    /// Start time.
    pub dtstart: DateTime<Utc>,
    /// End time.
    pub dtend: DateTime<Utc>,
    /// Raw RRULE string, if recurring.
    pub rrule: Option<String>,
    /// Event status.
    pub status: String,
    /// Event source.
    pub source: EventSource,
}

/// Result of creating an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResult {
    /// UID of the created event.
    pub uid: String,
    /// Status of the created event.
    pub status: String,
    /// Conflicts detected during creation.
    pub conflicts: Vec<Conflict>,
    /// Filename of the .ics file.
    pub file: String,
}

/// Result of updating an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    /// UID of the updated event.
    pub uid: String,
    /// Status of the updated event.
    pub status: String,
    /// Conflicts detected after update.
    pub conflicts: Vec<Conflict>,
}

/// A conflict between two events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// UID of the conflicting event.
    pub uid: String,
    /// Title of the conflicting event.
    pub title: String,
    /// Overlap duration in minutes.
    pub overlap_minutes: u32,
}

/// Free/busy slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeBusySlot {
    /// Slot start time.
    pub start: DateTime<Utc>,
    /// Slot end time.
    pub end: DateTime<Utc>,
    /// Whether this slot is busy.
    pub busy: bool,
}

/// A synthetic event (e.g. from cron expansion).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticEvent {
    /// Event title.
    pub title: String,
    /// Start time.
    pub start: DateTime<Utc>,
    /// End time.
    pub end: DateTime<Utc>,
}

/// Alarm event for dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmEvent {
    /// UID of the parent event.
    pub event_uid: String,
    /// Title of the parent event.
    pub event_title: String,
    /// When the alarm should fire.
    pub trigger_at: DateTime<Utc>,
    /// How many minutes before the event.
    pub minutes_before: u32,
}
