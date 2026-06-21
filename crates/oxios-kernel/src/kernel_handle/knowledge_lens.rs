//! KnowledgeLens — semantic search overlay for the markdown knowledge base.
//!
//! Wraps a [`KnowledgeBase`] and adds HNSW-based semantic vector search
//! via the agent's [`MemoryManager`]. Provides `recall_for_context()` for
//! injecting relevant knowledge into agent context windows.
//!
//! **RFC-003: Knowledge Base Independent Separation**
//! - Semantic search lives in the kernel (AI layer), not oxios-markdown
//! - `KnowledgeLens` subscribes to `KnowledgeBase.on_file_change()` to keep
//!   the HNSW index in sync automatically

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::memory::{MemoryEntry, MemoryManager, MemoryType};

/// Knowledge context injected into agent prompts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowledgeContext {
    /// Relevant knowledge notes for the query.
    pub notes: Vec<KnowledgeNote>,
    /// Memory entries from agent memory.
    pub memories: Vec<MemoryNote>,
    /// Number of HNSW index entries used.
    pub index_entries_used: usize,
}

/// A knowledge note extracted from the markdown knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNote {
    /// Relative path.
    pub path: String,
    /// Display name.
    pub name: String,
    /// Content snippet.
    pub content: String,
    /// Number of backlinks.
    pub backlink_count: usize,
}

/// A memory entry from the agent's memory system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNote {
    /// Memory ID.
    pub id: String,
    /// Source tag (e.g. "memory:agent", "session:...").
    pub source: String,
    /// Content snippet.
    pub content: String,
    /// Importance score (0-1).
    pub importance: f32,
}

/// Copilot response (AI-powered chat about the knowledge base).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotResponse {
    /// AI-generated answer.
    pub content: String,
    /// Note paths referenced in the response.
    pub referenced_notes: Vec<String>,
    /// Memory IDs referenced in the response.
    pub referenced_memories: Vec<String>,
}

/// KnowledgeLens — semantic overlay over KnowledgeBase.
///
/// Owns the HNSW index (via MemoryManager) and keeps it synchronized
/// with the markdown knowledge base via file-change callbacks.
pub struct KnowledgeLens {
    /// The underlying knowledge base.
    kb: Arc<oxios_markdown::KnowledgeBase>,
    /// Agent memory manager (provides HNSW index + keyword search).
    memory: Arc<MemoryManager>,
    /// Tracks which files were written by agents.
    agent_writes: Arc<RwLock<HashSet<String>>>,
    /// Callback handle for file-change events.
    #[allow(dead_code)]
    callback_handle: Option<mpsc::Sender<oxios_markdown::knowledge::FileChange>>,
}

impl std::fmt::Debug for KnowledgeLens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeLens").finish()
    }
}

