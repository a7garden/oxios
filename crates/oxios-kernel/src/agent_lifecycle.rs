//! Agent lifecycle management — fork, register, run, cleanup.
//!
//! Extracted from Orchestrator to reduce the god-object scope.
//! Handles: fork agent → register A2A → check permissions →
//! submit to scheduler → run → unregister → complete/fail.

use anyhow::{bail, Result};
use std::sync::Arc;

use tokio::time::{timeout, Duration};

use crate::a2a::{A2AProtocol, AgentCard};
use crate::access_manager::AccessManager;
use crate::event_bus::{EventBus, KernelEvent};
use crate::metrics::get_metrics;
use crate::scheduler::{AgentScheduler, Priority, ScheduledTask};
use crate::supervisor::Supervisor;
use crate::types::{AgentId, AgentStatus};
use oxios_ouroboros::{ExecutionResult, Seed};

/// Manages the full lifecycle of a single agent from fork to cleanup.
#[derive(Clone)]
pub struct AgentLifecycleManager {
    supervisor: Arc<dyn Supervisor>,
    scheduler: Arc<AgentScheduler>,
    access_manager: Arc<parking_lot::Mutex<AccessManager>>,
    a2a: Arc<A2AProtocol>,
    event_bus: EventBus,
    /// Maximum execution time in seconds for agent tasks (0 = no limit).
    max_execution_time_secs: u64,
}

impl AgentLifecycleManager {
    /// Create a new lifecycle manager.
    pub fn new(
        supervisor: Arc<dyn Supervisor>,
        scheduler: Arc<AgentScheduler>,
        access_manager: Arc<parking_lot::Mutex<AccessManager>>,
        a2a: Arc<A2AProtocol>,
        event_bus: EventBus,
        max_execution_time_secs: u64,
    ) -> Self {
        Self {
            supervisor,
            scheduler,
            access_manager,
            a2a,
            event_bus,
            max_execution_time_secs,
        }
    }

