//! Agent API — agent lifecycle, budget, memory.

use std::sync::Arc;
use crate::supervisor::Supervisor;
use crate::budget::{BudgetManager, BudgetInfo, BudgetLimit, BudgetExceeded};
use crate::memory::{MemoryEntry, MemoryType, MemoryManager};
use crate::types::AgentId;

/// Agent management system calls.
pub struct AgentApi {
    pub(crate) supervisor: Arc<dyn Supervisor>,
    pub(crate) budget_manager: Arc<BudgetManager>,
    pub(crate) memory_manager: Arc<MemoryManager>,
}

impl AgentApi {
    /// Create a new AgentApi.
    pub fn new(
        supervisor: Arc<dyn Supervisor>,
        budget_manager: Arc<BudgetManager>,
        memory_manager: Arc<MemoryManager>,
    ) -> Self {
        Self { supervisor, budget_manager, memory_manager }
    }
    /// List running agents.
    pub async fn list(&self) -> anyhow::Result<Vec<crate::types::AgentInfo>> {
        self.supervisor.list().await.map_err(|e| anyhow::anyhow!("supervisor: {e}"))
    }

    /// Kill a running agent.
    pub async fn kill(&self, agent_id: &str) -> anyhow::Result<()> {
        let id = uuid::Uuid::parse_str(agent_id)
            .map_err(|e| anyhow::anyhow!("invalid agent id: {e}"))?;
        self.supervisor.kill(id).await.map_err(|e| anyhow::anyhow!("supervisor: {e}"))
    }

    /// Check budget for an agent.
    pub fn check_budget(&self, agent_id: &AgentId) -> BudgetInfo {
        self.budget_manager.remaining(agent_id)
    }

    /// Set budget for an agent.
    pub fn set_budget(&self, limit: BudgetLimit) {
        self.budget_manager.set_budget(limit);
    }

    /// Remove budget for an agent.
    pub fn remove_budget(&self, agent_id: &AgentId) {
        self.budget_manager.remove_budget(agent_id);
    }

    /// Reserve tokens for an agent.
    pub fn reserve_budget(&self, agent_id: &AgentId, tokens: u64) -> Result<(), BudgetExceeded> {
        self.budget_manager.reserve(agent_id, tokens)
    }

    /// Reset budget window for an agent.
    pub fn reset_budget(&self, agent_id: &AgentId) {
        self.budget_manager.reset_window(agent_id);
    }

    /// Get memory stats.
    pub async fn memory_stats(&self) -> (usize, usize) {
        (self.memory_manager.vector_index_size(), self.memory_manager.total_entries().await)
    }

    /// Store a memory entry.
    pub async fn remember(&self, entry: MemoryEntry) -> anyhow::Result<String> {
        self.memory_manager.remember(entry).await
    }

    /// Search memory entries.
    pub async fn search_memory(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        self.memory_manager.search(query, memory_type, limit).await
    }
}
