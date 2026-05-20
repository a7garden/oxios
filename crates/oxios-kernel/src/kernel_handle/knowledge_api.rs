//! Knowledge API — markdown note management via VirtualFs.
//!
//! 12th KernelHandle API domain. Provides file I/O, backlink tracking,
//! memory sync, semantic search, and an AI-powered copilot for the
//! markdown knowledge base. All file operations go through
//! [`oxios_markdown::VirtualFs`] for sandboxed, path-traversal-safe access.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use oxios_markdown::{Backlink, BacklinkIndex, FileEntry, LinkGraph, VirtualFs};
use oxios_markdown::parser::{extract_headings, similar};
use oxios_markdown::types::DIR_USER_ROOT;

use crate::engine::EngineProvider;

/// Knowledge search hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteHit {
    /// File path relative to knowledge root.
    pub path: String,
    /// Display name of the file.
    pub name: String,
    /// Snippet around the match.
    pub snippet: String,
    /// Semantic similarity score (0.0–1.0), if available.
    pub semantic_score: Option<f32>,
    /// Number of backlinks pointing to this note.
    pub backlink_count: usize,
    /// Name similarity score (0–100).
    pub name_similarity: i32,
}

/// Copilot response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotResponse {
    /// AI-generated answer.
    pub content: String,
    /// Note paths referenced in the response.
    pub referenced_notes: Vec<String>,
}

/// Markdown knowledge base API.
///
/// Wraps [`VirtualFs`] for file I/O, [`BacklinkIndex`] for link tracking,
/// and provides an AI copilot via [`EngineProvider`].
///
/// **RFC-003: KnowledgeBase is now the single source of truth.**
/// HNSW semantic indexing is handled by `KnowledgeLens` in the kernel layer.
pub struct KnowledgeApi {
    /// Sandboxed filesystem.
    fs: Arc<RwLock<VirtualFs>>,
    /// Bidirectional link index.
    backlinks: Arc<RwLock<BacklinkIndex>>,
    /// AI engine provider for copilot chat.
    engine: Arc<dyn EngineProvider>,
    /// Default model ID for copilot chat.
    default_model: String,
    /// Tracks which files were written by agents (not by the user).
    agent_writes: Arc<Mutex<HashSet<String>>>,
}

impl std::fmt::Debug for KnowledgeApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeApi").finish()
    }
}