    /// Fork an agent, register it in A2A and access control, submit to
    /// scheduler, run the seed, then clean up.
    pub async fn spawn_and_run(&self, seed: &Seed, priority: Priority) -> Result<ExecutionResult> {
        // 1. Fork
        let agent_id = self.supervisor.fork(seed).await?;
        let agent_name = format!("agent-{}", agent_id);
        tracing::info!(agent_id = %agent_id, seed_id = %seed.id, "Agent forked");

        // 2. Register A2A card
        let card = self.build_agent_card(agent_id, &agent_name, seed);
        if let Err(e) = self.a2a.registry().register_agent(card).await {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to register A2A card");
        }

        // 2b. Deliver any pending A2A messages to this agent
        if let Err(e) = self.a2a.deliver_pending_messages(agent_id).await {
            tracing::debug!(agent_id = %agent_id, error = %e, "No pending A2A messages");
        }

        // 3. Ensure access permissions
        self.ensure_permissions(&agent_name);

        // 4. Submit and start task
        get_metrics().agents_forked.inc();
        let task =
            ScheduledTask::for_agent(agent_id, format!("Execute seed '{}'", seed.goal), priority);
        let task_id = self.scheduler.submit(task)?;
        self.scheduler.start_task(task_id)?;

        // 5. Run — always cleanup even on failure
        let result = if self.max_execution_time_secs > 0 {
            let exec_timeout = Duration::from_secs(self.max_execution_time_secs);
            match timeout(exec_timeout, self.supervisor.run_with_seed(agent_id, seed)).await {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    tracing::warn!(agent_id = %agent_id, error = %e, "Agent execution failed, cleaning up");
                    self.cleanup_on_failure(agent_id, task_id).await;
                    return Err(e);
                }
                Err(_) => {
                    let secs = exec_timeout.as_secs();
                    tracing::warn!(agent_id = %agent_id, secs, "Agent execution timed out after {}s", secs);
                    self.cleanup_on_failure(agent_id, task_id).await;
                    bail!("Agent execution timed out after {} seconds", secs);
                }
            }
        } else {
            match self.supervisor.run_with_seed(agent_id, seed).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(agent_id = %agent_id, error = %e, "Agent execution failed, cleaning up");
                    self.cleanup_on_failure(agent_id, task_id).await;
                    return Err(e);
                }
            }
        };

        // 6. Cleanup on success
        self.cleanup(agent_id, task_id, &result).await;

        Ok(result)
    }

    /// Kill an agent and clean up all registered state.
    pub async fn terminate(&self, agent_id: AgentId) -> Result<()> {
        if let Err(e) = self.supervisor.kill(agent_id).await {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to kill agent");
        }
        if let Err(e) = self.a2a.registry().unregister_agent(agent_id).await {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to unregister A2A card");
        }
        let _ = self
            .event_bus
            .publish(KernelEvent::AgentStopped { id: agent_id });
        Ok(())
    }

    /// Build an A2A agent card from the seed.
    fn build_agent_card(&self, agent_id: AgentId, agent_name: &str, seed: &Seed) -> AgentCard {
        let goal_lower = seed.goal.to_lowercase();

        let mut card = AgentCard::new(
            agent_id,
            agent_name,
            format!("Agent executing seed: {}", seed.goal),
        )
        .with_capability("execute-seed")
        .with_status(AgentStatus::Starting);

        // Infer capabilities from goal.
        if goal_lower.contains("review") || goal_lower.contains("code") {
            card = card.with_capability("code-review");
        }
        if goal_lower.contains("test") {
            card = card.with_capability("testing");
        }
        if goal_lower.contains("refactor") || goal_lower.contains("improve") {
            card = card.with_capability("refactoring");
        }
        if goal_lower.contains("write")
            || goal_lower.contains("create")
            || goal_lower.contains("implement")
        {
            card = card.with_capability("code-generation");
        }
        if goal_lower.contains("debug") || goal_lower.contains("fix") {
            card = card.with_capability("debugging");
        }

        card
    }

    /// Ensure default tool permissions exist for an agent.
    fn ensure_permissions(&self, agent_name: &str) {
        let mut access = self.access_manager.lock();
        for tool in ["bash", "read", "write", "edit", "grep", "find"] {
            access.can_use_tool(agent_name, tool);
        }
        if access.get_permissions(agent_name).is_none() {
            tracing::warn!(agent = %agent_name, "Agent has no permissions, using default");
            access.get_or_create_permissions(agent_name);
        }
    }

    /// Unregister A2A, complete/fail scheduler task.
    async fn cleanup(&self, agent_id: AgentId, task_id: uuid::Uuid, result: &ExecutionResult) {
        if let Err(e) = self.a2a.registry().unregister_agent(agent_id).await {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to unregister A2A card");
        }
        if result.success {
            let _ = self.scheduler.complete_task(task_id);
        } else {
            let _ = self.scheduler.fail_task(task_id, &result.output);
        }
    }

    /// Reap finished zombie tasks and log the cleanup.
    pub fn reap_zombies(&self) -> Vec<uuid::Uuid> {
        let reaped = self.scheduler.reap_zombies();
        if !reaped.is_empty() {
            tracing::warn!(count = reaped.len(), "Zombie tasks reaped");
            let mut access = self.access_manager.lock();
            for task_id in &reaped {
                access.log_access("scheduler", "zombie_reap", &task_id.to_string(), true, None);
            }
        }
        reaped
    }

    /// Cleanup when agent execution fails (no ExecutionResult available).
    async fn cleanup_on_failure(&self, agent_id: AgentId, task_id: uuid::Uuid) {
        if let Err(e) = self.a2a.registry().unregister_agent(agent_id).await {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to unregister A2A card");
        }
        let _ = self.scheduler.fail_task(task_id, "execution failed");
    }
}
