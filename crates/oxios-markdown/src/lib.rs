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
pub mod fs;
#[allow(dead_code)]
pub mod fslog;
pub mod merge;
pub mod parser;
#[allow(dead_code)]
pub mod sync;
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
pub use fs::{VirtualFs, split_posix_path};
pub use fs::split_posix_path;
pub use fslog::FsLog;
pub use merge::merge;
pub use parser::{similar, levenshtein, extract_markdown_links, extract_headings, norm_new_lines,
    today_chat_header, today_journal_path};
pub use sync::SyncEngine;
