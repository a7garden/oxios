//! Budget manager for agent-level token and call budget tracking.
//!
//! Implements sliding window budgets that reset after a configurable time period.
//! Budgets track tokens and calls per agent, preventing resource exhaustion attacks
//! and enforcing fair usage policies.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::types::AgentId;

/// Budget limit configuration for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetLimit {
    /// The agent this budget applies to.
    pub agent_id: AgentId,
    /// Maximum tokens allowed in the window.
    pub token_budget: u64,
    /// Maximum calls allowed in the window.
    pub calls_budget: u64,
    /// Window duration in seconds before reset.
    pub window_secs: u64,
}

/// Current usage state for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens consumed in the current window.
    pub tokens_used: u64,
    /// Calls made in the current window.
    pub calls_used: u64,
    /// When the current window started.
    pub window_start: DateTime<Utc>,
}

/// Budget information returned to callers.
#[derive(Debug, Clone)]
pub struct BudgetInfo {
    /// Tokens remaining in the current window.
    pub tokens_remaining: u64,
    /// Calls remaining in the current window.
    pub calls_remaining: u64,
    /// Seconds remaining until window resets.
    pub window_remaining_secs: u64,
    /// Whether all budget has been exhausted.
    pub is_exhausted: bool,
}

/// Full budget information including limits and usage.
/// Used by the web dashboard to display comprehensive budget data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullBudgetInfo {
    /// The agent this budget applies to.
    pub agent_id: AgentId,
    /// Maximum tokens allowed in the window.
    pub token_limit: u64,
    /// Tokens consumed in the current window.
    pub tokens_used: u64,
    /// Tokens remaining in the current window.
    pub tokens_remaining: u64,
    /// Maximum calls allowed in the window.
    pub calls_limit: u64,
    /// Calls made in the current window.
    pub calls_used: u64,
    /// Calls remaining in the current window.
    pub calls_remaining: u64,
    /// Window duration in seconds.
    pub window_secs: u64,
    /// Seconds remaining until window resets.
    pub window_remaining_secs: u64,
    /// Whether all budget has been exhausted.
    pub is_exhausted: bool,
}

/// Kind of budget that was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetKind {
    /// Token budget exceeded.
    Token,
    /// Call budget exceeded.
    Call,
}

/// Error when a budget limit is exceeded.
#[derive(Debug, Clone)]
pub struct BudgetExceeded {
    /// The agent that exceeded its budget.
    pub agent_id: AgentId,
    /// Which type of budget was exceeded.
    pub kind: BudgetKind,
    /// Human-readable message.
    pub message: String,
}

impl std::fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BudgetExceeded {}

/// Manages budgets for all agents with sliding window reset semantics.
pub struct BudgetManager {
    budgets: RwLock<HashMap<AgentId, BudgetLimit>>,
    usage: RwLock<HashMap<AgentId, Usage>>,
}

impl BudgetManager {
    /// Creates a new empty budget manager.
    pub fn new() -> Self {
        Self {
            budgets: RwLock::new(HashMap::new()),
            usage: RwLock::new(HashMap::new()),
        }
    }

    /// Sets or updates the budget for an agent.
    pub fn set_budget(&self, limit: BudgetLimit) {
        let agent_id = limit.agent_id;
        let now = Utc::now();

        {
            let mut budgets = self.budgets.write();
            budgets.insert(agent_id, limit);
        }

        // Initialize usage for new agents or keep existing if already set
        let mut usage = self.usage.write();
        usage.entry(agent_id).or_insert(Usage {
            tokens_used: 0,
            calls_used: 0,
            window_start: now,
        });
    }

    /// Removes the budget for an agent.
    pub fn remove_budget(&self, agent_id: &AgentId) {
        let mut budgets = self.budgets.write();
        let mut usage = self.usage.write();
        budgets.remove(agent_id);
        usage.remove(agent_id);
    }

