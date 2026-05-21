//! Shared types for the oxios-markdown crate.
//!
//! Core data structures used across all modules.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Directory & Filename Constants
// ============================================================================

/// Root directory identifier.
pub const DIR_USER_ROOT: &str = "/";

/// Archive directory name.
pub const DIR_ARCHIVE: &str = "archive";

/// Media directory name.
pub const DIR_MEDIA: &str = "media";

/// Journal directory name.
pub const DIR_JOURNAL: &str = "journal";

/// Habits directory name.
pub const DIR_HABITS: &str = "habits";

/// Insights directory name.
pub const DIR_INSIGHTS: &str = "insights";

/// Chat filename.
pub const CHAT_FILENAME: &str = "Chat.md";

/// Later filename.
pub const LATER_FILENAME: &str = "Later.md";

/// Done filename.
pub const DONE_FILENAME: &str = "Done.md";

/// Shop filename.
pub const SHOP_FILENAME: &str = "Shop.md";

/// Watch filename.
pub const WATCH_FILENAME: &str = "Watch.md";

/// Read filename.
pub const READ_FILENAME: &str = "Read.md";

/// Pomodoro task marker.
pub const POMODORO_TASK: &str = "Finished a break";

/// Markdown file extension.
pub const MD_EXT: &str = ".md";

// ============================================================================
// File / Entry Types
// ============================================================================

/// A file or directory entry in the knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Filename with extension (e.g., "Rust.md").
    pub name: String,
    /// MD5 hash (first 11 characters) for compact identification.
    pub hash: String,
    /// Display name: capitalized, without extension.
    pub display_name: String,
    /// Creation/modification time in milliseconds since epoch.
    pub ctime: i64,
    /// Whether the file has non-whitespace content.
    pub has_content: bool,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Parent directory path.
    pub parent_dir: String,
}

impl FileEntry {
    /// Create a new file entry.
    pub fn new(
        name: String,
        hash: String,
        display_name: String,
        ctime: i64,
        has_content: bool,
        is_dir: bool,
        parent_dir: String,
    ) -> Self {
        Self {
            name,
            hash,
            display_name,
            ctime,
            has_content,
            is_dir,
            parent_dir,
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Filesystem errors for the knowledge base.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    /// Storage quota exceeded.
    #[error("storage quota exceeded")]
    QuotaExceeded,
    /// Unsafe path (path traversal attempt).
    #[error("unsafe path, possible security issue")]
    UnsafePath,
    /// Cannot reverse a hash to find the original filename.
    #[error("cannot unhash, maybe the file is missing")]
    CannotUnhash,
    /// IO error.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

// ============================================================================
// Sync Types
// ============================================================================

/// Sync status: operation succeeded.
pub const STATUS_OK: &str = "ok";

/// Sync status: file not modified.
pub const STATUS_NOT_MODIFIED: &str = "notModified";

/// Sync status: file was updated on server.
pub const STATUS_UPDATED_ON_SERVER: &str = "updatedOnServer";

/// Sync status: file was merged from both sides.
pub const STATUS_MERGED: &str = "merged";

/// Maximum size for a single text sync (5 MB).
pub const MAX_TEXT_SIZE: usize = 5 * 1024 * 1024;

/// Maximum size for a batch text sync (10 MB).
pub const MAX_TEXTS_SIZE: usize = 10 * 1024 * 1024;

/// Maximum size for a single media sync (20 MB).
pub const MAX_MEDIA_SIZE: usize = 20 * 1024 * 1024;

/// Maximum size for a batch media sync (512 KB).
pub const MAX_MEDIAS_SIZE: usize = 512 * 1024;

/// Maximum size for an auth token (4 KB).
pub const MAX_TOKEN_SIZE: usize = 4 * 1024;

/// A file in the sync protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFile {
    /// Status of this file in the sync response.
    pub status: String,
    /// File path (relative to knowledge base root).
    pub path: String,
    /// Last modified timestamp (ms since epoch).
    #[serde(rename = "lastModified")]
    pub last_modified: i64,
    /// Client's last modification time.
    #[serde(rename = "clientLastModified", default)]
    pub client_last_modified: i64,
    /// Client's last sync time.
    #[serde(rename = "clientLastSynced", default)]
    pub client_last_synced: i64,
    /// File content.
    #[serde(default)]
    pub content: String,
}

/// A batch sync request from the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Modified files from the client.
    pub modified: Vec<SyncFile>,
    /// Deleted file paths from the client.
    pub deleted: Vec<String>,
    /// Client's known directory timestamps.
    pub timestamps: HashMap<String, i64>,
}

/// A sync response to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    /// Overall sync status.
    pub status: String,
    /// Files that need to be sent to the client.
    #[serde(default)]
    pub files: Vec<SyncFile>,
    /// Current directory timestamps on the server.
    #[serde(default)]
    pub timestamps: HashMap<String, i64>,
    /// Rename map: new_path → old_path.
    #[serde(default)]
    pub renames: HashMap<String, String>,
}

