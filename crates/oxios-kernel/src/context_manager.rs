//! Context Manager — manages LLM context windows like RAM.
//!
//! Inspired by AIOS / AgentRM context management:
//! - 3-tier storage: active (in-context) → cache → archive (compressed)
//! - Context switching (demote old active to cache)
//! - Token counting for context window management
//! - Snapshot & restore capability
//!
//! Just as an OS manages RAM pages, this manages LLM context slots.

use crate::types::AgentId;
use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// The context tier determines where the context is stored and how accessible it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ContextTier {
    /// Active context — currently in the LLM context window.
    /// This is the most expensive tier (uses tokens directly).
    Active,
    /// Cached context — recently used, quickly accessible.
    /// Still in memory but not in the active context window.
    Cache,
    /// Archived context — compressed, stored on disk.
    /// Can be restored to cache or active when needed.
    Archive,
}

/// A single context entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    /// Unique identifier for this context.
    pub id: Uuid,
    /// Associated agent ID (if any).
    pub agent_id: Option<AgentId>,
    /// Session this context belongs to.
    pub session_id: String,
    /// Current storage tier.
    pub tier: ContextTier,
    /// The actual context content.
    pub content: String,
    /// Estimated token count (approximate).
    pub token_count: usize,
    /// When this context was created.
    pub created_at: DateTime<Utc>,
    /// Last time this context was accessed.
    pub last_accessed: DateTime<Utc>,
}

impl ContextEntry {
    /// Creates a new active context entry.
    fn new(
        session_id: String,
        agent_id: Option<AgentId>,
        content: String,
        token_count: usize,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            agent_id,
            session_id: session_id.clone(),
            tier: ContextTier::Active,
            content,
            token_count,
            created_at: now,
            last_accessed: now,
        }
    }
}

/// Context Manager statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextStats {
    /// Total active contexts.
    pub active_count: usize,
    /// Total cached contexts.
    pub cache_count: usize,
    /// Total archived contexts.
    pub archive_count: usize,
    /// Total tokens in active tier.
    pub active_tokens: usize,
    /// Maximum tokens allowed in active tier.
    pub active_limit: usize,
    /// Maximum entries in cache tier.
    pub cache_limit: usize,
}

impl Default for ContextStats {
    fn default() -> Self {
        Self {
            active_count: 0,
            cache_count: 0,
            archive_count: 0,
            active_tokens: 0,
            active_limit: 100_000,
            cache_limit: 50,
        }
    }
}

/// Context Manager.
///
/// Manages LLM context windows like an OS manages RAM.
/// 3-tier storage hierarchy:
/// - **Active**: Current conversation in-context. Limited by token budget.
/// - **Cache**: Recent contexts, quickly accessible. LRU evicted.
/// - **Archive**: Compressed long-term storage. Restored on demand.
///
/// # Usage
/// ```rust,ignore
/// // Store a new context
/// ctx.store_active("session-123", "Hello, how can I help?", 8);
///
/// // Check if we have capacity
/// if ctx.has_capacity(1000) {
///     ctx.store_active("session-123", "Additional context...", 12);
/// }
///
/// // Demote old contexts when switching sessions
/// ctx.demote_to_cache("session-123");
/// ```
pub struct ContextManager {
    /// Maximum tokens in active context.
    active_limit: usize,
    /// Maximum entries in cache tier.
    cache_limit: usize,
    /// All contexts organized by tier.
    contexts: RwLock<HashMap<String, ContextEntry>>,
}

impl ContextManager {
    /// Creates a new context manager.
    ///
    /// # Arguments
    /// * `active_limit` - Maximum tokens in the active tier
    /// * `cache_limit` - Maximum entries in the cache tier
    pub fn new(active_limit: usize, cache_limit: usize) -> Self {
        Self {
            active_limit,
            cache_limit,
            contexts: RwLock::new(HashMap::new()),
        }
    }

    /// Stores a context in the active tier.
    ///
    /// If the active tier is full, older entries are automatically
    /// demoted to the cache tier.
    pub fn store_active(
        &self,
        session_id: &str,
        agent_id: Option<AgentId>,
        content: &str,
        token_count: usize,
    ) -> Result<()> {
        // Enforce capacity limits.
        self.enforce_active_limit(token_count)?;

        let entry = ContextEntry::new(
            session_id.to_string(),
            agent_id,
            content.to_string(),
            token_count,
        );

        let mut contexts = self.contexts.write();
        contexts.insert(session_id.to_string(), entry);

        tracing::debug!(
            session_id = %session_id,
            tokens = token_count,
            active_count = contexts.values().filter(|e| e.tier == ContextTier::Active).count(),
            "Context stored in active tier"
        );

        Ok(())
    }