impl KnowledgeLens {
    /// Create a new KnowledgeLens wrapping the given knowledge base.
    ///
    /// Registers a file-change callback to keep the memory index in sync.
    pub fn new(
        kb: Arc<oxios_markdown::KnowledgeBase>,
        memory: Arc<MemoryManager>,
    ) -> anyhow::Result<Self> {
        let (tx, mut rx) = mpsc::channel::<oxios_markdown::knowledge::FileChange>(64);
        let tx_for_cb = tx.clone();
        kb.on_file_change(move |_path, event| {
            let tx = tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(event).await;
            });
        });

        let lens = Self {
            kb,
            memory,
            agent_writes: Arc::new(RwLock::new(HashSet::new())),
            callback_handle: Some(tx_for_cb),
        };

        // Spawn background task to process file-change events
        let memory = lens.memory.clone();
        let kb = lens.kb.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                lens_handle_event(kb.clone(), memory.clone(), event);
            }
        });

        Ok(lens)
    }

    /// Get the root path of the knowledge base.
    pub fn root(&self) -> PathBuf {
        self.kb.root()
    }

    /// Get the underlying knowledge base (read-only access).
    pub fn knowledge_base(&self) -> &Arc<oxios_markdown::KnowledgeBase> {
        &self.kb
    }

    /// Mark a file as having been written by an agent.
    pub fn mark_agent_write(&self, path: &str) {
        self.agent_writes.write().insert(path.to_string());
    }

    /// Check if a file was written by an agent.
    pub fn is_agent_write(&self, path: &str) -> bool {
        self.agent_writes.read().contains(path)
    }

    /// Clear the agent-write marker for a file.
    pub fn clear_agent_write(&self, path: &str) {
        self.agent_writes.write().remove(path);
    }

    /// Recall relevant knowledge for a given context/query.
    ///
    /// Combines markdown note search (via KnowledgeBase) with agent memory
    /// search (via MemoryManager). Returns notes ranked by relevance.
    pub async fn recall_for_context(&self, query: &str, limit: usize) -> Result<KnowledgeContext> {
        // Search agent memory for relevant entries
        let mem_entries = self
            .memory
            .search(query, None, limit)
            .await
            .unwrap_or_default();

        let memories: Vec<MemoryNote> = mem_entries
            .iter()
            .map(|e| MemoryNote {
                id: e.id.clone(),
                source: e.source.clone(),
                content: e.content.chars().take(300).collect(),
                importance: e.importance,
            })
            .collect();

        // Search knowledge notes
        let note_hits = self.kb.search(query, limit)?;

        let notes: Vec<KnowledgeNote> = note_hits
            .into_iter()
            .map(|h| {
                let content = self
                    .kb
                    .note_read(&h.path)
                    .ok()
                    .flatten()
                    .map(|c| c.chars().take(500).collect::<String>())
                    .unwrap_or_default();
                KnowledgeNote {
                    path: h.path,
                    name: h.name,
                    content,
                    backlink_count: h.backlink_count,
                }
            })
            .collect();

        Ok(KnowledgeContext {
            notes,
            memories,
            index_entries_used: mem_entries.len(),
        })
    }

    /// Copilot chat — AI-powered question answering about the knowledge base.
    ///
    /// This method is async (uses `provider.stream()` which is Send).
    #[allow(clippy::unused_async)]
    pub async fn copilot_chat(
        &self,
        engine_handle: Arc<crate::engine::EngineHandle>,
        question: &str,
        context_path: Option<&str>,
    ) -> Result<CopilotResponse> {
        let mut context_parts = Vec::new();
        let mut referenced_notes = Vec::new();

        // 1. Current file context
        if let Some(path) = context_path
            && let Ok(Some(content)) = self.kb.note_read(path)
        {
            let snippet: String = content.chars().take(2000).collect();
            context_parts.push(format!("## Current: {path}\n\n{snippet}"));
            referenced_notes.push(path.to_string());
        }

        // 2. Related notes
        let hits = self.kb.search(question, 5).unwrap_or_default();
        for hit in &hits {
            if referenced_notes.contains(&hit.path) {
                continue;
            }
            if let Ok(Some(content)) = self.kb.note_read(&hit.path) {
                let snippet: String = content.chars().take(500).collect();
                context_parts.push(format!("## Related: {}\n\n{}", hit.path, snippet));
                referenced_notes.push(hit.path.clone());
            }
        }

        // 3. Memory context
        let mut referenced_memories = Vec::new();
        if let Ok(entries) = self.memory.search(question, None, 3).await {
            for mem in &entries {
                context_parts.push(format!(
                    "## Memory [{}]: {}",
                    mem.memory_type.label(),
                    mem.content.chars().take(200).collect::<String>()
                ));
                referenced_memories.push(mem.id.clone());
            }
        }

        // 4. AI call
        let system_prompt = format!(
            "You are a knowledge assistant embedded in a markdown note-taking system.\n\
             Answer questions about the user's notes using ONLY the provided context.\n\n\
             ## Rules\n\
             - Only answer based on the context below. If the context doesn't contain\n\
               the answer, say \"I couldn't find relevant notes on that topic.\"\n\
             - Cite which notes you're referencing by name.\n\
             - Be concise — the user is in an editor, not a chat room.\n\
             - Be concise — the user is in an editor, not a chat room.\n\n\
             ## Available Notes\n\n{}",
            context_parts.join("\n\n")
        );

        // Resolve the live default model + a cached provider through the same
        // single source of truth the rest of the kernel uses (interview,
        // execute, persistence). Honors hot-swaps and the user's configured
        // provider/key — fixes the old hardcoded anthropic engine bug.
        let resolved = engine_handle
            .resolve_default()
            .map_err(|e| anyhow::anyhow!("Model/provider: {e}"))?;

        let mut ctx = oxi_sdk::Context::new();
        ctx.set_system_prompt(&system_prompt);
        ctx.add_message(oxi_sdk::Message::User(oxi_sdk::UserMessage::new(question)));

        let stream = resolved
            .provider
            .stream(&resolved.model, &ctx, None)
            .await
            .map_err(|e| anyhow::anyhow!("Stream: {e}"))?;
        let mut text = String::new();
        use futures::StreamExt;
        let mut pinned = std::pin::pin!(stream);
        while let Some(event) = pinned.next().await {
            match event {
                oxi_sdk::ProviderEvent::TextDelta { delta, .. } => text.push_str(&delta),
                oxi_sdk::ProviderEvent::Done { .. } => break,
                oxi_sdk::ProviderEvent::Error { error, .. } => {
                    return Err(anyhow::anyhow!("AI: {error:?}"));
                }
                _ => {}
            }
        }

        Ok(CopilotResponse {
            content: text,
            referenced_notes,
            referenced_memories,
        })
    }
}

