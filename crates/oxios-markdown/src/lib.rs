//! # oxios-markdown
//!
//! Markdown knowledge management library — ported from [files.md](https://github.com/zakirullin/files.md)
//! by Artem Zakirullin. Licensed under MIT — see LICENSE-THIRD-PARTY.
//!
//! This crate provides core functionality for managing a knowledge base
//! stored as plain `.md` files:
//!
//! - **VirtualFs** — sandboxed filesystem with path traversal protection
//! - **SyncEngine** — mtime-based 3-way merge synchronization
//! - **BacklinkIndex** — bidirectional link tracking between notes
//! - **Journal** — daily journal with timestamped entries
//! - **Habits** — habit tracking with emoji-based visualization
//! - **Chat** — inbox/chat file management
//! - **Schedule** — task scheduling
//! - **Merge** — LCS-based conflict resolution
//!
//! # Example
//!
//! ```no_run
//! use oxios_markdown::VirtualFs;
//! use std::path::PathBuf;
//!
//! let fs = VirtualFs::new(PathBuf::from("/path/to/knowledge")).unwrap();
//! fs.write("brain", "Rust.md", "# Rust\n\nSee [Ownership](brain/Ownership.md)").unwrap();
//! let content = fs.read("brain", "Rust.md").unwrap();
//! ```

#![warn(missing_docs)]

pub mod backlinks;
pub mod chat;
pub mod fs;
pub mod fslog;
pub mod habits;
pub mod journal;
pub mod merge;
pub mod parser;
pub mod schedule;
pub mod sync;
pub mod tokens;
pub mod types;

// Re-export core types for convenience
pub use types::{
    FileEntry, FsError, Habits,
    SyncError, SyncFile, SyncRequest, SyncResponse,
    KnowledgeConfig,
    DIR_ARCHIVE, DIR_MEDIA, DIR_JOURNAL, DIR_USER_ROOT,
    CHAT_FILENAME, LATER_FILENAME, DONE_FILENAME, SHOP_FILENAME, WATCH_FILENAME, READ_FILENAME,
    MD_EXT,
    HABIT_SKIPPED, HABIT_COMPLETED, HABIT_COMPLETED_AT_WEEKEND, MOOD_HABIT, MOOD_EMOJIS,
    STATUS_OK, STATUS_NOT_MODIFIED, STATUS_UPDATED_ON_SERVER, STATUS_MERGED,
    MODE_CHAT, MODE_FULL, MODE_TASKS, MODE_NOTES, MODE_JOURNAL,
};

pub use backlinks::{Backlink, BacklinkIndex, LinkGraph, LinkEdge, LinkNode};
pub use chat::{read_chat_msgs, find_chat_msg_by_hash, rename_chat_msg, delete_chat_msg, today_header as chat_today_header};
pub use fs::VirtualFs;
pub use fslog::FsLog;
pub use habits::{habits, emoji_for_status, habit_emoji, weekday_emoji};
pub use journal::{add_record as journal_add_record, add_emoji as journal_add_emoji, today_journal_filename, today_header as journal_today_header};
pub use merge::merge;
pub use parser::{similar, levenshtein, extract_markdown_links, extract_headings, norm_new_lines};
pub use schedule::{ScheduleManager, format_schedule_date, beginning_of_day, tomorrow_timestamp};
pub use sync::SyncEngine;
pub use tokens::TokenManager;
