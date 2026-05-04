//! Oxios kernel: supervisor, event bus, state store, container, garden.
//!
//! The kernel is the core of the Oxios Agent OS. Everything passes
//! through here: agent lifecycle, inter-agent communication, container
//! management, and persistent state management.

#![warn(missing_docs)]

pub mod a2a;
pub mod access_manager;
pub mod agent_runtime;
pub mod argo;
pub mod config;
pub mod container;
pub mod context_manager;
pub mod engine;
pub mod event_bus;
pub mod garden;
pub mod host_exec;
pub mod host_tools;
pub mod mcp;
pub mod orchestrator;
pub mod persona;
pub mod persona_manager;
pub mod persona_store;
pub mod program;
pub mod scheduler;
pub mod skill;
pub mod state_store;
pub mod supervisor;
pub mod types;

// A2A protocol exports
pub use a2a::{
    A2AMessage, A2AProtocol, A2ARequest, A2AResponse,
    AgentCard, AgentCardRegistry, TaskPriority, TaskSpec,
};

// Access Manager exports (includes RBAC)
pub use access_manager::{
    AccessManager, AgentPermissions, AuditEntry,
    RbacManager, RbacPolicy, RbacAuditEntry,
    Role, Subject, Action,
    PendingApproval, ApprovalStatus,
};

pub use agent_runtime::AgentRuntime;
pub use engine::{EngineProvider, OxiEngineProvider};
pub use config::{OxiosConfig, PersonaConfig, McpConfig, McpServerDef};
pub use container::{
    AppleBackend, ContainerBackend, ContainerStats, ContainerStatus, ExecResult, GardenStartConfig, GardenWorkspaceInfo,
};
pub use context_manager::{ContextManager, ContextStats, ContextTier, ContextEntry};
pub use event_bus::{EventBus, KernelEvent};
pub use garden::{GardenInfo, GardenManager};
pub use host_exec::HostExecBridge;
pub use orchestrator::{OrchestrationResult, Orchestrator};
pub use scheduler::{AgentScheduler, Priority, ScheduledTask, SchedulerStats, TaskStatus};
pub use host_tools::{common as host_tools_common, HostToolStatus, HostToolValidator};
pub use mcp::{McpBridge, McpCapabilities, McpServer, McpTool};
pub use program::{InstallSource, Program, ProgramManager, ProgramMeta, ToolDef, HostRequirementsCheck, ArgumentDef};
pub use skill::{Skill, SkillMeta, SkillStore};
pub use state_store::{AgentResponse, Session, SessionId, SessionSummary, StateStore};
pub use persona::{default_personas, Persona};
pub use persona_manager::PersonaManager;
pub use persona_store::PersonaStore;
pub use supervisor::{BasicSupervisor, Supervisor};
pub use types::{AgentId, AgentInfo, AgentStatus};

// Argo Workflows exports
pub use argo::{ArgoConfig, ArgoWorkflow, WorkflowPhase, WorkflowStatus, WorkflowSummary};