    /// Gets the active context for a session.
    pub fn get_active(&self, session_id: &str) -> Option<ContextEntry> {
        let mut contexts = self.contexts.write();
        if let Some(entry) = contexts.get_mut(session_id) {
            entry.last_accessed = Utc::now();
            Some(entry.clone())
        } else {
            None
        }
    }

    /// Gets the content of an active context as a string.
    ///
    /// Returns empty string if no context exists.
    pub fn get_active_content(&self, session_id: &str) -> String {
        self.get_active(session_id)
            .map(|e| e.content)
            .unwrap_or_default()
    }

    /// Demotes an active context to the cache tier.
    ///
    /// Call this when switching between sessions or when context
    /// switching is needed to make room for new content.
    pub fn demote_to_cache(&self, session_id: &str) -> Result<()> {
        let mut contexts = self.contexts.write();

        if let Some(entry) = contexts.get_mut(session_id) {
            if entry.tier == ContextTier::Active {
                entry.tier = ContextTier::Cache;
                entry.last_accessed = Utc::now();
                tracing::debug!(session_id = %session_id, "Context demoted to cache tier");
            }
        }

        Ok(())
    }

    /// Demotes all active contexts to cache tier.
    ///
    /// Useful for clearing the active tier when needed.
    pub fn demote_all_to_cache(&self) -> Result<()> {
        let mut contexts = self.contexts.write();

        for entry in contexts.values_mut() {
            if entry.tier == ContextTier::Active {
                entry.tier = ContextTier::Cache;
                entry.last_accessed = Utc::now();
            }
        }

        tracing::info!(
            count = contexts.values().filter(|e| e.tier == ContextTier::Cache).count(),
            "All contexts demoted to cache tier"
        );

        Ok(())
    }

    /// Compresses cache entries to the archive tier.
    ///
    /// Returns the number of entries archived.
    ///
    /// LRU eviction: oldest cache entries are archived first.
    pub fn compress_archive(&self) -> Result<usize> {
        let mut contexts = self.contexts.write();

        // Count cache entries.
        let cache_count = contexts
            .values()
            .filter(|e| e.tier == ContextTier::Cache)
            .count();

        if cache_count <= self.cache_limit {
            return Ok(0);
        }

        // Archive the oldest cache entries (beyond cache_limit).
        let to_archive = cache_count - self.cache_limit;

        // Sort by last_accessed to find LRU entries.
        let mut cache_entries: Vec<_> = contexts
            .iter_mut()
            .filter(|e| e.1.tier == ContextTier::Cache)
            .collect();

        cache_entries.sort_by(|a, b| a.1.last_accessed.cmp(&b.1.last_accessed));

        let mut archived = 0;
        for (_, entry) in cache_entries.into_iter().take(to_archive) {
            entry.tier = ContextTier::Archive;
            entry.last_accessed = Utc::now();
            archived += 1;
        }

        tracing::info!(archived, "Cache entries compressed to archive tier");

        Ok(archived)
    }

    /// Restores an archived context to the cache tier.
    ///
    /// Returns the context if it exists and was archived.
    pub fn restore_from_archive(&self, session_id: &str) -> Result<Option<ContextEntry>> {
        let mut contexts = self.contexts.write();

        if let Some(entry) = contexts.get_mut(session_id) {
            if entry.tier == ContextTier::Archive {
                entry.tier = ContextTier::Cache;
                entry.last_accessed = Utc::now();
                tracing::debug!(session_id = %session_id, "Context restored from archive to cache");
                return Ok(Some(entry.clone()));
            }
        }

        Ok(None)
    }

    /// Returns the total token usage across all active contexts.
    pub fn active_token_usage(&self) -> usize {
        let contexts = self.contexts.read();
        contexts
            .values()
            .filter(|e| e.tier == ContextTier::Active)
            .map(|e| e.token_count)
            .sum()
    }

