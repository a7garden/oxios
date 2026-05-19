//! Knowledge API — markdown note management via VirtualFs.
//!
//! 12th KernelHandle API domain. Provides file I/O, backlink tracking,
//! and memory sync for the markdown knowledge base. All file operations
//! go through [`oxios_markdown::VirtualFs`] for sandboxed, path-traversal-safe
//! access.
//!
//! **Note**: `copilot_chat` is deliberately NOT included here. That belongs
//! in Phase 4 (Web UI). This API handles file I/O + backlinks + memory sync only.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use parking_lot::RwLock;

use oxios_markdown::{
    Backlink, BacklinkIndex, FileEntry, LinkGraph, VirtualFs,
};
use oxios_markdown::parser::{extract_headings, similar};
use oxios_markdown::types::DIR_USER_ROOT;

use crate::memory::{MemoryEntry, MemoryManager, MemoryType};

/// A hit from knowledge search.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// Markdown knowledge base API.
///
/// Wraps [`VirtualFs`] for file I/O, [`BacklinkIndex`] for link tracking,
/// and bridges to [`MemoryManager`] for persistent knowledge storage.
pub struct KnowledgeApi {
    /// Sandboxed filesystem.
    fs: Arc<RwLock<VirtualFs>>,
    /// Memory manager for persistent knowledge entries.
    memory: Arc<MemoryManager>,
    /// Bidirectional link index.
    backlinks: Arc<RwLock<BacklinkIndex>>,
}

impl std::fmt::Debug for KnowledgeApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeApi").finish()
    }
}

impl KnowledgeApi {
    /// Create a new KnowledgeApi for the given knowledge directory.
    pub fn new(knowledge_dir: PathBuf, memory: Arc<MemoryManager>) -> Self {
        let fs = VirtualFs::new(knowledge_dir).expect("Failed to create VirtualFs for knowledge");
        Self {
            fs: Arc::new(RwLock::new(fs)),
            memory,
            backlinks: Arc::new(RwLock::new(BacklinkIndex::new())),
        }
    }

    /// Create a KnowledgeApi scoped to a Space's subdirectory.
    pub fn for_space(space_dir: &std::path::Path, memory: Arc<MemoryManager>) -> Self {
        let knowledge_dir = space_dir.join("knowledge");
        Self::new(knowledge_dir, memory)
    }

    /// Get the root path of the knowledge base.
    pub fn root(&self) -> PathBuf {
        self.fs.read().root().to_path_buf()
    }

    // ── File I/O ────────────────────────────────────────────────

    /// Read a note's content.
    ///
    /// `path` is interpreted as "dir/filename" — the first path component
    /// is the directory, the rest is the filename. For root-level files,
    /// use `"/"` as the dir prefix (or just pass the filename directly).
    pub fn note_read(&self, path: &str) -> Result<Option<String>> {
        let (dir, filename) = split_path(path);
        let fs = self.fs.read();
        match fs.read(dir, filename) {
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
        let (dir, filename) = split_path(path);

        // 1. Write .md file
        self.fs.read().write(dir, filename, content)?;

        // 2. Update backlink index
        self.backlinks.write().index_file(path, content);

        // 3. Store in MemoryManager as Knowledge
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

        // Fire-and-forget async — note_write is sync but memory is async.
        let memory = self.memory.clone();
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            handle.spawn(async move {
                let _ = memory.remember(entry).await;
            });
        }
        // If no tokio runtime, silently skip memory indexing (tests, CLI).

        Ok(())
    }

    /// Delete a note.
    pub fn note_delete(&self, path: &str) -> Result<()> {
        let (dir, filename) = split_path(path);
        self.fs.read().del(dir, filename)?;
        self.backlinks.write().remove_file(path);
        Ok(())
    }

    /// Move/rename a note.
    pub fn note_move(&self, old_path: &str, new_path: &str) -> Result<()> {
        let (old_dir, old_filename) = split_path(old_path);
        let (new_dir, new_filename) = split_path(new_path);

        self.fs.read().rename(old_dir, old_filename, new_dir, new_filename)?;

        // Re-index at new location
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

    // ── Search ──────────────────────────────────────────────────

    /// Search notes by query string.
    ///
    /// Uses name-based fuzzy search via VirtualFs and augments with
    /// backlink counts.
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

        // Also try semantic search via MemoryManager
        let memory = self.memory.clone();
        let query_owned = query.to_string();
        let backlinks = self.backlinks.clone();

        // We do a blocking wait here since `search` is a sync method.
        // Use tokio::task::block_in_place for the async memory search.
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            let memory_hits = handle.block_on(async {
                memory.search(&query_owned, None, limit).await
            });
            if let Ok(entries) = memory_hits {
                for entry in entries {
                    if entry.source.starts_with("knowledge:") {
                        let note_path = entry
                            .id
                            .trim_start_matches("note-")
                            .replace("-", "/")
                            + ".md";
                        // Don't add duplicates
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

    // ── Backlinks & Graph ───────────────────────────────────────

    /// Get backlinks for a note.
    pub fn backlinks_for(&self, path: &str) -> Vec<Backlink> {
        self.backlinks.read().backlinks_for(path)
    }

    /// Get the full link graph for visualization.
    pub fn link_graph(&self) -> LinkGraph {
        self.backlinks.read().link_graph()
    }
}

/// Split a path like "brain/Rust.md" into ("brain", "Rust.md").
/// Root-level files like "Chat.md" become ("/", "Chat.md").
fn split_path(path: &str) -> (&str, &str) {
    let path = path.trim_start_matches('/');
    if let Some(slash_pos) = path.find('/') {
        let (dir, file) = path.split_at(slash_pos);
        (dir, &file[1..]) // skip the '/'
    } else {
        (DIR_USER_ROOT, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_path_subdir() {
        assert_eq!(split_path("brain/Rust.md"), ("brain", "Rust.md"));
    }

    #[test]
    fn test_split_path_root() {
        assert_eq!(split_path("Chat.md"), ("/", "Chat.md"));
    }

    #[test]
    fn test_split_path_leading_slash() {
        assert_eq!(split_path("/brain/Rust.md"), ("brain", "Rust.md"));
    }

    #[test]
    fn test_split_path_nested() {
        assert_eq!(split_path("a/b/c.md"), ("a", "b/c.md"));
    }

    fn make_test_api() -> KnowledgeApi {
        let dir = std::env::temp_dir().join(format!("test-knowledge-{}", uuid::Uuid::new_v4()));
        let state_store = Arc::new(
            crate::state_store::StateStore::new(dir.join("state")).expect("test state store"),
        );
        let memory = Arc::new(crate::memory::MemoryManager::new(state_store));
        KnowledgeApi::new(dir.join("kb"), memory)
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
}
