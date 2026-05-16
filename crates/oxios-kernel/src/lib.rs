//! Oxios kernel: supervisor, event bus, state store.
//!
//! The kernel is the core of the Oxios Agent OS. Everything passes
//! through here: agent lifecycle, inter-agent communication, and
//! persistent state management.

#![warn(missing_docs)]

pub mod a2a;
pub mod access_manager;
pub mod agent_group;
pub mod agent_lifecycle;
pub mod agent_runtime;
pub mod audit_trail;
pub mod backup;
pub mod budget;
pub mod capability;
pub mod circuit_breaker;
pub mod metrics;

pub mod auth;
pub mod config;
pub mod credential;
pub mod cron;
pub mod daemon;
pub mod embedding;
pub mod engine;
pub mod error;
pub mod event_bus;
pub mod git_layer;
pub mod host_tools;
pub mod mcp;
pub mod memory;
pub mod onboarding;
pub mod orchestrator;
pub mod persona;
pub mod persona_manager;
pub mod persona_store;
pub mod program;
pub mod resource_monitor;
pub mod scheduler;
pub mod skill;
pub mod space;
pub mod state_store;
pub mod supervisor;
pub mod tools;
pub mod types;
#[cfg(feature = "wasm-sandbox")]
pub mod wasm_sandbox;

#[cfg(feature = "otel")]
pub mod telemetry_otel;
#[cfg(feature = "otel")]
pub use telemetry_otel as telemetry;

#[cfg(not(feature = "otel"))]
pub mod telemetry_stub;
#[cfg(not(feature = "otel"))]
pub use telemetry_stub as telemetry;

pub mod kernel_handle;
pub use kernel_handle::KernelHandle;
pub use kernel_handle::{
    A2aApi, AgentApi, BrowserApi, ExecApi, ExtensionApi, InfraApi, McpApi, PersonaApi, SecurityApi,
    SpaceApi, StateApi,
};

pub use circuit_breaker::CircuitBreaker;
pub use metrics::{get_metrics, register_builtin_metrics, registry};

pub use a2a::{
    A2AMessage, A2AProtocol, A2ARequest, A2AResponse, AgentCard, AgentCardRegistry,
    DelegationHandler, TaskPriority, TaskSpec,
};

// Access Manager exports (includes RBAC)
pub use access_manager::{
    AccessManager, Action, AgentPermissions, ApprovalStatus, PendingApproval, RbacAuditEntry,
    RbacManager, RbacPolicy, Role, Subject,
};

pub use agent_group::{OxiosAgentGroup, OxiosAgentGroupStatus, OxiosGroupAgent};
pub use agent_lifecycle::AgentLifecycleManager;
pub use agent_runtime::AgentRuntime;
pub use config::{
    BrowserConfig, ChannelsConfig, CronConfig, DaemonConfig, EngineConfig, ExecConfig, GitConfig,
    InlineCronJob, McpConfig, McpServerDef, MemoryConfig, OxiosConfig, PersonaConfig,
    TelegramChannelConfig,
};
pub use engine::{EngineProvider, OxiEngineProvider, OxiosEngine};
pub use error::{HttpStatus, KernelError, KernelResult};