// ─── File change event handler ────────────────────────────────────────────────

fn lens_handle_event(
    kb: Arc<oxios_markdown::KnowledgeBase>,
    memory: Arc<MemoryManager>,
    event: oxios_markdown::knowledge::FileChange,
) {
    use oxios_markdown::knowledge::FileChange::*;
    match event {
        Created(path) | Updated(path) => {
            if let Ok(Some(content)) = kb.note_read(&path) {
                index_to_memory(&path, &content, &memory);
            }
        }
        Deleted(path) => {
            let id = format!("note-{}", path.replace('/', "-").trim_end_matches(".md"));
            let rt = tokio::runtime::Handle::try_current();
            if let Ok(handle) = rt {
                let memory = memory.clone();
                handle.spawn(async move {
                    let _ = memory.forget(&id, MemoryType::Knowledge).await;
                });
            }
        }
        Moved { old, new } => {
            let id = format!("note-{}", old.replace('/', "-").trim_end_matches(".md"));
            let rt = tokio::runtime::Handle::try_current();
            if let Ok(handle) = rt {
                let memory = memory.clone();
                let kb = kb.clone();
                let new_path = new.clone();
                handle.spawn(async move {
                    let _ = memory.forget(&id, MemoryType::Knowledge).await;
                    if let Ok(Some(content)) = kb.note_read(&new_path) {
                        index_to_memory(&new_path, &content, &memory);
                    }
                });
            }
        }
    }
}

fn index_to_memory(path: &str, content: &str, memory: &Arc<MemoryManager>) {
    let tags = oxios_markdown::parser::extract_headings(content)
        .into_iter()
        .take(5)
        .collect::<Vec<_>>();
    let now = chrono::Utc::now();
    let importance = 0.5_f32.min(0.3 + (tags.len() as f32 * 0.05));

    let entry = MemoryEntry {
        id: format!("note-{}", path.replace('/', "-").trim_end_matches(".md")),
        memory_type: MemoryType::Knowledge,
        tier: crate::memory::MemoryTier::Warm,
        content: content.to_string(),
        content_hash: 0,
        source: "knowledge:lens".to_string(),
        session_id: None,
        tags,
        importance,
        pinned: false,
        protection: crate::memory::ProtectionLevel::None,
        auto_classified: false,
        session_appearances: 0,
        user_corrected: false,
        seen_in_sessions: vec![],
        created_at: now,
        accessed_at: now,
        modified_at: now,
        access_count: 0,
        decay_score: 1.0,
        compaction_level: 0,
        compacted_from: vec![],
        related_ids: vec![],
        contradicts: None,
    };

    let rt = tokio::runtime::Handle::try_current();
    if let Ok(handle) = rt {
        let memory = memory.clone();
        handle.spawn(async move {
            let _ = memory.remember(entry).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_context_default() {
        let ctx = KnowledgeContext::default();
        assert!(ctx.notes.is_empty());
        assert!(ctx.memories.is_empty());
        assert_eq!(ctx.index_entries_used, 0);
    }

    #[test]
    fn test_knowledge_note_serialization() {
        let note = KnowledgeNote {
            path: "notes/Rust.md".to_string(),
            name: "Rust".to_string(),
            content: "Rust is a systems language".to_string(),
            backlink_count: 3,
        };
        let json = serde_json::to_string(&note).unwrap();
        let restored: KnowledgeNote = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.path, "notes/Rust.md");
        assert_eq!(restored.backlink_count, 3);
    }

    #[test]
    fn test_memory_note_serialization() {
        let note = MemoryNote {
            id: "mem-123".to_string(),
            source: "session:abc".to_string(),
            content: "User prefers dark mode".to_string(),
            importance: 0.85,
        };
        let json = serde_json::to_string(&note).unwrap();
        let restored: MemoryNote = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "mem-123");
        assert!((restored.importance - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_copilot_response_serialization() {
        let resp = CopilotResponse {
            content: "The answer is 42".to_string(),
            referenced_notes: vec!["notes/answer.md".to_string()],
            referenced_memories: vec!["mem-1".to_string()],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: CopilotResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.content, "The answer is 42");
        assert_eq!(restored.referenced_notes.len(), 1);
        assert_eq!(restored.referenced_memories.len(), 1);
    }

    #[test]
    fn test_knowledge_context_with_data() {
        let ctx = KnowledgeContext {
            notes: vec![KnowledgeNote {
                path: "test.md".to_string(),
                name: "Test".to_string(),
                content: "Hello".to_string(),
                backlink_count: 0,
            }],
            memories: vec![],
            index_entries_used: 42,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let restored: KnowledgeContext = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.notes.len(), 1);
        assert_eq!(restored.index_entries_used, 42);
    }
}