    /// Checks if the active tier has capacity for the given number of tokens.
    ///
    /// Returns true if adding `tokens` would not exceed the active limit.
    pub fn has_capacity(&self, tokens: usize) -> bool {
        let current = self.active_token_usage();
        current + tokens <= self.active_limit
    }

    /// Returns the current capacity remaining in the active tier.
    pub fn active_capacity_remaining(&self) -> usize {
        self.active_limit.saturating_sub(self.active_token_usage())
    }

    /// Clears all contexts in the active tier.
    ///
    /// Active contexts are demoted to cache, not deleted.
    pub fn clear_active(&self) -> Result<()> {
        self.demote_all_to_cache()
    }

    /// Deletes a context by session ID.
    ///
    /// This removes the context from all tiers.
    pub fn delete(&self, session_id: &str) -> Result<()> {
        let mut contexts = self.contexts.write();
        contexts.remove(session_id);
        tracing::debug!(session_id = %session_id, "Context deleted");
        Ok(())
    }

    /// Returns statistics about the context manager.
    pub fn stats(&self) -> ContextStats {
        let contexts = self.contexts.read();

        let active_count = contexts.values().filter(|e| e.tier == ContextTier::Active).count();
        let cache_count = contexts.values().filter(|e| e.tier == ContextTier::Cache).count();
        let archive_count = contexts.values().filter(|e| e.tier == ContextTier::Archive).count();

        ContextStats {
            active_count,
            cache_count,
            archive_count,
            active_tokens: self.active_token_usage(),
            active_limit: self.active_limit,
            cache_limit: self.cache_limit,
        }
    }

    /// Lists all active session IDs.
    pub fn active_sessions(&self) -> Vec<String> {
        let contexts = self.contexts.read();
        contexts
            .values()
            .filter(|e| e.tier == ContextTier::Active)
            .map(|e| e.session_id.clone())
            .collect()
    }

    /// Gets all contexts for an agent (across all tiers).
    pub fn contexts_for_agent(&self, agent_id: &AgentId) -> Vec<ContextEntry> {
        let contexts = self.contexts.read();
        contexts
            .values()
            .filter(|e| e.agent_id.as_ref() == Some(agent_id))
            .cloned()
            .collect()
    }

    /// Enforces the active token limit by demoting oldest entries.
    fn enforce_active_limit(&self, incoming_tokens: usize) -> Result<()> {
        let mut contexts = self.contexts.write();
        let current_tokens: usize = contexts
            .values()
            .filter(|e| e.tier == ContextTier::Active)
            .map(|e| e.token_count)
            .sum();

        // If we have room, we're good.
        if current_tokens + incoming_tokens <= self.active_limit {
            return Ok(());
        }

        // Need to make room: demote oldest active contexts until we have space.
        let mut active_entries: Vec<_> = contexts
            .iter_mut()
            .filter(|e| e.1.tier == ContextTier::Active)
            .collect();

        active_entries.sort_by(|a, b| a.1.last_accessed.cmp(&b.1.last_accessed));

        let mut freed = 0usize;
        for (_, entry) in active_entries {
            if current_tokens + incoming_tokens - freed <= self.active_limit {
                break;
            }
            entry.tier = ContextTier::Cache;
            entry.last_accessed = Utc::now();
            freed += entry.token_count;
        }

        tracing::debug!(
            freed_tokens = freed,
            "Enforced active limit by demoting contexts"
        );

        Ok(())
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new(100_000, 50)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    

    fn _make_context(
        ctx: &ContextManager,
        session: &str,
        content: &str,
        tokens: usize,
    ) -> Result<()> {
        ctx.store_active(session, None, content, tokens)
    }

    // --- basic store / get tests ---

    #[test]
    fn test_store_and_retrieve() {
        let ctx = ContextManager::new(10_000, 10);

        ctx.store_active("session-1", None, "Hello world", 2).unwrap();
        let entry = ctx.get_active("session-1").unwrap();
        assert_eq!(entry.content, "Hello world");
        assert_eq!(entry.tier, ContextTier::Active);
        assert_eq!(entry.token_count, 2);
    }

    #[test]
    fn test_get_active_content() {
        let ctx = ContextManager::new(10_000, 10);
        ctx.store_active("s1", None, "hello", 1).unwrap();
        assert_eq!(ctx.get_active_content("s1"), "hello");
        assert_eq!(ctx.get_active_content("s2"), ""); // missing → empty
    }

    #[test]
    fn test_get_nonexistent() {
        let ctx = ContextManager::new(10_000, 10);
        assert!(ctx.get_active("none").is_none());
    }

    #[test]
    fn test_store_multiple_sessions() {
        let ctx = ContextManager::new(10_000, 10);
        ctx.store_active("s1", None, "content1", 100).unwrap();
        ctx.store_active("s2", None, "content2", 200).unwrap();

        let stats = ctx.stats();
        assert_eq!(stats.active_count, 2);
        assert_eq!(stats.active_tokens, 300);
    }

    #[test]
    fn test_delete_context() {
        let ctx = ContextManager::new(10_000, 10);
        ctx.store_active("s1", None, "hello", 1).unwrap();
        ctx.delete("s1").unwrap();
        assert!(ctx.get_active("s1").is_none());
        assert_eq!(ctx.stats().active_count, 0);
    }

    // --- demote / compress tests ---

    #[test]
    fn test_demote_to_cache() {
        let ctx = ContextManager::new(10_000, 10);

        ctx.store_active("session-1", None, "Test content", 2).unwrap();
        ctx.demote_to_cache("session-1").unwrap();

        let entry = ctx.get_active("session-1").unwrap();
        assert_eq!(entry.tier, ContextTier::Cache);
    }

    #[test]
    fn test_demote_all_to_cache() {
        let ctx = ContextManager::new(10_000, 10);

        ctx.store_active("s1", None, "A", 10).unwrap();
        ctx.store_active("s2", None, "B", 20).unwrap();
        ctx.demote_all_to_cache().unwrap();

        let stats = ctx.stats();
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.cache_count, 2);
    }

