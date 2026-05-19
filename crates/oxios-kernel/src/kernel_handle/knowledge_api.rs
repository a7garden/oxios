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

use oxios_markdown::{Backlink, BacklinkIndex, FileEntry, LinkGraph, VirtualFs, split_posix_path};
use oxios_markdown::parser::{extract_headings, similar};
use oxios_markdown::types::DIR_USER_ROOT;

use crate::engine::EngineProvider;
use crate::memory::{MemoryEntry, MemoryManager, MemoryType};

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
    /// Memory IDs referenced in the response.
    pub referenced_memories: Vec<String>,
}

/// Markdown knowledge base API.
///
/// Wraps [`VirtualFs`] for file I/O, [`BacklinkIndex`] for link tracking,
/// bridges to [`MemoryManager`] for persistent knowledge storage, and
/// provides an AI copilot via [`EngineProvider`].
pub struct KnowledgeApi {
    /// Sandboxed filesystem.
    fs: Arc<RwLock<VirtualFs>>,
    /// Memory manager for persistent knowledge entries.
    memory: Arc<MemoryManager>,
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
        memory: Arc<MemoryManager>,
        engine: Arc<dyn EngineProvider>,
        default_model: String,
    ) -> Self {
        let fs = VirtualFs::new(knowledge_dir).expect("Failed to create VirtualFs for knowledge");
        Self {
            fs: Arc::new(RwLock::new(fs)),
            memory,
            backlinks: Arc::new(RwLock::new(BacklinkIndex::new())),
            engine,
            default_model,
            agent_writes: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Create a KnowledgeApi scoped to a Space's subdirectory.
    pub fn for_space(
        space_dir: &std::path::Path,
        memory: Arc<MemoryManager>,
        engine: Arc<dyn EngineProvider>,
        default_model: String,
    ) -> Self {
        Self::new(space_dir.join("knowledge"), memory, engine, default_model)
    }

    /// Get the root path of the knowledge base.
    pub fn root(&self) -> PathBuf {
        self.fs.read().root().to_path_buf()
    }

    /// Get the default model ID used for copilot chat.
    pub fn model_id(&self) -> &str {
        &self.default_model
    }

    /// Space 전환 시 knowledge base 루트를 교체.
    pub fn switch_space(&self, space_dir: &std::path::Path) -> Result<()> {
        let new_root = space_dir.join("knowledge");
        let new_fs = VirtualFs::new(new_root)?;
        *self.fs.write() = new_fs;
        self.backlinks.write().clear();
        Ok(())
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
        self.index_to_memory(path, content);
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

    // ── Memory indexing helper ─────────────────────────────────────

    /// MemoryManager에 knowledge 엔트리 저장 (fire-and-forget).
    fn index_to_memory(&self, path: &str, content: &str) {
        let tags = extract_headings(content)
            .into_iter()
            .take(5)
            .collect::<Vec<_>>();
        let now = Utc::now();
        let backlink_count = self.backlinks.read().backlink_count(path) as f32;
        let importance = (0.3 + (backlink_count * 0.1).min(0.3))
            .min(1.0);

        let entry = MemoryEntry {
            id: format!("note-{}", path.replace('/', "-").trim_end_matches(".md")),
            memory_type: MemoryType::Knowledge,
            content: content.to_string(),
            source: "knowledge:agent".to_string(),
            session_id: None,
            tags,
            importance,
            created_at: now,
            accessed_at: now,
            access_count: 0,
        };

        let memory = self.memory.clone();
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            handle.spawn(async move {
                let _ = memory.remember(entry).await;
            });
        }
    }

    // ── Search ─────────────────────────────────────────────────────

    /// Search notes by query string.
    ///
    /// Uses name-based fuzzy search via VirtualFs and augments with
    /// semantic search via MemoryManager.
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

        // Semantic search via MemoryManager
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let memory = self.memory.clone();
            let backlinks = self.backlinks.clone();
            let query_owned = query.to_string();
            if let Ok(entries) = handle.block_on(async {
                memory.search(&query_owned, None, limit).await
            }) {
                for entry in entries {
                    if entry.source.starts_with("knowledge:") {
                        let note_path = entry
                            .id
                            .trim_start_matches("note-")
                            .replace("-", "/")
                            + ".md";
                        if hits.iter().any(|h| h.path == note_path) {
                            continue;
                        }
                        let bl_count = backlinks.read().backlink_count(&note_path);
                        hits.push(NoteHit {
                            path: note_path,
                            name: entry.id.clone(),
                            snippet: entry.content.chars().take(200).collect(),
                            semantic_score: Some(entry.importance),
                            backlink_count: bl_count,
                            name_similarity: 0,
                        });
                    }
                }
            }
        }

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

        // 3. 메모리 검색
        let mut referenced_memories = Vec::new();
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let memory = self.memory.clone();
            let q = question.to_string();
            if let Ok(entries) = handle.block_on(async move {
                memory.search(&q, None, 3).await
            }) {
                for mem in &entries {
                    context_parts.push(format!(
                        "## Memory [{}]",
                        mem.memory_type.label()
                    ));
                    referenced_memories.push(mem.id.clone());
                }
            }
        }

        // 4. AI 호출
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
            referenced_memories,
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
                ctx.add_message(oxi_sdk::Message::User(oxi_sdk::UserMessage::new(&q)));

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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_api() -> KnowledgeApi {
        let dir = std::env::temp_dir().join(format!("test-knowledge-{}", uuid::Uuid::new_v4()));
        let state_store = Arc::new(
            crate::state_store::StateStore::new(dir.join("state")).expect("test state store"),
        );
        let memory = Arc::new(crate::memory::MemoryManager::new(state_store));
        let engine = Arc::new(crate::engine::OxiEngineProvider::new("anthropic/claude-sonnet-4"));
        KnowledgeApi::new(
            dir.join("kb"),
            memory,
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
    fn test_switch_space() {
        let api = make_test_api();
        api.note_write("test.md", "old space").unwrap();
        let new_dir = std::env::temp_dir().join(format!("test-knowledge-switch-{}", uuid::Uuid::new_v4()));
        api.switch_space(&new_dir).unwrap();
        // Old content should not be accessible
        assert_eq!(api.note_read("test.md").unwrap(), None);
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
