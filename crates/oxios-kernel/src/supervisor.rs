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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;

use crate::agent_runtime::AgentRuntime;
use crate::event_bus::EventBus;
use crate::resource_monitor::ResourceMonitor;
use crate::types::{AgentId, AgentInfo, AgentStatus};
use oxios_ouroboros::ExecutionResult;

/// Tracks the runtime handles needed to cancel a running agent.
struct AgentHandle {
    /// Flag set on `kill()` to cooperatively signal cancellation.
    cancelled: Arc<AtomicBool>,
    /// The tokio task running the agent execution. Aborted on `kill()`.
    task: JoinHandle<Result<ExecutionResult>>,
}

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
    /// Per-agent cancellation tokens and join handles for task abortion.
    handles: RwLock<HashMap<AgentId, AgentHandle>>,
    event_bus: EventBus,
    runtime: Arc<AgentRuntime>,
    resource_monitor: Option<Arc<ResourceMonitor>>,
}

impl BasicSupervisor {
    /// Creates a new supervisor with the given event bus and agent runtime.
    pub fn new(event_bus: EventBus, runtime: AgentRuntime) -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            handles: RwLock::new(HashMap::new()),
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

        let _ = self
            .event_bus
            .publish(crate::event_bus::KernelEvent::AgentCreated {
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

        // Spawn the execution as a tokio task so we can track and abort it.
        let cancelled = Arc::new(AtomicBool::new(false));
        let runtime = Arc::clone(&self.runtime);
        let seed = seed.clone();
        let cancelled_clone = cancelled.clone();

        let handle: JoinHandle<Result<ExecutionResult>> = tokio::spawn(async move {
            // Check for cancellation before starting.
            if cancelled_clone.load(Ordering::Relaxed) {
                return Ok(ExecutionResult {
                    output: "Agent cancelled before execution".into(),
                    steps_completed: 0,
                    success: false,
                });
            }
            runtime.execute(id, &seed).await
        });

        // Store the handle so kill() can abort the task.
        {
            let mut handles = self.handles.write();
            handles.insert(
                id,
                AgentHandle {
                    cancelled,
                    task: handle,
                },
            );
        }

        // Await the spawned task.
        let result = {
            let agent_handle = {
                let mut handles = self.handles.write();
                handles.remove(&id)
            };
            // Guard is dropped above, safe to await.

            match agent_handle {
                Some(ah) => match ah.task.await {
                    Ok(res) => res,
                    Err(join_err) => {
                        // Task was aborted (e.g. kill()) or panicked.
                        tracing::warn!(agent_id = %id, error = %join_err, "Agent task join error");
                        Ok(ExecutionResult {
                            output: format!("Agent task aborted: {join_err}"),
                            steps_completed: 0,
                            success: false,
                        })
                    }
                },
                None => anyhow::bail!("Agent {id} handle disappeared"),
            }
        };

        match result {
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

                let _ = self
                    .event_bus
                    .publish(crate::event_bus::KernelEvent::AgentStopped { id });
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

                let _ = self
                    .event_bus
                    .publish(crate::event_bus::KernelEvent::AgentFailed {
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
        // Cancel and abort the running task, if any.
        {
            let mut handles = self.handles.write();
            if let Some(agent_handle) = handles.remove(&id) {
                agent_handle.cancelled.store(true, Ordering::Relaxed);
                agent_handle.task.abort();
                tracing::info!(agent_id = %id, "Agent task aborted");
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;
    use crate::types::AgentStatus;
    use async_trait::async_trait;
    use futures::Stream;
    use oxi_sdk::{Context, Model, ProviderError, ProviderEvent, StreamOptions};
    use oxios_ouroboros::Seed;
    use std::pin::Pin;

    /// Minimal mock LLM provider for constructing an AgentRuntime in tests.
    struct MockProvider;

    #[async_trait]
    impl oxi_sdk::Provider for MockProvider {
        async fn stream(
            &self,
            _model: &Model,
            _context: &Context,
            _options: Option<StreamOptions>,
        ) -> Result<Pin<Box<dyn Stream<Item = ProviderEvent> + Send>>, ProviderError> {
            // Return an empty stream — never actually invoked in supervisor lifecycle tests.
            let stream = futures::stream::empty();
            Ok(Box::pin(stream))
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    /// Helper to create a real BasicSupervisor wired to a real EventBus.
    async fn make_supervisor() -> BasicSupervisor {
        let event_bus = EventBus::new(64);
        let provider = Arc::new(MockProvider);

        // Build a mock KernelHandle with temp dirs.
        let tmp = std::env::temp_dir().join(format!("oxios-test-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&tmp);

        let state_store_2 =
            Arc::new(crate::state_store::StateStore::new(tmp.join("state")).expect("state store"));
        let state_store = state_store_2.clone();
        let state_store_for_space = state_store_2.clone();
        let memory_manager = Arc::new({
            let mut mm = crate::memory::MemoryManager::new(state_store.clone());
            mm.set_git_layer(Arc::new(
                crate::git_layer::GitLayer::new(tmp.join("git"), false).expect("git layer"),
            ));
            mm
        });

        let kernel_handle = Arc::new(crate::KernelHandle::new(
            crate::kernel_handle::StateApi::new(state_store),
            crate::kernel_handle::AgentApi::new(
                Arc::new(crate::supervisor::NoOpSupervisor),
                Arc::new(crate::budget::BudgetManager::new()),
                memory_manager.clone(),
                Some(event_bus.clone()),
            ),
            crate::kernel_handle::SecurityApi::new(
                Arc::new(parking_lot::Mutex::new(crate::auth::AuthManager::new())),
                Arc::new(crate::audit_trail::AuditTrail::new(100)),
                Arc::new(parking_lot::Mutex::new(
                    crate::access_manager::AccessManager::new(),
                )),
                Arc::new(
                    crate::state_store::StateStore::new(tmp.join("state2")).expect("state store 2"),
                ),
            ),
            crate::kernel_handle::PersonaApi::new(Arc::new(
                crate::persona_manager::PersonaManager::new(),
            )),
            crate::kernel_handle::ExtensionApi::new(
                Arc::new(crate::program::ProgramManager::new(tmp.join("programs"))),
                Arc::new(crate::skill::SkillStore::new(tmp.join("skills")).expect("skill store")),
                Arc::new(crate::host_tools::HostToolValidator::new(vec![], vec![])),
            ),
            crate::kernel_handle::McpApi::new(Arc::new(crate::mcp::McpBridge::new())),
            crate::kernel_handle::InfraApi::new(
                Arc::new(
                    crate::git_layer::GitLayer::new(tmp.join("git"), false).expect("git layer"),
                ),
                Arc::new(crate::scheduler::AgentScheduler::new(4, 60, 300)),
                Arc::new(crate::cron::CronScheduler::new(
                    Arc::new(
                        crate::state_store::StateStore::new(tmp.join("cron")).expect("cron state"),
                    ),
                    60,
                )),
                Arc::new(crate::resource_monitor::ResourceMonitor::new(60, 100)),
                EventBus::new(64),
                crate::config::OxiosConfig::default(),
                std::time::Instant::now(),
            ),
            crate::kernel_handle::SpaceApi::new(
                Arc::new(
                    crate::space::SpaceManager::new(state_store_for_space, EventBus::new(64))
                        .await
                        .expect("space mgr"),
                ),
                EventBus::new(64),
            ),
            crate::kernel_handle::ExecApi::new(
                Arc::new(crate::config::ExecConfig::default()),
                Arc::new(parking_lot::Mutex::new(
                    crate::access_manager::AccessManager::new(),
                )),
            ),
            crate::kernel_handle::BrowserApi::default(),
            crate::kernel_handle::A2aApi::new(Arc::new(crate::a2a::A2AProtocol::new(
                EventBus::new(64),
            ))),
            Arc::new(oxios_markdown::KnowledgeBase::new(tmp.join("knowledge")).unwrap()),
            Arc::new(crate::kernel_handle::KnowledgeLens::new(
                Arc::new(oxios_markdown::KnowledgeBase::new(tmp.join("knowledge")).unwrap()),
                memory_manager.clone(),
            ).unwrap()),
        ));

        let runtime = AgentRuntime::new(provider, "mock/model", kernel_handle);
        BasicSupervisor::new(event_bus, runtime)
    }

    /// Helper to create a minimal Seed for testing.
    fn make_seed(goal: &str) -> Seed {
        Seed {
            id: uuid::Uuid::new_v4(),
            goal: goal.to_string(),
            constraints: vec![],
            acceptance_criteria: vec![],
            ontology: vec![],
            created_at: chrono::Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
        }
    }

    #[tokio::test]
    async fn test_fork_creates_agent() {
        let supervisor = make_supervisor().await;
        let seed = make_seed("Test agent");

        let id = supervisor.fork(&seed).await.unwrap();

        let agents = supervisor.list().await.unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, id);
        assert_eq!(agents[0].name, "Test agent");
        assert_eq!(agents[0].status, AgentStatus::Starting);
        assert_eq!(agents[0].seed_id, Some(seed.id));
    }

    #[tokio::test]
    async fn test_exec_updates_status_to_running() {
        let supervisor = make_supervisor().await;
        let seed = make_seed("Running agent");

        let id = supervisor.fork(&seed).await.unwrap();
        assert_eq!(supervisor.wait(id).await.unwrap(), AgentStatus::Starting);

        supervisor.exec(id).await.unwrap();
        assert_eq!(supervisor.wait(id).await.unwrap(), AgentStatus::Running);
    }

    #[tokio::test]
    async fn test_kill_sets_stopped() {
        let supervisor = make_supervisor().await;
        let seed = make_seed("Doomed agent");

        let id = supervisor.fork(&seed).await.unwrap();
        supervisor.exec(id).await.unwrap();
        assert_eq!(supervisor.wait(id).await.unwrap(), AgentStatus::Running);

        supervisor.kill(id).await.unwrap();
        assert_eq!(supervisor.wait(id).await.unwrap(), AgentStatus::Stopped);
    }

    #[tokio::test]
    async fn test_kill_unknown_agent_returns_error() {
        let supervisor = make_supervisor().await;
        let unknown_id = uuid::Uuid::new_v4();

        let result = supervisor.kill(unknown_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_list_returns_all_agents() {
        let supervisor = make_supervisor().await;

        let id1 = supervisor.fork(&make_seed("Agent 1")).await.unwrap();
        let id2 = supervisor.fork(&make_seed("Agent 2")).await.unwrap();
        let id3 = supervisor.fork(&make_seed("Agent 3")).await.unwrap();

        let agents = supervisor.list().await.unwrap();
        assert_eq!(agents.len(), 3);

        let ids: std::collections::HashSet<AgentId> = agents.iter().map(|a| a.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
        assert!(ids.contains(&id3));
    }

    #[tokio::test]
    async fn test_exec_unknown_agent_returns_error() {
        let supervisor = make_supervisor().await;
        let unknown_id = uuid::Uuid::new_v4();

        let result = supervisor.exec(unknown_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_wait_unknown_agent_returns_error() {
        let supervisor = make_supervisor().await;
        let unknown_id = uuid::Uuid::new_v4();

        let result = supervisor.wait(unknown_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}

/// A no-op supervisor used during KernelBuilder::build() to break the
/// KernelHandle → AgentRuntime → Supervisor → KernelHandle cycle.
///
/// AgentApi.supervisor is only used for list/kill operations, not during
/// tool registration, so this placeholder is safe during build time.
pub struct NoOpSupervisor;

#[async_trait::async_trait]
impl Supervisor for NoOpSupervisor {
    async fn fork(&self, _spec: &Seed) -> Result<AgentId> {
        Err(anyhow::anyhow!(
            "NoOpSupervisor: fork not available during build"
        ))
    }
    async fn exec(&self, _id: AgentId) -> Result<()> {
        Err(anyhow::anyhow!(
            "NoOpSupervisor: exec not available during build"
        ))
    }
    async fn run_with_seed(&self, _id: AgentId, _seed: &Seed) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "NoOpSupervisor: run_with_seed not available during build"
        ))
    }
    async fn wait(&self, _id: AgentId) -> Result<AgentStatus> {
        Err(anyhow::anyhow!(
            "NoOpSupervisor: wait not available during build"
        ))
    }
    async fn kill(&self, _id: AgentId) -> Result<()> {
        Err(anyhow::anyhow!(
            "NoOpSupervisor: kill not available during build"
        ))
    }
    async fn list(&self) -> Result<Vec<AgentInfo>> {
        Ok(Vec::new())
    }
}