    #[test]
    fn test_clear_active_demotes() {
        let ctx = ContextManager::new(10_000, 10);
        ctx.store_active("s1", None, "A", 10).unwrap();
        ctx.clear_active().unwrap();

        // Clear calls demote_all_to_cache internally.
        assert_eq!(ctx.stats().active_count, 0);
    }

    #[test]
    fn test_compress_lru_eviction() {
        let ctx = ContextManager::new(10_000, 2);

        // Create 5 contexts in cache.
        for i in 0..5 {
            let id = format!("session-{}", i);
            ctx.store_active(&id, None, "content", 10).unwrap();
            ctx.demote_to_cache(&id).unwrap();
        }

        // Compress should archive 3 entries (5 - 2 = 3).
        let archived = ctx.compress_archive().unwrap();
        assert_eq!(archived, 3);

        let stats = ctx.stats();
        assert_eq!(stats.cache_count, 2);
        assert_eq!(stats.archive_count, 3);
    }

    #[test]
    fn test_compress_no_op_when_within_limit() {
        let ctx = ContextManager::new(10_000, 10);

        // Create exactly cache_limit entries.
        for i in 0..5 {
            let id = format!("session-{}", i);
            ctx.store_active(&id, None, "content", 10).unwrap();
            ctx.demote_to_cache(&id).unwrap();
        }

        let archived = ctx.compress_archive().unwrap();
        assert_eq!(archived, 0);
    }

    #[test]
    fn test_restore_from_archive() {
        let ctx = ContextManager::new(10_000, 2);

        // Create 5 cache entries → 3 archived.
        for i in 0..5 {
            let id = format!("session-{}", i);
            ctx.store_active(&id, None, "content", 10).unwrap();
            ctx.demote_to_cache(&id).unwrap();
        }
        let archived = ctx.compress_archive().unwrap();
        assert_eq!(archived, 3);

        // Restore the first archived entry.
        let restored = ctx.restore_from_archive("session-0").unwrap();
        assert!(restored.is_some());
        assert_eq!(restored.unwrap().tier, ContextTier::Cache);

        // Restore a non-archived entry should be None.
        let restored = ctx.restore_from_archive("session-4").unwrap();
        assert!(restored.is_none());
    }

    // --- token limit / capacity tests ---

    #[test]
    fn test_has_capacity() {
        let ctx = ContextManager::new(100, 10);

        assert!(ctx.has_capacity(50));
        assert!(ctx.has_capacity(100));
        assert!(!ctx.has_capacity(101));

        ctx.store_active("session-1", None, "x", 60).unwrap();
        assert!(!ctx.has_capacity(50));
        assert!(ctx.has_capacity(30)); // 60 + 30 = 90 <= 100
    }

