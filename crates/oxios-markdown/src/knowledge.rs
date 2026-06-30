//! KnowledgeBase — markdown knowledge base application layer.
//!
//! Integrates `VirtualFs`, `BacklinkIndex`, and all app-layer features
//! (chat, journal, habits, checklist, etc.) into a single struct.
//!
//! **No kernel dependencies. No AI dependencies.**
//! This crate can be used standalone by any channel (web, CLI, etc.)
//! without going through the kernel.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use parking_lot::{Mutex as ParkingMutex, RwLock};

/// Callback type for file change notifications.
/// Used by [`KnowledgeLens`] to keep the semantic index in sync.
pub type FileChangeCallback = Box<dyn Fn(&str, FileChange) + Send + Sync>;

use crate::backlinks::{Backlink, BacklinkIndex, LinkGraph};
use crate::chat::{delete_chat_msg, move_from_chat, read_chat_msgs, rename_chat_msg};
use crate::checklist::{
    add_checklist_item, checklist_items, complete_checklist_item, incomplete_checklist_items,
    remove_checklist_item, remove_completed_checklist_items,
};
use crate::fs::VirtualFs;
use crate::habits::{habits, last_week_habits, write_habits};
use crate::html::markdown_to_html;
use crate::i18n::emoji_for;
use crate::journal::{add_emoji as journal_add_emoji, add_record as journal_add_record};
use crate::parser::{extract_headings, similar};
use crate::plugins::world_clock_for_names;
use crate::stats::{done_today, today_report};
use crate::types::NoteMeta;
use crate::types::{CHAT_FILENAME, DIR_USER_ROOT, FileEntry, Habits, KnowledgeConfig};
#[cfg(test)]
use crate::types::{NoteQuality, NoteSource};
use crate::worker::{move_due_tasks, remove_completed_items};
use crate::{today_chat_header, today_journal_filename};

/// File change event emitted via `on_file_change` callbacks.
#[derive(Debug, Clone)]
pub enum FileChange {
    /// A new file was created.
    Created(String),
    /// An existing file was updated.
    Updated(String),
    /// A file was deleted.
    Deleted(String),
    /// A file was moved or renamed.
    Moved {
        /// Original path before the move.
        old: String,
        /// New path after the move.
        new: String,
    },
}

/// Knowledge search hit (file-name based).
#[derive(Debug, Clone)]
pub struct NoteHit {
    /// File path relative to knowledge root.
    pub path: String,
    /// Display name of the file.
    pub name: String,
    /// Content snippet.
    pub snippet: String,
    /// Number of backlinks pointing to this note.
    pub backlink_count: usize,
    /// Name similarity score (0–100).
    pub name_similarity: i32,
}

/// Markdown knowledge base application layer.
///
/// Wraps [`VirtualFs`] for sandboxed file I/O, [`BacklinkIndex`] for
/// link tracking, and provides all app-layer features (chat, journal,
/// habits, checklist, etc.).
///
/// **No kernel dependencies.** Can be used standalone by any channel.
pub struct KnowledgeBase {
    /// Sandboxed filesystem.
    fs: RwLock<VirtualFs>,
    /// Bidirectional link index.
    backlinks: RwLock<BacklinkIndex>,
    /// Files written by agents (not by the user).
    agent_writes: ParkingMutex<HashSet<String>>,
    /// Callbacks invoked on file changes.
    /// Used by [`KnowledgeLens`] to keep semantic index in sync.
    on_change: RwLock<Vec<FileChangeCallback>>,
}

impl std::fmt::Debug for KnowledgeBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeBase")
            .field("root", &self.fs.read().root())
            .finish()
    }
}

impl KnowledgeBase {
    /// Create a new KnowledgeBase for the given root directory.
    pub fn new(root: PathBuf) -> Result<Self> {
        let fs = VirtualFs::new(root)?;
        Ok(Self {
            fs: RwLock::new(fs),
            backlinks: RwLock::new(BacklinkIndex::new()),
            agent_writes: ParkingMutex::new(HashSet::new()),
            on_change: RwLock::new(Vec::new()),
        })
    }