    /// Attempts to reserve tokens for an agent.
    ///
    /// Returns `Ok(())` if the tokens can be reserved.
    /// Returns `Err(BudgetExceeded)` if the agent has exceeded its token budget.
    ///
    /// The usage window is automatically reset if it has expired.
    pub fn reserve(&self, agent_id: &AgentId, tokens: u64) -> Result<(), BudgetExceeded> {
        let limit = {
            let budgets = self.budgets.read();
            budgets.get(agent_id).cloned()
        };

        let limit = match limit {
            Some(l) => l,
            None => {
                return Err(BudgetExceeded {
                    agent_id: *agent_id,
                    kind: BudgetKind::Token,
                    message: format!("No budget configured for agent {}", agent_id),
                });
            }
        };

        {
            let mut usage = self.usage.write();
            let usage_entry = usage.entry(*agent_id).or_insert_with(|| Usage {
                tokens_used: 0,
                calls_used: 0,
                window_start: Utc::now(),
            });

            reset_if_expired(usage_entry, limit.window_secs);

            if usage_entry.tokens_used + tokens > limit.token_budget {
                return Err(BudgetExceeded {
                    agent_id: *agent_id,
                    kind: BudgetKind::Token,
                    message: format!(
                        "Token budget exceeded: requested {} but only {} remaining",
                        tokens,
                        limit.token_budget.saturating_sub(usage_entry.tokens_used)
                    ),
                });
            }

            usage_entry.tokens_used += tokens;
        }

        Ok(())
    }

    /// Releases tokens back (e.g., on retry or error).
    ///
    /// Tokens are subtracted from usage. Does not allow negative usage.
    pub fn release(&self, agent_id: &AgentId, tokens_used: u64) {
        let mut usage = self.usage.write();
        if let Some(entry) = usage.get_mut(agent_id) {
            entry.tokens_used = entry.tokens_used.saturating_sub(tokens_used);
        }
    }

    /// Tracks a call for an agent.
    ///
    /// Returns `Err(BudgetExceeded)` if the call limit has been exceeded.
    pub fn track_call(&self, agent_id: &AgentId) -> Result<(), BudgetExceeded> {
        let limit = {
            let budgets = self.budgets.read();
            budgets.get(agent_id).cloned()
        };

        let limit = match limit {
            Some(l) => l,
            None => {
                return Err(BudgetExceeded {
                    agent_id: *agent_id,
                    kind: BudgetKind::Call,
                    message: format!("No budget configured for agent {}", agent_id),
                });
            }
        };

        {
            let mut usage = self.usage.write();
            let usage_entry = usage.entry(*agent_id).or_insert_with(|| Usage {
                tokens_used: 0,
                calls_used: 0,
                window_start: Utc::now(),
            });

            reset_if_expired(usage_entry, limit.window_secs);

            if usage_entry.calls_used >= limit.calls_budget {
                return Err(BudgetExceeded {
                    agent_id: *agent_id,
                    kind: BudgetKind::Call,
                    message: format!(
                        "Call budget exceeded: {} calls used, limit is {}",
                        usage_entry.calls_used, limit.calls_budget
                    ),
                });
            }

            usage_entry.calls_used += 1;
        }

        Ok(())
    }

    /// Returns current budget information for an agent.
    pub fn remaining(&self, agent_id: &AgentId) -> BudgetInfo {
        let limit = {
            let budgets = self.budgets.read();
            budgets.get(agent_id).cloned()
        };

        match limit {
            Some(limit) => {
                let usage = self.usage.read();
                let usage_entry = usage.get(agent_id);

                if let Some(entry) = usage_entry {
                    let elapsed = Utc::now()
                        .signed_duration_since(entry.window_start)
                        .to_std()
                        .unwrap_or(Duration::ZERO);
                    let window_remaining = Duration::from_secs(limit.window_secs)
                        .saturating_sub(elapsed)
                        .as_secs();

                    let tokens_remaining = limit.token_budget.saturating_sub(entry.tokens_used);
                    let calls_remaining = limit.calls_budget.saturating_sub(entry.calls_used);
                    let is_exhausted = tokens_remaining == 0 || calls_remaining == 0;

                    BudgetInfo {
                        tokens_remaining,
                        calls_remaining,
                        window_remaining_secs: window_remaining,
                        is_exhausted,
                    }
                } else {
                    BudgetInfo {
                        tokens_remaining: limit.token_budget,
                        calls_remaining: limit.calls_budget,
                        window_remaining_secs: limit.window_secs,
                        is_exhausted: false,
                    }
                }
            }
            None => BudgetInfo {
                tokens_remaining: 0,
                calls_remaining: 0,
                window_remaining_secs: 0,
                is_exhausted: true,
            },
        }
    }

    /// Returns `true` if an agent can be scheduled (has budget remaining).
    pub fn can_schedule(&self, agent_id: &AgentId) -> bool {
        !self.remaining(agent_id).is_exhausted
    }