    #[test]
    fn test_active_capacity_remaining() {
        let ctx = ContextManager::new(100, 10);
        assert_eq!(ctx.active_capacity_remaining(), 100);

        ctx.store_active("s1", None, "x", 40).unwrap();
        assert_eq!(ctx.active_capacity_remaining(), 60);
    }

    #[test]
    fn test_enforce_active_limit_auto_demotes() {
        let ctx = ContextManager::new(100, 10);

        // Store a context that takes 50 tokens.
        ctx.store_active("s1", None, "A", 50).unwrap();

        // Trying to store 60 tokens should fail without making room.
        // But enforce_active_limit should demote s1 to make room.
        ctx.store_active("s2", None, "B", 60).unwrap();

        let stats = ctx.stats();
        // s1 was demoted, so active only has s2's 60 tokens.
        assert_eq!(stats.active_tokens, 60);
        assert_eq!(stats.cache_count, 1);
    }

    #[test]
    fn test_enforce_limit_multiple_demotions() {
        let ctx = ContextManager::new(100, 10);

        // Add three 40-token contexts.
        ctx.store_active("s1", None, "A", 40).unwrap();
        ctx.store_active("s2", None, "B", 40).unwrap();
        ctx.store_active("s3", None, "C", 40).unwrap();

        // Active is now full (80+40=120 > 100). s1 is demoted to make room.
        let stats = ctx.stats();
        assert!(stats.active_tokens <= 100); // s2 + s3 fit in active (80 tokens)
        assert!(stats.cache_count >= 1); // s1 was demoted
    }

    #[test]
    fn test_active_token_usage() {
        let ctx = ContextManager::new(10_000, 10);

        assert_eq!(ctx.active_token_usage(), 0);

        ctx.store_active("s1", None, "A", 100).unwrap();
        ctx.store_active("s2", None, "B", 200).unwrap();

        assert_eq!(ctx.active_token_usage(), 300);

        // Demote one, should drop to 100.
        ctx.demote_to_cache("s1").unwrap();
        assert_eq!(ctx.active_token_usage(), 200);
    }

    // --- stats ---

    #[test]
    fn test_stats() {
        let ctx = ContextManager::new(1000, 10);

        ctx.store_active("s1", None, "A", 100).unwrap();
        ctx.store_active("s2", None, "B", 200).unwrap();

        let stats = ctx.stats();
        assert_eq!(stats.active_count, 2);
        assert_eq!(stats.active_tokens, 300);
        assert_eq!(stats.active_limit, 1000);
        assert_eq!(stats.cache_limit, 10);
    }

    #[test]
    fn test_active_sessions() {
        let ctx = ContextManager::new(1000, 10);
        ctx.store_active("s1", None, "A", 10).unwrap();
        ctx.store_active("s2", None, "B", 10).unwrap();

        let sessions = ctx.active_sessions();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&"s1".to_string()));
        assert!(sessions.contains(&"s2".to_string()));
    }

    #[test]
    fn test_contexts_for_agent() {
        let ctx = ContextManager::new(1000, 10);
        let agent = AgentId::new_v4();

        ctx.store_active("s1", Some(agent), "A", 10).unwrap();
        ctx.store_active("s2", None, "B", 10).unwrap(); // No agent.
        ctx.store_active("s3", Some(agent), "C", 10).unwrap();

        let contexts = ctx.contexts_for_agent(&agent);
        assert_eq!(contexts.len(), 2);
    }

    #[test]
    fn test_last_accessed_updated_on_get() {
        let ctx = ContextManager::new(1000, 10);
        ctx.store_active("s1", None, "A", 10).unwrap();

        // Get the entry.
        let entry1 = ctx.get_active("s1").unwrap();
        let t1 = entry1.last_accessed;

        // Sleep a bit.
        std::thread::sleep(Duration::from_millis(10));

        // Get again.
        let entry2 = ctx.get_active("s1").unwrap();
        assert!(entry2.last_accessed >= t1);
    }

    #[test]
    fn test_default_context_manager() {
        let ctx = ContextManager::default();
        let stats = ctx.stats();
        assert_eq!(stats.active_limit, 100_000);
        assert_eq!(stats.cache_limit, 50);
    }
}