//! # oxios-markdown
//!
//! Markdown knowledge management library — ported from [files.md](https://github.com/zakirullin/files.md)
//! by Artem Zakirullin. Licensed under MIT — see LICENSE-THIRD-PARTY.
//!
//! This crate provides core functionality for managing a knowledge base
//! stored as plain `.md` files:
//!
//! - **VirtualFs** — sandboxed filesystem with path traversal protection
//! - **BacklinkIndex** — bidirectional link tracking between notes
//! - **Merge** — LCS-based conflict resolution
//! - **Parser** — text processing utilities (similarity, links, headings)
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
pub mod checklist;
pub mod fs;
pub mod html;
#[allow(dead_code)]
pub mod fslog;
pub mod habits;
pub mod journal;
pub mod merge;
pub mod parser;
pub mod schedule;
#[allow(dead_code)]
pub mod sync;
pub mod tokens;
pub mod types;
pub mod worker;

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
pub use chat::{read_chat_msgs, find_chat_msg_by_hash, rename_chat_msg, delete_chat_msg, append_to_chat_msg, move_from_chat, today_header as chat_today_header};
pub use fs::VirtualFs;
pub use fs::split_posix_path;
pub use fslog::FsLog;
pub use habits::{habits, last_week_habits, write_habits, emoji_for_status, habit_emoji, weekday_emoji};
pub use journal::{add_record as journal_add_record, add_emoji as journal_add_emoji, today_journal_filename, today_header as journal_today_header};
pub use html::{markdown_to_html, escape_html, strip_html_tags, replace_with_placeholders, restore_from_placeholders};
pub use merge::merge;
pub use parser::{similar, levenshtein, extract_markdown_links, extract_headings, norm_new_lines,
    today_chat_header, today_journal_path,
    ucfirst, lcfirst, substr, is_multiline, split_text_into_chunks, emoji_prefix};
pub use checklist::{checklist_items, incomplete_checklist_items, add_checklist_item,
    complete_checklist_item, remove_checklist_item, remove_completed_checklist_items,
    checklist_item, add_header_and_text};
pub use worker::{remove_completed_items, remove_completed_checklist, remove_completed_inbox_entries, move_due_tasks, schedule_report, next_exclude_today, NightlyReport};
pub use sync::SyncEngine;
pub use tokens::TokenManager;