    /// Manually resets the usage window for an agent.
    pub fn reset_window(&self, agent_id: &AgentId) {
        let mut usage = self.usage.write();
        if let Some(entry) = usage.get_mut(agent_id) {
            entry.tokens_used = 0;
            entry.calls_used = 0;
            entry.window_start = Utc::now();
        }
    }

    /// Returns full budget information including limits and usage for an agent.
    ///
    /// Returns `None` if no budget is configured for the agent.
    pub fn full_info(&self, agent_id: &AgentId) -> Option<FullBudgetInfo> {
        let limit = self.budgets.read().get(agent_id).cloned()?;

        let usage = self.usage.read().get(agent_id).cloned();
        let (tokens_used, calls_used, window_remaining_secs) = if let Some(entry) = usage {
            let elapsed = Utc::now()
                .signed_duration_since(entry.window_start)
                .to_std()
                .unwrap_or(Duration::ZERO);
            let window_duration = Duration::from_secs(limit.window_secs);
            let window_remaining = window_duration.saturating_sub(elapsed).as_secs();
            let elapsed_secs = elapsed.as_secs();

            if window_remaining == 0 && elapsed_secs >= limit.window_secs {
                (0u64, 0u64, 0u64)
            } else {
                (entry.tokens_used, entry.calls_used, window_remaining)
            }
        } else {
            (0u64, 0u64, limit.window_secs)
        };

        let tokens_remaining = limit.token_budget.saturating_sub(tokens_used);
        let calls_remaining = limit.calls_budget.saturating_sub(calls_used);
        let is_exhausted = tokens_remaining == 0 || calls_remaining == 0;

        Some(FullBudgetInfo {
            agent_id: *agent_id,
            token_limit: limit.token_budget,
            tokens_used,
            tokens_remaining,
            calls_limit: limit.calls_budget,
            calls_used,
            calls_remaining,
            window_secs: limit.window_secs,
            window_remaining_secs,
            is_exhausted,
        })
    }

    /// Returns full budget info for all agents with configured budgets.
    pub fn all_full_info(&self) -> Vec<FullBudgetInfo> {
        let budgets = self.budgets.read();
        budgets.keys().filter_map(|id| self.full_info(id)).collect()
    }

    /// Persist budgets and usage to a JSON file at the given path.
    pub fn persist(&self, path: &Path) -> anyhow::Result<()> {
        let budgets = self.budgets.read();
        let usage = self.usage.read();
        let data = PersistedBudgets {
            budgets: budgets.clone(),
            usage: usage.clone(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&data)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Restore budgets and usage from a JSON file at the given path.
    ///
    /// Returns `Ok(())` if the file doesn't exist (empty state).
    /// Returns an error if the file exists but cannot be parsed.
    pub fn restore(&self, path: &Path) -> anyhow::Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let json = std::fs::read_to_string(path)?;
        let data: PersistedBudgets = serde_json::from_str(&json)?;
        {
            let mut budgets = self.budgets.write();
            *budgets = data.budgets;
        }
        {
            let mut usage = self.usage.write();
            *usage = data.usage;
        }
        Ok(())
    }
}

/// Intermediate struct for JSON persistence.
#[derive(Serialize, Deserialize)]
struct PersistedBudgets {
    budgets: HashMap<AgentId, BudgetLimit>,
    usage: HashMap<AgentId, Usage>,
}

/// Resets the usage window if it has expired (sliding window semantics).
fn reset_if_expired(usage: &mut Usage, window_secs: u64) {
    let window_duration = chrono::Duration::seconds(window_secs as i64);
    let elapsed = Utc::now().signed_duration_since(usage.window_start);
    if elapsed >= window_duration {
        usage.tokens_used = 0;
        usage.calls_used = 0;
        usage.window_start = Utc::now();
    }
}

impl Default for BudgetManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn test_agent_id() -> AgentId {
        uuid::Uuid::new_v4()
    }