impl KnowledgeApi {
    /// Create a new KnowledgeApi for the given knowledge directory.
    pub fn new(
        knowledge_dir: PathBuf,
        engine: Arc<dyn EngineProvider>,
        default_model: String,
    ) -> Self {
        let fs = VirtualFs::new(knowledge_dir).expect("Failed to create VirtualFs for knowledge");
        Self {
            fs: Arc::new(RwLock::new(fs)),
            backlinks: Arc::new(RwLock::new(BacklinkIndex::new())),
            engine,
            default_model,
            agent_writes: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Get the root path of the knowledge base.
    pub fn root(&self) -> PathBuf {
        self.fs.read().root().to_path_buf()
    }

    /// Get the default model ID used for copilot chat.
    pub fn model_id(&self) -> &str {
        &self.default_model
    }

    // ── File I/O — POSIX path 기반 ────────────────────────────────

    /// Read a note's content.
    pub fn note_read(&self, path: &str) -> Result<Option<String>> {
        let fs = self.fs.read();
        match fs.read_path(path) {
            Ok(content) => Ok(Some(content)),
            Err(_) => Ok(None),
        }
    }

    /// Write a note — creates or overwrites.
    ///
    /// 1. Writes the `.md` file via VirtualFs.
    /// 2. Updates the backlink index.
    /// 3. Stores as a Knowledge entry in MemoryManager.
    pub fn note_write(&self, path: &str, content: &str) -> Result<()> {
        self.fs.read().write_path(path, content)?;
        self.backlinks.write().index_file(path, content);
        Ok(())
    }

    /// Delete a note.
    pub fn note_delete(&self, path: &str) -> Result<()> {
        self.fs.read().delete_path(path)?;
        self.backlinks.write().remove_file(path);
        Ok(())
    }

    /// Move/rename a note.
    pub fn note_move(&self, old_path: &str, new_path: &str) -> Result<()> {
        self.fs.read().rename_path(old_path, new_path)?;
        self.backlinks.write().remove_file(old_path);
        if let Some(content) = self.note_read(new_path)? {
            self.backlinks.write().index_file(new_path, &content);
        }
        Ok(())
    }

    /// List notes in a directory.
    pub fn note_tree(&self, dir: &str) -> Result<Vec<FileEntry>> {
        let fs = self.fs.read();
        let dir = if dir.is_empty() || dir == "/" { DIR_USER_ROOT } else { dir };
        Ok(fs.files_and_dirs(dir)?)
    }

    // ── Search ─────────────────────────────────────────────────────

    /// Search notes by query string.
    ///
    /// Uses name-based fuzzy search via VirtualFs.
    /// For semantic search, use `KnowledgeLens` instead.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<NoteHit>> {
        let fs = self.fs.read();
        let files = fs.search_files_by_name(query)?;

        let mut hits: Vec<NoteHit> = files
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
                    name: f.display_name.clone(),
                    snippet: String::new(),
                    semantic_score: None,
                    backlink_count: bl_count,
                    name_similarity: name_sim,
                }
            })
            .collect();

        hits.truncate(limit);
        Ok(hits)
    }

    // ── Backlinks & Graph ──────────────────────────────────────────

    /// Get backlinks for a note.
    pub fn backlinks_for(&self, path: &str) -> Vec<Backlink> {
        self.backlinks.read().backlinks_for(path)
    }

    /// Get the full link graph for visualization.
    pub fn link_graph(&self) -> LinkGraph {
        self.backlinks.read().link_graph()
    }

    // ── Index All ──────────────────────────────────────────────────

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
            } else if entry.name.ends_with(".md") {
                if let Ok(content) = fs.read_path(&entry.name) {
                    self.backlinks.write().index_file(&entry.name, &content);
                    count += 1;
                }
            }
        }

        tracing::info!(files = count, "Knowledge base indexed");
        Ok(count)
    }

    // ── Copilot Chat ───────────────────────────────────────────────

    /// AI-powered copilot chat about the knowledge base.
    ///
    /// This is a **sync** method that internally uses `block_in_place`
    /// to run async operations. It gathers context from the current file,
    /// related notes, and memories, then calls the AI engine.
    #[allow(clippy::unused_async)]
    pub fn copilot_chat(
        &self,
        question: &str,
        context_path: Option<&str>,
    ) -> Result<CopilotResponse> {
        let mut context_parts = Vec::new();
        let mut referenced_notes = Vec::new();

        // 1. 현재 파일
        if let Some(path) = context_path {
            if let Some(content) = self.note_read(path)? {
                let snippet: String = content.chars().take(2000).collect();
                context_parts.push(format!("## Current: {}\n\n{}", path, snippet));
                referenced_notes.push(path.to_string());
            }
        }

        // 2. 관련 노트 검색
        let hits = self.search(question, 5).unwrap_or_default();
        for hit in &hits {
            if referenced_notes.contains(&hit.path) {
                continue;
            }
            if let Some(content) = self.note_read(&hit.path)? {
                let snippet: String = content.chars().take(500).collect();
                context_parts.push(format!("## Related: {}\n\n{}", hit.path, snippet));
                referenced_notes.push(hit.path.clone());
            }
        }

        // 3. AI 호출 (KnowledgeLens handles memory context in the agent layer)
        let system_prompt = format!(
            "You are a knowledge assistant embedded in a markdown editor.\
             Answer questions about the user's notes using the provided context.\
             Be concise. Respond in the same language as the question.\n\n\
             ## Context:\n\n{}",
            context_parts.join("\n\n")
        );

        let response_text = self.call_engine(&system_prompt, question)?;

        Ok(CopilotResponse {
            content: response_text,
            referenced_notes,
        })
    }

    /// Call the AI engine synchronously using `block_in_place`.
    fn call_engine(&self, system_prompt: &str, question: &str) -> Result<String> {
        let engine = self.engine.clone();
        let model_id = self.default_model.clone();
        let sp = system_prompt.to_string();
        let q = question.to_string();

        // provider.stream() returns !Send future.
        // Use spawn_blocking to avoid contaminating the async context.
        let result: Result<String> = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let provider_name = model_id.split_once('/').map(|(p, _)| p).unwrap_or("anthropic");
                let provider = engine.create_provider(provider_name)
                    .map_err(|e| anyhow::anyhow!("Provider: {e}"))?;
                let model = engine.resolve_model(&model_id)
                    .map_err(|e| anyhow::anyhow!("Model: {e}"))?;

                let mut ctx = oxi_sdk::Context::new();
                ctx.set_system_prompt(&sp);
                ctx.add_message(oxi_sdk::Message::User(oxi_sdk::UserMessage::new(&*q)));

                let stream = provider.stream(&model, &ctx, None).await
                    .map_err(|e| anyhow::anyhow!("Stream: {e}"))?;

                let mut text = String::new();
                use futures::StreamExt;
                let mut pinned = std::pin::pin!(stream);
                while let Some(event) = pinned.next().await {
                    match event {
                        oxi_sdk::ProviderEvent::TextDelta { delta, .. } => text.push_str(&delta),
                        oxi_sdk::ProviderEvent::Done { .. } => break,
                        oxi_sdk::ProviderEvent::Error { error, .. } => {
                            return Err(anyhow::anyhow!("AI: {:?}", error));
                        }
                        _ => {}
                    }
                }
                Ok(text)
            })
        });
        result
    }

    // ── Agent Writes ───────────────────────────────────────────────

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

    // ═══════════════════════════════════════════════════════════════
    // Chat / Inbox — files.md Chat.md message management
    // ═══════════════════════════════════════════════════════════════

    /// Append a timestamped message to Chat.md.
    ///
    /// Creates or updates the daily header ("#### 20 May, Tuesday")
    /// and appends the message with a timestamp.
    pub fn chat_append(&self, message: &str) -> Result<()> {
        let header = oxios_markdown::today_chat_header();
        let timestamp = chrono::Local::now().format("`15:04`").to_string();
        let entry = format!("{} {}", timestamp, message);

        let mut content = self.note_read(oxios_markdown::CHAT_FILENAME)?.unwrap_or_default();
        if !content.contains(&header) {
            if !content.trim_end().ends_with('\n') { content.push('\n'); }
            content.push_str(&header);
            content.push('\n');
        }
        content.push_str(&entry);
        content.push('\n');
        self.note_write(oxios_markdown::CHAT_FILENAME, &content)?;
        Ok(())
    }

    /// Parse Chat.md into structured message blocks.
    pub fn chat_messages(&self) -> Result<Vec<String>> {
        let content = self.note_read(oxios_markdown::CHAT_FILENAME)?.unwrap_or_default();
        Ok(oxios_markdown::read_chat_msgs(&content))
    }

    /// Delete a specific chat message by its content hash.
    pub fn chat_delete(&self, msg_hash: &str) -> Result<bool> {
        let content = self.note_read(oxios_markdown::CHAT_FILENAME)?.unwrap_or_default();
        match oxios_markdown::delete_chat_msg(&content, msg_hash) {
            Ok(new_content) => {
                self.note_write(oxios_markdown::CHAT_FILENAME, &new_content)?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Rename a specific chat message by its content hash.
    pub fn chat_rename(&self, msg_hash: &str, new_body: &str) -> Result<bool> {
        let content = self.note_read(oxios_markdown::CHAT_FILENAME)?.unwrap_or_default();
        match oxios_markdown::rename_chat_msg(&content, msg_hash, new_body) {
            Ok(new_content) => {
                self.note_write(oxios_markdown::CHAT_FILENAME, &new_content)?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Journal — daily timestamped records
    // ═══════════════════════════════════════════════════════════════

    /// Add a timestamped record to today's journal entry.
    pub fn journal_add_record(&self, record: &str) -> Result<()> {
        let fs = self.fs.read();
        let tz = chrono::Local::now().offset().to_owned();
        oxios_markdown::journal_add_record(&fs, record, tz)?;
        Ok(())
    }

    /// Add an emoji to today's journal header.
    pub fn journal_add_emoji(&self, emoji: &str) -> Result<()> {
        let fs = self.fs.read();
        let tz = chrono::Local::now().offset().to_owned();
        oxios_markdown::journal_add_emoji(&fs, emoji, tz)?;
        Ok(())
    }

    /// Get today's journal file path (e.g., "journal/2026.05 May.md").
    pub fn journal_today_path(&self) -> String {
        let tz = chrono::Local::now().offset().to_owned();
        oxios_markdown::today_journal_filename(tz)
    }

    // ═══════════════════════════════════════════════════════════════
    // Habits — yearly tracking with emoji visualization
    // ═══════════════════════════════════════════════════════════════

    /// Read habit tracking data for a given year.
    ///
    /// Returns a map of habit name → {day_of_year → status}.
    pub fn habits(&self, year: i32) -> Result<oxios_markdown::types::Habits> {
        let fs = self.fs.read();
        Ok(oxios_markdown::habits(&fs, year)?)
    }

    /// Get the emoji for a habit's status on a given day.
    pub fn habit_emoji_for_status(
        &self,
        habit_name: &str,
        day: &chrono::DateTime<chrono::FixedOffset>,
        status: i32,
    ) -> &'static str {
        oxios_markdown::emoji_for_status(habit_name, day, status)
    }

    // ═══════════════════════════════════════════════════════════════
    // Config — knowledge base configuration
    // ═══════════════════════════════════════════════════════════════

    /// Read the knowledge base config (config.json).
    pub fn config(&self) -> Result<oxios_markdown::types::KnowledgeConfig> {
        let fs = self.fs.read();
        match fs.read_path("config.json") {
            Ok(content) => Ok(serde_json::from_str(&content).unwrap_or_default()),
            Err(_) => Ok(oxios_markdown::types::KnowledgeConfig::default()),
        }
    }

    /// Write the knowledge base config.
    pub fn set_config(&self, config: &oxios_markdown::types::KnowledgeConfig) -> Result<()> {
        let json = serde_json::to_string_pretty(config)?;
        self.note_write("config.json", &json)?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // Checklist — Later/Done/Shop/Watch/Read task management
    // ═══════════════════════════════════════════════════════════════

    /// Parse checklist items from a file. Returns (items, is_completed_map).
    pub fn checklist_items(&self, path: &str) -> Result<(Vec<String>, std::collections::HashMap<String, bool>)> {
        let content = self.note_read(path)?.unwrap_or_default();
        Ok(oxios_markdown::checklist::checklist_items(&content))
    }

    /// Get incomplete checklist items from a file.
    pub fn checklist_incomplete(&self, path: &str) -> Result<Vec<String>> {
        let content = self.note_read(path)?.unwrap_or_default();
        Ok(oxios_markdown::checklist::incomplete_checklist_items(&content))
    }

    /// Add a checklist item to a file. Returns the updated content.
    pub fn checklist_add(&self, path: &str, item: &str, checked: bool) -> Result<()> {
        let content = self.note_read(path)?.unwrap_or_default();
        let updated = oxios_markdown::checklist::add_checklist_item(&content, item, checked);
        self.note_write(path, &updated)
    }

    /// Complete a checklist item by hash. Returns true if found.
    pub fn checklist_complete(&self, path: &str, item_hash: &str) -> Result<bool> {
        let content = self.note_read(path)?.unwrap_or_default();
        let (new_content, found) = oxios_markdown::checklist::complete_checklist_item(&content, item_hash);
        if !found.is_empty() {
            self.note_write(path, &new_content)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove a checklist item by text or hash. Returns true if found.
    pub fn checklist_remove(&self, path: &str, item_or_hash: &str) -> Result<bool> {
        let content = self.note_read(path)?.unwrap_or_default();
        let (new_content, removed) = oxios_markdown::checklist::remove_checklist_item(&content, item_or_hash);
        if !removed.is_empty() {
            self.note_write(path, &new_content)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove all completed checklist items. Returns (kept_content, removed_content).
    pub fn checklist_remove_completed(&self, path: &str) -> Result<(String, String)> {
        let content = self.note_read(path)?.unwrap_or_default();
        let (kept, removed) = oxios_markdown::checklist::remove_completed_checklist_items(&content);
        if !removed.is_empty() {
            self.note_write(path, &kept)?;
        }
        Ok((kept, removed))
    }

    // ═══════════════════════════════════════════════════════════════
    // Habits — yearly tracking (write + last week)
    // ═══════════════════════════════════════════════════════════════

    /// Get last week's habit data.
    pub fn habits_last_week(&self) -> Result<oxios_markdown::types::Habits> {
        let fs = self.fs.read();
        let tz = chrono::Local::now().offset().to_owned();
        Ok(oxios_markdown::last_week_habits(&fs, tz)?)
    }

    /// Write habit data for a year.
    pub fn habits_write(&self, year: i32, habits: &oxios_markdown::types::Habits) -> Result<()> {
        let fs = self.fs.read();
        oxios_markdown::write_habits(&fs, year, habits)?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // Worker — nightly cleanup + scheduled tasks
    // ═══════════════════════════════════════════════════════════════

    /// Run nightly cleanup: remove completed items from Chat/Later, archive to Done, journal ✅.
    pub fn run_nightly_cleanup(&self) -> Result<oxios_markdown::worker::NightlyReport> {
        let fs = self.fs.read();
        let config = self.config()?;
        Ok(oxios_markdown::worker::remove_completed_items(&fs, &config)?)
    }

    /// Move due scheduled tasks to Chat. Modifies config in place.
    pub fn run_scheduled_tasks(&self) -> Result<Vec<String>> {
        let fs = self.fs.read();
        let mut config = self.config()?;
        let moved = oxios_markdown::worker::move_due_tasks(&fs, &mut config)?;
        if !moved.is_empty() {
            self.set_config(&config)?;
        }
        Ok(moved)
    }

    // ═══════════════════════════════════════════════════════════════
    // Stats — today's completion report
    // ═══════════════════════════════════════════════════════════════

    /// Get today's completed tasks report.
    pub fn today_report(&self) -> Result<oxios_markdown::stats::TodayReport> {
        let fs = self.fs.read();
        Ok(oxios_markdown::stats::today_report(&fs)?)
    }

    /// Get list of files completed today.
    pub fn done_today(&self) -> Result<Vec<oxios_markdown::FileEntry>> {
        let fs = self.fs.read();
        Ok(oxios_markdown::stats::done_today(&fs)?)
    }

    // ═══════════════════════════════════════════════════════════════
    // HTML — Markdown → HTML conversion
    // ═══════════════════════════════════════════════════════════════

    /// Convert markdown to Telegram-compatible HTML.
    pub fn markdown_to_html(&self, md: &str) -> String {
        oxios_markdown::markdown_to_html(md)
    }

    // ═══════════════════════════════════════════════════════════════
    // i18n — emoji auto-mapping
    // ═══════════════════════════════════════════════════════════════

    /// Find an emoji for a keyword.
    pub fn auto_emoji(&self, text: &str) -> String {
        oxios_markdown::i18n::emoji_for(text)
    }

    // ═══════════════════════════════════════════════════════════════
    // Chat — move from chat to target file
    // ═══════════════════════════════════════════════════════════════

    /// Move a chat message to a target file as a checklist item.
    pub fn chat_move_to(&self, msg_hash: &str, target_path: &str) -> Result<bool> {
        let chat_content = self.note_read(oxios_markdown::CHAT_FILENAME)?.unwrap_or_default();
        let target_content = self.note_read(target_path)?.unwrap_or_default();
        let (new_chat, new_target) = oxios_markdown::move_from_chat(&chat_content, msg_hash, &target_content);
        if new_chat != chat_content {
            self.note_write(oxios_markdown::CHAT_FILENAME, &new_chat)?;
            self.note_write(target_path, &new_target)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Text extraction — images, links from markdown
    // ═══════════════════════════════════════════════════════════════

    /// Extract text, images, and links from markdown content.
    pub fn extract_text_imgs_links(&self, text: &str) -> oxios_markdown::tgtxt::ExtractResult {
        oxios_markdown::tgtxt::extract_text_imgs_links(text)
    }

    // ═══════════════════════════════════════════════════════════════
    // World Clock — timezone report
    // ═══════════════════════════════════════════════════════════════

    /// Generate world clock report for given timezone names.
    pub fn world_clock(&self, timezone_names: &[&str]) -> Vec<oxios_markdown::plugins::TimezoneEntry> {
        oxios_markdown::plugins::world_clock_for_names(timezone_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_api() -> KnowledgeApi {
        let dir = std::env::temp_dir().join(format!("test-knowledge-{}", uuid::Uuid::new_v4()));
        let engine = Arc::new(crate::engine::OxiEngineProvider::new("anthropic/claude-sonnet-4"));
        KnowledgeApi::new(
            dir.join("kb"),
            engine,
            "anthropic/claude-sonnet-4".to_string(),
        )
    }

    #[test]
    fn test_note_write_and_read() {
        let api = make_test_api();
        api.note_write("brain/Rust.md", "# Rust\n\nHello world").unwrap();
        let content = api.note_read("brain/Rust.md").unwrap();
        assert_eq!(content, Some("# Rust\n\nHello world".to_string()));
    }

    #[test]
    fn test_note_read_missing() {
        let api = make_test_api();
        let content = api.note_read("nonexistent.md").unwrap();
        assert_eq!(content, None);
    }

    #[test]
    fn test_note_delete() {
        let api = make_test_api();
        api.note_write("del.md", "to delete").unwrap();
        api.note_delete("del.md").unwrap();
        assert_eq!(api.note_read("del.md").unwrap(), None);
    }

    #[test]
    fn test_note_move() {
        let api = make_test_api();
        api.note_write("old.md", "content").unwrap();
        api.note_move("old.md", "new.md").unwrap();
        assert_eq!(api.note_read("old.md").unwrap(), None);
        assert_eq!(api.note_read("new.md").unwrap(), Some("content".to_string()));
    }

    #[test]
    fn test_backlinks() {
        let api = make_test_api();
        api.note_write("brain/Rust.md", "See [Ownership](brain/Ownership.md)").unwrap();
        let bl = api.backlinks_for("brain/Ownership.md");
        assert_eq!(bl.len(), 1);
        assert_eq!(bl[0].source_path, "brain/Rust.md");
    }

    #[test]
    fn test_note_tree() {
        let api = make_test_api();
        api.note_write("brain/Rust.md", "Rust").unwrap();
        let entries = api.note_tree("brain").unwrap();
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_search_by_name() {
        let api = make_test_api();
        api.note_write("brain/Rust.md", "Rust content").unwrap();
        let hits = api.search("Rust", 10).unwrap();
        assert!(!hits.is_empty());
    }

    #[test]
    fn test_link_graph() {
        let api = make_test_api();
        api.note_write("a.md", "[b](b.md)").unwrap();
        let graph = api.link_graph();
        assert!(!graph.edges.is_empty());
    }

    #[test]
    fn test_agent_write_tracking() {
        let api = make_test_api();
        assert!(!api.is_agent_write("test.md"));
        api.mark_agent_write("test.md");
        assert!(api.is_agent_write("test.md"));
        api.clear_agent_write("test.md");
        assert!(!api.is_agent_write("test.md"));
    }

    #[test]
    fn test_index_all() {
        let api = make_test_api();
        api.note_write("brain/Rust.md", "Rust [Go](brain/Go.md)").unwrap();
        api.note_write("brain/Go.md", "Go language").unwrap();
        api.note_write("index.md", "Welcome").unwrap();
        // Re-index from scratch
        let count = api.index_all().unwrap();
        assert_eq!(count, 3);
        // Backlinks should be present after index_all
        let bl = api.backlinks_for("brain/Go.md");
        assert_eq!(bl.len(), 1);
    }

    #[test]
    #[ignore = "Requires API key and network access"]
    fn test_copilot_chat() {
        let api = make_test_api();
        api.note_write("brain/Rust.md", "# Rust\n\nA systems programming language.").unwrap();
        let response = api.copilot_chat("What is Rust?", Some("brain/Rust.md")).unwrap();
        assert!(!response.content.is_empty());
        assert!(response.referenced_notes.contains(&"brain/Rust.md".to_string()));
    }
}