    /// Create a new KnowledgeBase scoped to a Space's subdirectory.
    pub fn for_space(space_dir: &std::path::Path) -> Result<Self> {
        Self::new(space_dir.join("knowledge"))
    }

    /// Get the root path of the knowledge base.
    pub fn root(&self) -> PathBuf {
        self.fs.read().root().to_path_buf()
    }

    /// Register a callback to be invoked on every file change.
    ///
    /// The callback receives `(path, FileChange)`.
    /// Multiple callbacks can be registered.
    pub fn on_file_change<F>(&self, f: F)
    where
        F: Fn(&str, FileChange) + Send + Sync + 'static,
    {
        self.on_change.write().push(Box::new(f));
    }

    /// Emit file change notifications to all registered callbacks.
    fn notify_change(&self, path: &str, change: FileChange) {
        for cb in self.on_change.read().iter() {
            cb(path, change.clone());
        }
    }

    // ── File I/O ───────────────────────────────────────────────────

    /// Read a note's content.
    pub fn note_read(&self, path: &str) -> Result<Option<String>> {
        let fs = self.fs.read();
        match fs.read_path(path) {
            Ok(content) => Ok(Some(content)),
            Err(_) => Ok(None),
        }
    }

    /// Read a note's raw bytes — for binary assets (images, etc.) that aren't
    /// valid UTF-8. Text notes should use [`note_read`].
    pub fn note_read_bytes(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let fs = self.fs.read();
        match fs.read_path_bytes(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(_) => Ok(None),
        }
    }

    /// Write a note — creates or overwrites.
    ///
    /// Writes the `.md` file via VirtualFs, updates the backlink index,
    /// and notifies registered `on_file_change` callbacks.
    pub fn note_write(&self, path: &str, content: &str) -> Result<()> {
        // Hold the write lock across the read-check + write so concurrent
        // writers cannot interleave their write_all calls (F1). Drop the
        // lock before notifying callbacks to avoid reentrancy deadlocks.
        let is_new = {
            let fs = self.fs.write();
            let is_new = fs.read_path(path).is_err();
            fs.write_path(path, content)?;
            is_new
        };

        {
            let mut backlinks = self.backlinks.write();
            backlinks.remove_file(path);
            backlinks.index_file(path, content);
        }

        self.notify_change(
            path,
            if is_new {
                FileChange::Created(path.to_string())
            } else {
                FileChange::Updated(path.to_string())
            },
        );
        Ok(())
    }

    /// Write a note with provenance metadata (RFC-022).
    ///
    /// Prepends a YAML frontmatter block with `oxios:` metadata,
    /// then delegates to `note_write`. If the file already has an
    /// `oxios:` frontmatter block, it is merged (preserving `saved_at`,
    /// updating `quality`/`source`). If the file has non-Oxios
    /// frontmatter (e.g., Obsidian tags), it is left intact and
    /// the note is treated as user-authored — no metadata is added.
    pub fn note_write_with_meta(&self, path: &str, content: &str, meta: &NoteMeta) -> Result<bool> {
        // Check existing content for frontmatter
        let existing = self.note_read(path).ok().flatten();
        let final_content = match existing {
            Some(ref existing_content) => {
                let (existing_meta, body) = parse_note_meta(existing_content);
                match existing_meta {
                    // Has Oxios frontmatter — merge
                    Some(old_meta) => {
                        let merged = NoteMeta {
                            saved_at: old_meta.saved_at.or(meta.saved_at.clone()),
                            ..meta.clone()
                        };
                        format_frontmatter(&merged, if body.is_empty() { content } else { &body })
                    }
                    // No Oxios frontmatter — user-authored or foreign frontmatter.
                    // Don't touch it. Return Ok without writing.
                    None => {
                        tracing::debug!(
                            path,
                            "Skipping note_write_with_meta on user-authored note"
                        );
                        return Ok(false);
                    }
                }
            }
            None => format_frontmatter(meta, content),
        };
        self.note_write(path, &final_content).map(|_| true)
    }