    #[test]
    fn test_budget_creation() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        let limit = BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 10,
            window_secs: 60,
        };

        manager.set_budget(limit.clone());

        let info = manager.remaining(&agent_id);
        assert_eq!(info.tokens_remaining, 1000);
        assert_eq!(info.calls_remaining, 10);
        assert!(!info.is_exhausted);
    }

    #[test]
    fn test_reserve_success() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        let limit = BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 10,
            window_secs: 60,
        };

        manager.set_budget(limit);

        // Reserve within budget
        let result = manager.reserve(&agent_id, 500);
        assert!(result.is_ok());

        let info = manager.remaining(&agent_id);
        assert_eq!(info.tokens_remaining, 500);
    }

    #[test]
    fn test_exhaust_tokens() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        let limit = BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 10,
            window_secs: 60,
        };

        manager.set_budget(limit);

        // Exhaust token budget
        let result = manager.reserve(&agent_id, 1000);
        assert!(result.is_ok());

        // Try to reserve more
        let result = manager.reserve(&agent_id, 1);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.agent_id, agent_id);
        assert_eq!(err.kind, BudgetKind::Token);
    }

    #[test]
    fn test_exhaust_calls() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        let limit = BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 3,
            window_secs: 60,
        };

        manager.set_budget(limit);

        // Exhaust call budget
        assert!(manager.track_call(&agent_id).is_ok());
        assert!(manager.track_call(&agent_id).is_ok());
        assert!(manager.track_call(&agent_id).is_ok());

        // Try one more call
        let result = manager.track_call(&agent_id);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.agent_id, agent_id);
        assert_eq!(err.kind, BudgetKind::Call);
    }

    #[test]
    fn test_window_reset() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        // Use a very short window for testing
        let limit = BudgetLimit {
            agent_id,
            token_budget: 100,
            calls_budget: 5,
            window_secs: 1,
        };

        manager.set_budget(limit);

        // Exhaust budget
        manager.reserve(&agent_id, 100).unwrap();
        assert!(manager.reserve(&agent_id, 1).is_err());

        // Wait for window to expire
        thread::sleep(Duration::from_secs(2));

        // Should be able to reserve again
        let result = manager.reserve(&agent_id, 50);
        assert!(result.is_ok());

        let info = manager.remaining(&agent_id);
        assert_eq!(info.tokens_remaining, 50);
    }

    #[test]
    fn test_can_schedule() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        let limit = BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 10,
            window_secs: 60,
        };

        manager.set_budget(limit);

        // Should be schedulable
        assert!(manager.can_schedule(&agent_id));

        // Exhaust budget
        for _ in 0..10 {
            manager.track_call(&agent_id).unwrap();
        }

        // Should not be schedulable
        assert!(!manager.can_schedule(&agent_id));
    }

    #[test]
    fn test_no_budget_configured() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        // Try to reserve without budget
        let result = manager.reserve(&agent_id, 100);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.message.contains("No budget configured"));

        // Track call without budget
        let result = manager.track_call(&agent_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_budget() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        let limit = BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 10,
            window_secs: 60,
        };

        manager.set_budget(limit);
        manager.reserve(&agent_id, 100).unwrap();

        manager.remove_budget(&agent_id);

        // Should fail after removal
        let result = manager.reserve(&agent_id, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_release_tokens() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        let limit = BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 10,
            window_secs: 60,
        };

        manager.set_budget(limit);
        manager.reserve(&agent_id, 500).unwrap();

        let info_before = manager.remaining(&agent_id);
        assert_eq!(info_before.tokens_remaining, 500);

        // Release some tokens
        manager.release(&agent_id, 200);

        let info_after = manager.remaining(&agent_id);
        assert_eq!(info_after.tokens_remaining, 700);
    }

    #[test]
    fn test_reset_window() {
        let manager = BudgetManager::new();
        let agent_id = test_agent_id();

        let limit = BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 10,
            window_secs: 60,
        };

        manager.set_budget(limit);
        manager.reserve(&agent_id, 500).unwrap();

        let info_before = manager.remaining(&agent_id);
        assert_eq!(info_before.tokens_remaining, 500);

        // Manual reset
        manager.reset_window(&agent_id);

        let info_after = manager.remaining(&agent_id);
        assert_eq!(info_after.tokens_remaining, 1000);
        assert_eq!(info_after.calls_remaining, 10);
    }

    #[test]
    fn test_multiple_agents() {
        let manager = BudgetManager::new();
        let agent1 = test_agent_id();
        let agent2 = test_agent_id();

        manager.set_budget(BudgetLimit {
            agent_id: agent1,
            token_budget: 1000,
            calls_budget: 10,
            window_secs: 60,
        });

        manager.set_budget(BudgetLimit {
            agent_id: agent2,
            token_budget: 500,
            calls_budget: 5,
            window_secs: 60,
        });

        // Reserve for agent1
        manager.reserve(&agent1, 300).unwrap();

        // Reserve for agent2
        manager.reserve(&agent2, 200).unwrap();

        let info1 = manager.remaining(&agent1);
        let info2 = manager.remaining(&agent2);

        assert_eq!(info1.tokens_remaining, 700);
        assert_eq!(info2.tokens_remaining, 300);
    }
}
