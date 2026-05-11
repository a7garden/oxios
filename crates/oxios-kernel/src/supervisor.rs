//! Supervisor: agent lifecycle management.
//!
//! The supervisor handles forking, executing, monitoring, and
//! terminating agent instances. It is the "init" of Oxios.
//!
//! When an agent is forked and executed, the supervisor delegates
//! the actual tool-calling loop to the [`AgentRuntime`].

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use oxios_ouroboros::Seed;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::agent_runtime::AgentRuntime;
use crate::event_bus::EventBus;
use crate::resource_monitor::ResourceMonitor;
use crate::types::{AgentId, AgentInfo, AgentStatus};
use oxios_ouroboros::ExecutionResult;

/// Supervisor trait for managing agent lifecycles.
#[async_trait]
pub trait Supervisor: Send + Sync {
    /// Fork a new agent from a seed specification.
    async fn fork(&self, spec: &Seed) -> Result<AgentId>;

    /// Start executing an agent.
    async fn exec(&self, id: AgentId) -> Result<()>;

    /// Fork and execute an agent with its seed, running to completion.
    /// Returns the execution result from the agent runtime.
    async fn run_with_seed(&self, id: AgentId, seed: &Seed) -> Result<ExecutionResult>;

    /// Wait for an agent to complete and return its final status.
    async fn wait(&self, id: AgentId) -> Result<AgentStatus>;

    /// Terminate an agent.
    async fn kill(&self, id: AgentId) -> Result<()>;

    /// List all known agents.
    async fn list(&self) -> Result<Vec<AgentInfo>>;
}

/// Basic in-memory supervisor implementation with AgentRuntime integration.
pub struct BasicSupervisor {
    agents: RwLock<HashMap<AgentId, AgentInfo>>,
    event_bus: EventBus,
    runtime: Arc<AgentRuntime>,
    resource_monitor: Option<Arc<ResourceMonitor>>,
}

impl BasicSupervisor {
    /// Creates a new supervisor with the given event bus and agent runtime.
    pub fn new(event_bus: EventBus, runtime: AgentRuntime) -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            event_bus,
            runtime: Arc::new(runtime),
            resource_monitor: None,
        }
    }

    /// Attach a resource monitor for agent count tracking.
    pub fn set_resource_monitor(&mut self, rm: Arc<ResourceMonitor>) {
        self.resource_monitor = Some(rm);
    }

    /// Update the resource monitor with current active agent count.
    fn update_agent_count(&self) {
        if let Some(ref rm) = self.resource_monitor {
            let count = self.agents.read().len();
            rm.set_active_agents(count);
        }
    }
}

#[async_trait]
impl Supervisor for BasicSupervisor {
    async fn fork(&self, spec: &Seed) -> Result<AgentId> {
        let id = AgentId::new_v4();
        let info = AgentInfo {
            id,
            name: spec.goal.clone(),
            status: AgentStatus::Starting,
            created_at: Utc::now(),
            seed_id: Some(spec.id),
        };

        {
            let mut agents = self.agents.write();
            agents.insert(id, info);
        }

        self.update_agent_count();

        let _ = self.event_bus.publish(crate::event_bus::KernelEvent::AgentCreated {
            id,
            name: spec.goal.clone(),
        });

        tracing::info!(agent_id = %id, "Forked new agent from seed");
        Ok(id)
    }

    async fn exec(&self, id: AgentId) -> Result<()> {
        {
            let mut agents = self.agents.write();
            match agents.get_mut(&id) {
                Some(agent) => {
                    agent.status = AgentStatus::Running;
                }
                None => anyhow::bail!("Agent {id} not found"),
            }
        }

        self.update_agent_count();

        let _ = self
            .event_bus
            .publish(crate::event_bus::KernelEvent::AgentStarted { id });
        tracing::info!(agent_id = %id, "Agent execution started");

        Ok(())
    }

    async fn run_with_seed(&self, id: AgentId, seed: &Seed) -> Result<ExecutionResult> {
        // Mark as running.
        {
            let mut agents = self.agents.write();
            match agents.get_mut(&id) {
                Some(agent) => agent.status = AgentStatus::Running,
                None => anyhow::bail!("Agent {id} not found"),
            }
        }

        let _ = self
            .event_bus
            .publish(crate::event_bus::KernelEvent::AgentStarted { id });

        tracing::info!(agent_id = %id, seed_id = %seed.id, "Running agent task");

        match self.runtime.execute(id, seed).await {
            Ok(result) => {
                tracing::info!(
                    agent_id = %id,
                    success = result.success,
                    steps = result.steps_completed,
                    "Agent task completed"
                );

                {
                    let mut agents = self.agents.write();
                    if let Some(agent) = agents.get_mut(&id) {
                        agent.status = if result.success {
                            AgentStatus::Idle
                        } else {
                            AgentStatus::Failed
                        };
                    }
                }

                let _ = self.event_bus.publish(crate::event_bus::KernelEvent::AgentStopped { id });
                self.update_agent_count();
                Ok(result)
            }
            Err(e) => {
                tracing::error!(agent_id = %id, error = %e, "Agent task failed");

                {
                    let mut agents = self.agents.write();
                    if let Some(agent) = agents.get_mut(&id) {
                        agent.status = AgentStatus::Failed;
                    }
                }

                let _ = self.event_bus.publish(crate::event_bus::KernelEvent::AgentFailed {
                    id,
                    error: e.to_string(),
                });
                self.update_agent_count();

                Ok(ExecutionResult {
                    output: format!("Agent failed: {e}"),
                    steps_completed: 0,
                    success: false,
                })
            }
        }
    }

    async fn wait(&self, id: AgentId) -> Result<AgentStatus> {
        let agents = self.agents.read();
        match agents.get(&id) {
            Some(info) => Ok(info.status),
            None => anyhow::bail!("Agent {id} not found"),
        }
    }

    async fn kill(&self, id: AgentId) -> Result<()> {
        {
            let mut agents = self.agents.write();
            if let Some(agent) = agents.get_mut(&id) {
                agent.status = AgentStatus::Stopped;
            } else {
                anyhow::bail!("Agent {id} not found");
            }
        }

        let _ = self
            .event_bus
            .publish(crate::event_bus::KernelEvent::AgentStopped { id });
        self.update_agent_count();
        tracing::info!(agent_id = %id, "Agent killed");
        Ok(())
    }

    async fn list(&self) -> Result<Vec<AgentInfo>> {
        let agents = self.agents.read();
        Ok(agents.values().cloned().collect())
    }
}
