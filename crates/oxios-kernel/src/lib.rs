//! Oxios kernel: supervisor, event bus, state store.
//!
//! The kernel is the core of the Oxios Agent OS. Everything passes
//! through here: agent lifecycle, inter-agent communication, and
//! persistent state management.

#![warn(missing_docs)]

pub mod backup;
pub mod budget;
pub mod capability;
pub mod circuit_breaker;
pub mod metrics;
pub mod a2a;
pub mod access_manager;
pub mod agent_group;
pub mod agent_lifecycle;
pub mod agent_runtime;
pub mod audit_trail;

pub mod auth;
pub mod config;
pub mod credential;
pub mod daemon;
pub mod embedding;
pub mod engine;
pub mod error;
pub mod event_bus;
pub mod cron;
pub mod git_layer;
#[cfg(feature = "wasm-sandbox")]
pub mod wasm_sandbox;
pub mod host_tools;
pub mod memory;
pub mod mcp;
pub mod onboarding;
pub mod orchestrator;
pub mod persona;
pub mod persona_manager;
pub mod persona_store;
pub mod program;
pub mod resource_monitor;
pub mod scheduler;
pub mod space;
pub mod skill;
pub mod state_store;
pub mod supervisor;
pub mod tools;
pub mod types;

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
pub use kernel_handle::{StateApi, AgentApi, SecurityApi, PersonaApi, ExtensionApi, McpApi, InfraApi, SpaceApi, ExecApi, BrowserApi, A2aApi};

pub use circuit_breaker::CircuitBreaker;
pub use metrics::{registry, register_builtin_metrics, get_metrics};

pub use a2a::{
    A2AMessage, A2AProtocol, A2ARequest, A2AResponse,
    AgentCard, AgentCardRegistry, TaskPriority, TaskSpec,
    DelegationHandler,
};

// Access Manager exports (includes RBAC)
pub use access_manager::{
    AccessManager, AgentPermissions,
    RbacManager, RbacPolicy, RbacAuditEntry,
    Role, Subject, Action,
    PendingApproval, ApprovalStatus,
};

pub use agent_group::{OxiosAgentGroup, OxiosAgentGroupStatus, OxiosGroupAgent};
pub use agent_lifecycle::AgentLifecycleManager;
pub use agent_runtime::AgentRuntime;
pub use error::{HttpStatus, KernelError, KernelResult};
pub use engine::{OxiosEngine, OxiEngineProvider, EngineProvider};
pub use config::{EngineConfig, DaemonConfig, ExecConfig, OxiosConfig, MemoryConfig, PersonaConfig, McpConfig, McpServerDef, CronConfig, InlineCronJob, GitConfig, ChannelsConfig, TelegramChannelConfig, BrowserConfig};

// Auth manager exports
pub use auth::{AuthManager, KeyMeta};
pub use event_bus::{EventBus, KernelEvent};
pub use orchestrator::{OrchestrationResult, Orchestrator, SubTask, AgentRole};
pub use scheduler::{AgentScheduler, Priority, ScheduledTask, SchedulerStats, TaskStatus};
pub use cron::{CronScheduler, CronJob, CronJobResult, CronJobUpdate, JobSource};
pub use host_tools::{common as host_tools_common, HostToolStatus, HostToolValidator};
pub use mcp::{McpBridge, McpCapabilities, McpServer, McpTool, McpToolCallResult as CallToolResult};
pub use embedding::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};
pub use memory::{
    MemoryEntry, MemoryManager, MemoryType, TextVector,
    MemoryBudget, CurationReport, CurationCandidate, content_hash,
    HnswIndex, MemoryGraph, ChunkConfig, TextChunk,
    chunk_fixed, chunk_paragraphs, l2_normalize_f32, l2_normalize_f64, cosine_similarity_f32,
    HnswMemoryIndex, SemanticHit,
};
pub use memory::auto_memory_bridge::{
    AutoMemoryBridge, SyncDirection, MemoryInsight, InsightCategory,
    ImportResult, ExportResult, SyncResult, GuidancePattern,
};
pub use memory::flash_attention::{
    FlashAttention, FlashAttentionConfig, BenchmarkResult as AttentionBenchmarkResult,
    MemoryEstimate,
};
pub use memory::hyperbolic::{
    HyperbolicEmbedding, HyperbolicConfig,
    euclidean_to_poincare, hyperbolic_distance, mobius_add, mobius_scalar_mul,
    batch_euclidean_to_poincare,
};
pub use program::{InstallSource, Program, ProgramManager, ProgramMeta, ToolDef, HostRequirementsCheck, ArgumentDef};
pub use skill::{Skill, SkillMeta, SkillStore};
pub use state_store::{AgentResponse, Session, SessionId, SessionSummary, StateStore};

// Space exports
pub use space::{
    Space, SpaceId, SpaceSource, SpaceManager, SpaceManagerError,
    ConversationBuffer, ConversationTurn,
    KnowledgeBridge, KnowledgeFlow, CrossRefEntry,
    extract_filesystem_path, match_keywords, PathMatcher,
};

#[cfg(feature = "wasm-sandbox")]
pub use wasm_sandbox::{WasmSandbox, WasmConfig, WasmError, ResourceKind};

pub use persona::{default_personas, Persona};
pub use persona_manager::PersonaManager;
pub use persona_store::PersonaStore;
pub use supervisor::{BasicSupervisor, Supervisor};
pub use tools::{ExecTool, ProgramTool};
#[cfg(feature = "browser")]
pub use tools::BrowserTool;
pub use types::{AgentId, AgentInfo, AgentStatus};

pub use backup::{BackupManifest, BackupSection};
pub use audit_trail::{
    AuditTrail, AuditEntry, AuditAction, AuditError, HashDigest,
    AgentId as AuditAgentId,
};

pub use git_layer::{GitLayer, CommitInfo, LogEntry};

// Budget manager exports
pub use budget::{BudgetExceeded, BudgetInfo, BudgetKind, BudgetLimit, BudgetManager};
pub use resource_monitor::{OverloadThreshold, ResourceMonitor, ResourceSnapshot};

// Capability system exports
pub use capability::{
    Capability, CapabilityId, CSpace, Issuer, ResourceRef, Rights,
};
pub use capability::template::CapabilityTemplate;
pub use credential::CredentialStore;
pub use daemon::{DaemonManager, DaemonStatus};
// Re-export oxi-sdk types we want available in the kernel.
// Note: oxios has its own `AgentGroup` (struct for orchestration state)
// but we also re-export oxi_sdk's AgentGroup for SDK usage.
pub use oxi_sdk::{
    MessageBus, InterAgentMessage,
    AgentMetrics as SdkAgentMetrics, MetricsSnapshot as SdkMetricsSnapshot,
    ProviderPool, RateLimitPolicy,
    Provider, ProviderRegistry, Model, StreamOptions,
    Agent, AgentLoop, AgentConfig, AgentEvent,
    StructuredOutput, OutputMode,
    KernelToolProvider, KernelToolContext,
    Oxi, OxiBuilder, AgentBuilder,
    AgentGroup as SdkAgentGroup,
    GroupResult as SdkGroupResult,
    GroupStrategy as SdkGroupStrategy,
    AgentGroupOutput as SdkAgentGroupOutput,
};