// Auth manager exports
pub use auth::{AuthManager, KeyMeta};
pub use cron::{CronJob, CronJobResult, CronJobUpdate, CronScheduler, JobSource};
pub use embedding::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};
pub use event_bus::{EventBus, KernelEvent};
pub use host_tools::{common as host_tools_common, HostToolStatus, HostToolValidator};
pub use mcp::{
    McpBridge, McpCapabilities, McpServer, McpTool, McpToolCallResult as CallToolResult,
};
pub use memory::auto_memory_bridge::{
    AutoMemoryBridge, ExportResult, GuidancePattern, ImportResult, InsightCategory, MemoryInsight,
    SyncDirection, SyncResult,
};
pub use memory::flash_attention::{
    BenchmarkResult as AttentionBenchmarkResult, FlashAttention, FlashAttentionConfig,
    MemoryEstimate,
};
pub use memory::hyperbolic::{
    batch_euclidean_to_poincare, euclidean_to_poincare, hyperbolic_distance, mobius_add,
    mobius_scalar_mul, HyperbolicConfig, HyperbolicEmbedding,
};
pub use memory::{
    chunk_fixed, chunk_paragraphs, content_hash, cosine_similarity_f32, l2_normalize_f32,
    l2_normalize_f64, ChunkConfig, CurationCandidate, CurationReport, HnswIndex, HnswMemoryIndex,
    MemoryBudget, MemoryEntry, MemoryGraph, MemoryManager, MemoryType, SemanticHit, TextChunk,
    TextVector,
};
pub use orchestrator::{AgentRole, OrchestrationResult, Orchestrator, SubTask};
pub use program::{
    ArgumentDef, HostRequirementsCheck, InstallSource, Program, ProgramManager, ProgramMeta,
    ToolDef,
};
pub use scheduler::{AgentScheduler, Priority, ScheduledTask, SchedulerStats, TaskStatus};
pub use skill::{Skill, SkillMeta, SkillStore};
pub use state_store::{AgentResponse, Session, SessionId, SessionSummary, StateStore};

// Space exports
pub use space::{
    extract_filesystem_path, match_keywords, ConversationBuffer, ConversationTurn, CrossRefEntry,
    KnowledgeBridge, KnowledgeFlow, PathMatcher, Space, SpaceId, SpaceManager, SpaceManagerError,
    SpaceSource,
};

#[cfg(feature = "wasm-sandbox")]
pub use wasm_sandbox::{ResourceKind, WasmConfig, WasmError, WasmSandbox};

pub use persona::{default_personas, Persona};
pub use persona_manager::PersonaManager;
pub use persona_store::PersonaStore;
pub use supervisor::{BasicSupervisor, Supervisor};
#[cfg(feature = "browser")]
pub use tools::BrowserTool;
pub use tools::{ExecTool, ProgramTool};
pub use types::{AgentId, AgentInfo, AgentStatus};

pub use audit_trail::{
    AgentId as AuditAgentId, AuditAction, AuditEntry, AuditError, AuditTrail, HashDigest,
};
pub use backup::{BackupManifest, BackupSection};

pub use git_layer::{CommitInfo, GitLayer, LogEntry};

// Budget manager exports
pub use budget::{BudgetExceeded, BudgetInfo, BudgetKind, BudgetLimit, BudgetManager};
pub use resource_monitor::{OverloadThreshold, ResourceMonitor, ResourceSnapshot};

// Capability system exports
pub use capability::template::CapabilityTemplate;
pub use capability::{CSpace, Capability, CapabilityId, Issuer, ResourceRef, Rights};
pub use credential::CredentialStore;
pub use daemon::{DaemonManager, DaemonStatus};
// ── oxi-sdk re-exports ──────────────────────────────────────────────
//
// Only types that are actually USED by kernel modules are re-exported.
// Dead re-exports were removed (see audit 2026-05-16).
//
// When oxi-sdk adds full re-exports of oxi-ai/oxi-agent types,
// we can drop direct oxi-ai/oxi-agent dependencies entirely.
// See ../oxi/docs/proposals/sdk-consumer-requirements.md
pub use oxi_sdk::{
    AgentEvent,
    // Agent loop (used by agent_runtime.rs — via oxi_sdk re-exports,
    // will switch when oxi-sdk re-exports them)
    AgentLoop,
    InterAgentMessage,
    KernelToolContext,
    // Kernel tool bridge (used by tools/kernel_bridge.rs)
    KernelToolProvider,
    // A2A messaging (used by kernel_handle/a2a_api.rs)
    MessageBus,
    Model,
    Oxi,
    OxiBuilder,
    // Engine (used by engine.rs)
    Provider,
    StreamOptions,
};