impl Default for SyncResponse {
    fn default() -> Self {
        SyncResponse {
            status: STATUS_OK.to_string(),
            files: vec![],
            timestamps: HashMap::new(),
            renames: HashMap::new(),
        }
    }
}

/// Sync-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// Invalid JSON in the request.
    #[error("invalid JSON")]
    InvalidJson,
    /// File not found.
    #[error("file not found")]
    NotFound,
    /// Storage quota exceeded.
    #[error("quota exceeded")]
    QuotaExceeded,
    /// Storage layer error.
    #[error("storage error: {0}")]
    Storage(String),
    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<FsError> for SyncError {
    fn from(err: FsError) -> Self {
        match err {
            FsError::QuotaExceeded => SyncError::QuotaExceeded,
            _ => SyncError::Storage(err.to_string()),
        }
    }
}

// ============================================================================
// Habits Types
// ============================================================================

/// Per-year habit map: day-of-year → status (0=skipped, 1=completed).
pub type YearHabits = HashMap<i32, i32>;

/// All habits: habit name → year data.
pub type Habits = HashMap<String, YearHabits>;

/// Habit skipped marker.
pub const HABIT_SKIPPED: &str = "⚪️";

/// Habit completed marker.
pub const HABIT_COMPLETED: &str = "🟢";

/// Habit completed at weekend marker.
pub const HABIT_COMPLETED_AT_WEEKEND: &str = "🟡";

/// Mood habit name.
pub const MOOD_HABIT: &str = "Mood";

/// Default mood emojis (index = mood level).
pub const MOOD_EMOJIS: &[&str] = &["⚪️", "🤕", "😔", "😐", "🙂", "😊"];

// ============================================================================
// Schedule Types
// ============================================================================

/// A scheduled task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schedule {
    /// Target filename.
    pub filename: String,
    /// Scheduled timestamp (ms since epoch).
    pub scheduled_at: i64,
    /// Cron expression (e.g., "9:00").
    pub cron: String,
    /// Command placeholder (for future use).
    #[serde(default)]
    pub cmd: String,
}

// Knowledge Config Types
// ============================================================================

/// User knowledge base configuration.
///
/// Stored as `config.json` in the knowledge base root.
/// Decoupled from any server-specific config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeConfig {
    /// Language code (e.g., "en", "ko").
    #[serde(default = "default_language")]
    pub language: String,
    /// Timezone string (e.g., "+09:00", "UTC").
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Move-to commands (quick file organization).
    #[serde(default)]
    pub move_to_commands: Vec<String>,
    /// Pomodoro timer duration in minutes.
    #[serde(default = "default_pomodoro_duration")]
    pub pomodoro_duration_in_minutes: i64,
    /// Scheduled tasks.
    #[serde(default)]
    pub schedules: Vec<Schedule>,
    /// Quick commands.
    #[serde(default)]
    pub quick_commands: Vec<String>,
    /// Whether to show two emojis per button.
    #[serde(default)]
    pub two_emojis_enabled: bool,
    /// Mode: "chat", "full", "tasks", "notes", "journal".
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Whether quick habits are enabled.
    #[serde(default)]
    pub quick_habits_enabled: bool,
    /// Associated channel IDs.
    #[serde(default)]
    pub channels: Vec<i64>,
}

fn default_language() -> String {
    "en".to_string()
}
fn default_timezone() -> String {
    "UTC".to_string()
}
fn default_pomodoro_duration() -> i64 {
    50
}
fn default_mode() -> String {
    "full".to_string()
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            timezone: default_timezone(),
            move_to_commands: vec![],
            pomodoro_duration_in_minutes: default_pomodoro_duration(),
            schedules: vec![],
            quick_commands: vec![],
            two_emojis_enabled: false,
            mode: default_mode(),
            quick_habits_enabled: false,
            channels: vec![],
        }
    }
}

/// Chat/Inbox mode constants.
pub const MODE_CHAT: &str = "chat";
/// Full mode constant.
pub const MODE_FULL: &str = "full";
/// Tasks-only mode constant.
pub const MODE_TASKS: &str = "tasks";
/// Notes-only mode constant.
pub const MODE_NOTES: &str = "notes";
/// Journal-only mode constant.
pub const MODE_JOURNAL: &str = "journal";