    /// List notes that need Dream review (RFC-022).
    ///
    /// Scans the vault for `.md` files with `needs_review: true` in their
    /// Oxios frontmatter. Reads only the frontmatter block (stops at the
    /// closing `---`) for efficiency.
    pub fn notes_needing_review(&self) -> Result<Vec<(String, NoteMeta)>> {
        let fs = self.fs.read();
        let mut result = Vec::new();

        let files = fs.all_md_files()?;
        for (path, _size) in &files {
            if let Ok(content) = fs.read_path(path) {
                let (meta, _body) = parse_note_meta(&content);
                if let Some(m) = meta
                    && m.needs_review
                {
                    result.push((path.clone(), m));
                }
            }
        }

        // Oldest first — they've been raw the longest
        result.sort_by(|a, b| {
            a.1.saved_at
                .as_deref()
                .unwrap_or("")
                .cmp(b.1.saved_at.as_deref().unwrap_or(""))
        });

        Ok(result)
    }
    /// Delete the note at `path`, removing it from the filesystem and
    /// dropping any recorded backlinks for that file.
    pub fn note_delete(&self, path: &str) -> Result<()> {
        {
            let fs = self.fs.write();
            fs.delete_path(path)?;
        }
        self.backlinks.write().remove_file(path);
        self.notify_change(path, FileChange::Deleted(path.to_string()));
        Ok(())
    }

    /// Restore a note's content without triggering file-change callbacks.
    ///
    /// Used when reverting to a previous git version — writes the file
    /// and updates the backlink index, but does **not** fire `on_file_change`
    /// callbacks. This prevents an infinite loop where restore → write →
    /// callback → git commit → ... repeats.
    pub fn note_restore(&self, path: &str, content: &str) -> Result<()> {
        {
            let fs = self.fs.write();
            fs.write_path(path, content)?;
        }
        let mut backlinks = self.backlinks.write();
        backlinks.remove_file(path);
        backlinks.index_file(path, content);
        // Intentionally skip notify_change()
        Ok(())
    }

    /// Move/rename a note.
    pub fn note_move(&self, old_path: &str, new_path: &str) -> Result<()> {
        // Rename under the write lock, then read the destination's content
        // before dropping the guard (note_read would re-acquire the lock).
        let new_content = {
            let fs = self.fs.write();
            fs.rename_path(old_path, new_path)?;
            fs.read_path(new_path).ok()
        };
        {
            let mut backlinks = self.backlinks.write();
            backlinks.remove_file(old_path);
            if let Some(content) = new_content {
                backlinks.index_file(new_path, &content);
            }
        }
        self.notify_change(
            old_path,
            FileChange::Moved {
                old: old_path.to_string(),
                new: new_path.to_string(),
            },
        );
        Ok(())
    }

    /// List notes in a directory.
    pub fn note_tree(&self, dir: &str) -> Result<Vec<FileEntry>> {
        let fs = self.fs.read();
        let dir = if dir.is_empty() || dir == "/" {
            DIR_USER_ROOT
        } else {
            dir
        };
        Ok(fs.files_and_dirs(dir)?)
    }

    // ── Search (file-name based only) ────────────────────────────

    /// Search notes by file name fuzzy matching.
    ///
    /// **Note:** Semantic search is handled by `KnowledgeLens`,
    /// not by this method.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<NoteHit>> {
        let fs = self.fs.read();
        let files = fs.search_files_by_name(query)?;

        let hits: Vec<NoteHit> = files
            .into_iter()
            .take(limit)
            .map(|f| {
                let path = if f.parent_dir == DIR_USER_ROOT || f.parent_dir == "/" {
                    f.name.clone()
                } else {
                    format!("{}/{}", f.parent_dir, f.name)
                };
                let name_sim = similar(&f.display_name, query) as i32;
                let bl_count = self.backlinks.read().backlink_count(&path);
                NoteHit {
                    path,
                    name: f.display_name,
                    snippet: String::new(),
                    backlink_count: bl_count,
                    name_similarity: name_sim,
                }
            })
            .collect();

        Ok(hits)
    }

    // ── Backlinks & Graph ─────────────────────────────────────────

    /// Get backlinks for a note.
    pub fn backlinks_for(&self, path: &str) -> Vec<Backlink> {
        self.backlinks.read().backlinks_for(path)
    }

    /// Get the full link graph for visualization.
    pub fn link_graph(&self) -> LinkGraph {
        self.backlinks.read().link_graph()
    }

    /// Index all markdown files in the knowledge base.
    ///
    /// Walks the entire directory tree and builds the backlink index.
    /// Returns the number of files indexed.
    pub fn index_all(&self) -> Result<usize> {
        let fs = self.fs.read();
        let entries = fs.files_and_dirs(DIR_USER_ROOT)?;
        let mut count = 0;

        for entry in &entries {
            if entry.is_dir {
                let sub = fs.files_and_dirs(&entry.name)?;
                for sub_entry in &sub {
                    if !sub_entry.is_dir && sub_entry.name.ends_with(".md") {
                        let path = format!("{}/{}", entry.name, sub_entry.name);
                        if let Ok(content) = fs.read_path(&path) {
                            self.backlinks.write().index_file(&path, &content);
                            count += 1;
                        }
                    }
                }
            } else if entry.name.ends_with(".md")
                && let Ok(content) = fs.read_path(&entry.name)
            {
                self.backlinks.write().index_file(&entry.name, &content);
                count += 1;
            }
        }

        tracing::info!(files = count, "Knowledge base indexed");
        Ok(count)
    }

    // ── Chat / Inbox ───────────────────────────────────────────────

    /// Append a timestamped message to Chat.md.
    pub fn chat_append(&self, message: &str) -> Result<()> {
        let header = today_chat_header();
        let timestamp = chrono::Local::now().format("`15:04`").to_string();
        let entry = format!("- [ ] {timestamp} {message}");

        let mut content = self.note_read(CHAT_FILENAME)?.unwrap_or_default();
        if !content.contains(&header) {
            if !content.trim_end().ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&header);
            content.push('\n');
        }
        content.push_str(&entry);
        content.push('\n');
        self.note_write(CHAT_FILENAME, &content)?;
        Ok(())
    }

    /// Parse Chat.md into structured message blocks.
    pub fn chat_messages(&self) -> Result<Vec<String>> {
        let content = self.note_read(CHAT_FILENAME)?.unwrap_or_default();
        Ok(read_chat_msgs(&content))
    }

    /// Delete a specific chat message by its content hash.
    pub fn chat_delete(&self, msg_hash: &str) -> Result<bool> {
        let content = self.note_read(CHAT_FILENAME)?.unwrap_or_default();
        match delete_chat_msg(&content, msg_hash) {
            Ok(new_content) => {
                self.note_write(CHAT_FILENAME, &new_content)?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Rename a specific chat message by its content hash.
    pub fn chat_rename(&self, msg_hash: &str, new_body: &str) -> Result<bool> {
        let content = self.note_read(CHAT_FILENAME)?.unwrap_or_default();
        match rename_chat_msg(&content, msg_hash, new_body) {
            Ok(new_content) => {
                self.note_write(CHAT_FILENAME, &new_content)?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Move a chat message to a target file as a checklist item.
    pub fn chat_move_to(&self, msg_hash: &str, target_path: &str) -> Result<bool> {
        let chat_content = self.note_read(CHAT_FILENAME)?.unwrap_or_default();
        let target_content = self.note_read(target_path)?.unwrap_or_default();
        let (new_chat, new_target) = move_from_chat(&chat_content, msg_hash, &target_content);
        if new_chat != chat_content {
            self.note_write(CHAT_FILENAME, &new_chat)?;
            self.note_write(target_path, &new_target)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ── Journal ───────────────────────────────────────────────────

    /// Add a timestamped record to today's journal entry.
    pub fn journal_add_record(&self, record: &str) -> Result<()> {
        let fs = self.fs.write();
        let tz = chrono::Local::now().offset().to_owned();
        journal_add_record(&fs, record, tz)?;
        Ok(())
    }

    /// Add an emoji to today's journal header.
    pub fn journal_add_emoji(&self, emoji: &str) -> Result<()> {
        let fs = self.fs.write();
        let tz = chrono::Local::now().offset().to_owned();
        journal_add_emoji(&fs, emoji, tz)?;
        Ok(())
    }

    /// Get today's journal file path (e.g., "journal/2026.05 May.md").
    pub fn journal_today_path(&self) -> String {
        let tz = chrono::Local::now().offset().to_owned();
        today_journal_filename(tz)
    }

    // ── Habits ───────────────────────────────────────────────────

    /// Read habit tracking data for a given year.
    pub fn habits(&self, year: i32) -> Result<Habits> {
        let fs = self.fs.read();
        Ok(habits(&fs, year)?)
    }

    /// Get last week's habit data.
    pub fn habits_last_week(&self) -> Result<Habits> {
        let fs = self.fs.read();
        let tz = chrono::Local::now().offset().to_owned();
        Ok(last_week_habits(&fs, tz)?)
    }

    /// Write habit data for a year.
    pub fn habits_write(&self, year: i32, habits: &Habits) -> Result<()> {
        let fs = self.fs.write();
        write_habits(&fs, year, habits)?;
        Ok(())
    }

    // ── Config ────────────────────────────────────────────────────

    /// Read the knowledge base config (config.json).
    pub fn config(&self) -> Result<KnowledgeConfig> {
        let fs = self.fs.read();
        match fs.read_path("config.json") {
            Ok(content) => Ok(serde_json::from_str(&content).unwrap_or_default()),
            Err(_) => Ok(KnowledgeConfig::default()),
        }
    }

    /// Write the knowledge base config.
    pub fn set_config(&self, config: &KnowledgeConfig) -> Result<()> {
        let json = serde_json::to_string_pretty(config)?;
        self.note_write("config.json", &json)?;
        Ok(())
    }

    // ── Checklist ────────────────────────────────────────────────

    /// Parse checklist items from a file.
    pub fn checklist_items(
        &self,
        path: &str,
    ) -> Result<(Vec<String>, std::collections::HashMap<String, bool>)> {
        let content = self.note_read(path)?.unwrap_or_default();
        Ok(checklist_items(&content))
    }

    /// Get incomplete checklist items from a file.
    pub fn checklist_incomplete(&self, path: &str) -> Result<Vec<String>> {
        let content = self.note_read(path)?.unwrap_or_default();
        Ok(incomplete_checklist_items(&content))
    }

    /// Add a checklist item to a file.
    pub fn checklist_add(&self, path: &str, item: &str, checked: bool) -> Result<()> {
        let content = self.note_read(path)?.unwrap_or_default();
        let updated = add_checklist_item(&content, item, checked);
        self.note_write(path, &updated)
    }

    /// Complete a checklist item by hash.
    pub fn checklist_complete(&self, path: &str, item_hash: &str) -> Result<bool> {
        let content = self.note_read(path)?.unwrap_or_default();
        let (new_content, found) = complete_checklist_item(&content, item_hash);
        if !found.is_empty() {
            self.note_write(path, &new_content)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove a checklist item by text or hash.
    pub fn checklist_remove(&self, path: &str, item_or_hash: &str) -> Result<bool> {
        let content = self.note_read(path)?.unwrap_or_default();
        let (new_content, removed) = remove_checklist_item(&content, item_or_hash);
        if !removed.is_empty() {
            self.note_write(path, &new_content)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove all completed checklist items.
    pub fn checklist_remove_completed(&self, path: &str) -> Result<(String, String)> {
        let content = self.note_read(path)?.unwrap_or_default();
        let (kept, removed) = remove_completed_checklist_items(&content);
        if !removed.is_empty() {
            self.note_write(path, &kept)?;
        }
        Ok((kept, removed))
    }

    // ── Worker ────────────────────────────────────────────────────

    /// Run nightly cleanup.
    pub fn run_nightly_cleanup(&self) -> Result<crate::worker::NightlyReport> {
        // Read config before acquiring the write lock — config() takes
        // a read lock and would otherwise deadlock against our write guard.
        let config = self.config()?;
        let fs = self.fs.write();
        Ok(remove_completed_items(&fs, &config)?)
    }

    /// Move due scheduled tasks to Chat.
    pub fn run_scheduled_tasks(&self) -> Result<Vec<String>> {
        // Read config first, take the write lock only for the worker pass,
        // then release it before set_config() (which calls note_write and
        // would re-acquire the lock).
        let mut config = self.config()?;
        let moved = {
            let fs = self.fs.write();
            move_due_tasks(&fs, &mut config)?
        };
        if !moved.is_empty() {
            self.set_config(&config)?;
        }
        Ok(moved)
    }

    // ── Stats ────────────────────────────────────────────────────

    /// Get today's completion report.
    pub fn today_report(&self) -> Result<crate::stats::TodayReport> {
        let fs = self.fs.read();
        Ok(today_report(&fs)?)
    }

    /// Get list of files completed today.
    pub fn done_today(&self) -> Result<Vec<FileEntry>> {
        let fs = self.fs.read();
        Ok(done_today(&fs)?)
    }

    // ── Utilities ───────────────────────────────────────────────

    /// Convert markdown to HTML.
    pub fn markdown_to_html(&self, md: &str) -> String {
        markdown_to_html(md)
    }

    /// Find an emoji for a keyword.
    pub fn auto_emoji(&self, text: &str) -> String {
        emoji_for(text)
    }

    /// Generate world clock report for given timezone names.
    pub fn world_clock(&self, timezone_names: &[&str]) -> Vec<crate::plugins::TimezoneEntry> {
        world_clock_for_names(timezone_names)
    }

    // ── Agent Write Tracking ──────────────────────────────────────

    /// Mark a file as having been written by an agent.
    pub fn mark_agent_write(&self, path: &str) {
        self.agent_writes.lock().insert(path.to_string());
    }

    /// Check if a file was written by an agent.
    pub fn is_agent_write(&self, path: &str) -> bool {
        self.agent_writes.lock().contains(path)
    }

    /// Clear the agent-write marker for a file.
    pub fn clear_agent_write(&self, path: &str) {
        self.agent_writes.lock().remove(path);
    }

    // ── Text extraction ──────────────────────────────────────────

    /// Extract text, images, and links from markdown content.
    pub fn extract_text_imgs_links(&self, text: &str) -> crate::tgtxt::ExtractResult {
        crate::tgtxt::extract_text_imgs_links(text)
    }

    // ── Headings (for tag extraction) ─────────────────────────────

    /// Extract headings from content for tag generation.
    pub fn extract_headings(&self, content: &str) -> Vec<String> {
        extract_headings(content).into_iter().take(5).collect()
    }
}

// ---------------------------------------------------------------------------
// Frontmatter helpers (RFC-022)
// ---------------------------------------------------------------------------

/// Parse Oxios frontmatter from a note's content.
///
/// Returns `(Some(NoteMeta), body)` if the `oxios:` key is present in the
/// frontmatter. Returns `(None, original_content)` if there is no frontmatter
/// or the frontmatter does not contain the `oxios:` key (e.g., user-written
/// Obsidian frontmatter). In the latter case, the full original content
/// (including any user frontmatter) is returned as the body.
pub fn parse_note_meta(content: &str) -> (Option<NoteMeta>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let rest = after_first.trim_start_matches(['-', '\n', '\r']);
    if let Some(end_offset) = rest.find("\n---") {
        let yaml_block = &rest[..end_offset];
        let body_start = end_offset + 4; // skip \n---
        let body = rest[body_start..].trim_start().to_string();

        // Parse YAML looking for the `oxios:` key
        if !yaml_block.contains("oxios:") {
            // User frontmatter, not ours
            return (None, content.to_string());
        }

        #[derive(serde::Deserialize)]
        struct FrontmatterWrapper {
            oxios: NoteMeta,
        }

        match serde_yaml::from_str::<FrontmatterWrapper>(yaml_block) {
            Ok(wrapper) => (Some(wrapper.oxios), body),
            Err(_) => (None, content.to_string()),
        }
    } else {
        (None, content.to_string())
    }
}

/// Format a NoteMeta as YAML frontmatter prepended to content.
///
/// `serde_yaml::to_string` produces flat YAML like `author: agent\nsource: Hook\n`.
/// We must indent each line with 2 spaces so they become children of the
/// `oxios:` mapping key.
fn format_frontmatter(meta: &NoteMeta, body: &str) -> String {
    let yaml = serde_yaml::to_string(meta).unwrap_or_default();
    let indented: String = yaml
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("---\noxios:\n{}\n---\n\n{}", indented, body)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_kb() -> KnowledgeBase {
        let dir = std::env::temp_dir().join(format!("test-kb-{}", uuid::Uuid::new_v4()));
        KnowledgeBase::new(dir.join("kb")).expect("test knowledge base")
    }

    #[test]
    fn test_note_write_and_read() {
        let kb = make_test_kb();
        kb.note_write("brain/Rust.md", "# Rust\n\nHello world")
            .unwrap();
        let content = kb.note_read("brain/Rust.md").unwrap();
        assert_eq!(content, Some("# Rust\n\nHello world".to_string()));
    }

    #[test]
    fn test_note_read_missing() {
        let kb = make_test_kb();
        assert_eq!(kb.note_read("nonexistent.md").unwrap(), None);
    }

    #[test]
    fn test_note_delete() {
        let kb = make_test_kb();
        kb.note_write("del.md", "to delete").unwrap();
        kb.note_delete("del.md").unwrap();
        assert_eq!(kb.note_read("del.md").unwrap(), None);
    }

    #[test]
    fn test_note_move() {
        let kb = make_test_kb();
        kb.note_write("old.md", "content").unwrap();
        kb.note_move("old.md", "new.md").unwrap();
        assert_eq!(kb.note_read("old.md").unwrap(), None);
        assert_eq!(kb.note_read("new.md").unwrap(), Some("content".to_string()));
    }

    #[test]
    fn test_backlinks() {
        let kb = make_test_kb();
        kb.note_write("brain/Rust.md", "See [Ownership](brain/Ownership.md)")
            .unwrap();
        let bl = kb.backlinks_for("brain/Ownership.md");
        assert_eq!(bl.len(), 1);
        assert_eq!(bl[0].source_path, "brain/Rust.md");
    }

    #[test]
    fn test_note_tree() {
        let kb = make_test_kb();
        kb.note_write("brain/Rust.md", "Rust").unwrap();
        let entries = kb.note_tree("brain").unwrap();
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_search_by_name() {
        let kb = make_test_kb();
        kb.note_write("brain/Rust.md", "Rust content").unwrap();
        let hits = kb.search("Rust", 10).unwrap();
        assert!(!hits.is_empty());
    }

    #[test]
    fn test_link_graph() {
        let kb = make_test_kb();
        kb.note_write("a.md", "[b](b.md)").unwrap();
        let graph = kb.link_graph();
        assert!(!graph.edges.is_empty());
    }

    #[test]
    fn test_agent_write_tracking() {
        let kb = make_test_kb();
        assert!(!kb.is_agent_write("test.md"));
        kb.mark_agent_write("test.md");
        assert!(kb.is_agent_write("test.md"));
        kb.clear_agent_write("test.md");
        assert!(!kb.is_agent_write("test.md"));
    }

    #[test]
    fn test_index_all() {
        let kb = make_test_kb();
        kb.note_write("brain/Rust.md", "Rust [Go](brain/Go.md)")
            .unwrap();
        kb.note_write("brain/Go.md", "Go language").unwrap();
        kb.note_write("index.md", "Welcome").unwrap();
        let count = kb.index_all().unwrap();
        assert_eq!(count, 3);
        let bl = kb.backlinks_for("brain/Go.md");
        assert_eq!(bl.len(), 1);
    }

    #[test]
    fn test_on_file_change_callback() {
        let kb = make_test_kb();
        let _called = std::sync::atomic::AtomicBool::new(false);
        let path_clone: std::sync::Arc<std::sync::atomic::AtomicBool> =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = path_clone.clone();

        kb.on_file_change(move |path, change| {
            let _ = path;
            let _ = change;
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        kb.note_write("test.md", "hello").unwrap();
        assert!(path_clone.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_chat_append() {
        let kb = make_test_kb();
        kb.chat_append("Test message").unwrap();
        let messages = kb.chat_messages().unwrap();
        // The captured message must be a parseable marker block (- [ ] `HH:MM` text),
        // not merged into the date header. chat_append must emit the `- [ ]` prefix
        // that read_chat_msgs splits on.
        assert!(
            messages
                .iter()
                .any(|m| m.starts_with("- [") && m.contains("Test message")),
            "captured message should be a parseable marker block: {messages:?}"
        );
    }

    #[test]
    fn test_config() {
        let kb = make_test_kb();
        let cfg = kb.config().unwrap();
        // Should return default for non-existent config
        let cfg2 = kb.config().unwrap();
        assert_eq!(cfg.language, cfg2.language);
    }

    #[test]
    fn test_markdown_to_html() {
        let kb = make_test_kb();
        let html = kb.markdown_to_html("# Hello\n\n**world**");
        // markdown_to_html wraps content in a <p> tag by default, check for content
        assert!(html.contains("Hello"), "HTML should contain Hello: {html}");
        assert!(html.contains("world"), "HTML should contain world: {html}");
    }

    #[test]
    fn test_auto_emoji() {
        let kb = make_test_kb();
        let emoji = kb.auto_emoji("cooking pasta");
        assert!(!emoji.is_empty());
    }

    #[test]
    fn test_extract_headings() {
        let kb = make_test_kb();
        let headings = kb.extract_headings("# Title\n\n## Section\n\n### Subsection");
        assert!(headings.len() >= 2);
    }

    #[test]
    fn test_frontmatter_roundtrip() {
        let meta = NoteMeta {
            author: "agent".to_string(),
            source: NoteSource::Hook,
            quality: NoteQuality::Raw,
            needs_review: true,
            session_id: Some("abc123".to_string()),
            message_index: Some(3),
            saved_at: Some("2026-06-13T00:00:00Z".to_string()),
        };
        let body = "## Test\n\nContent here.";
        let formatted = format_frontmatter(&meta, body);
        assert!(formatted.starts_with("---\noxios:\n"));
        let (parsed_meta, parsed_body) = parse_note_meta(&formatted);
        assert!(
            parsed_meta.is_some(),
            "Failed to parse round-tripped frontmatter"
        );
        let pm = parsed_meta.unwrap();
        assert_eq!(pm.author, "agent");
        assert_eq!(pm.session_id.as_deref(), Some("abc123"));
        assert_eq!(pm.message_index, Some(3));
        assert_eq!(parsed_body.trim(), body.trim());
    }

    #[test]
    fn test_parse_user_frontmatter_ignored() {
        let content = "---\ntags: [rust, design]\n---\n\n## My Note\nContent.";
        let (meta, body) = parse_note_meta(content);
        assert!(
            meta.is_none(),
            "User frontmatter should not be parsed as NoteMeta"
        );
        assert!(
            body.contains("tags: [rust, design]"),
            "User frontmatter preserved"
        );
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just a note\nSome content.";
        let (meta, body) = parse_note_meta(content);
        assert!(meta.is_none());
        assert_eq!(body, content);
    }
}
