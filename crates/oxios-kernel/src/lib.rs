//! Oxios kernel: supervisor, event bus, state store.
//!
//! The kernel is the core of the Oxios Agent OS. Everything passes
//! through here: agent lifecycle, inter-agent communication, and
//! persistent state management.

#![allow(missing_docs)]

// ─── Lifecycle ──────────────────────────────────────────────────────
// Agent 생성, 실행, 종료. OS의 init + process management.
pub mod agent_group;
pub mod agent_lifecycle;
pub mod agent_runtime;
pub mod daemon;
pub mod supervisor;

// ─── Orchestration ──────────────────────────────────────────────────
// 작업 조율, 스케줄링, 예산 관리.
pub mod budget;
pub mod cron;
pub mod orchestrator;
pub mod scheduler;

// ─── Security ───────────────────────────────────────────────────────
// 접근 제어, 인증, 권한, 감사.
pub mod access_manager;
pub mod auth;
pub mod capability;
pub mod credential;

// ─── Audit Persistence ───────────────────────────────────────────────
//
// `audit_persistence` wires `oxi_sdk::observability::AuditPersistence`
// to the kernel's filesystem-based `StateStore`. The trail itself
// lives in `oxi_sdk::observability::AuditTrail` and is re-exported
// below — RFC-014 Phase F.
mod audit_persistence;

// ─── Communication ──────────────────────────────────────────────────
// 이벤트, 메시징, 외부 프로토콜, 멀티 에이전트 조정.
pub mod a2a;
pub mod coordination;
pub mod event_bus;
pub mod mcp;

// ─── Intelligence ───────────────────────────────────────────────────
// 메모리, 임베딩, 페르소나, 온보딩.
//
// `embedding` moved to `oxios-memory` in RFC-018 b.2. The kernel
// re-exports its items below for back-compat.
pub mod memory;
pub mod onboarding;
pub mod persona;

// ─── Tools & Skills ───────────────────────────────────────────────
// 에이전트가 사용하는 도구, 스킬.
pub mod skill;
pub mod tools;
#[cfg(feature = "wasm-sandbox")]
pub mod wasm_sandbox;
pub mod workers;

// ─── State & Config ─────────────────────────────────────────────────
// 영속 상태, 설정, 백업, 리소스 모니터링.
pub mod backup;
pub mod config;
pub mod git_layer;
pub mod project;
pub mod resource_monitor;
pub mod session_context;
pub mod state_store;

// ─── Infrastructure ─────────────────────────────────────────────────
// 엔진, 에러, 타입, 메트릭, 텔레메트리, 옵저버빌리티.
pub mod engine;
pub mod error;
pub mod metrics;
pub mod observability;
#[cfg(feature = "otel")]
pub mod telemetry_otel;
pub mod types;
#[cfg(feature = "otel")]
pub use telemetry_otel as telemetry;
#[cfg(not(feature = "otel"))]
pub mod telemetry_stub;
#[cfg(not(feature = "otel"))]
pub use telemetry_stub as telemetry;

// ─── API Surface ────────────────────────────────────────────────────
// 외부에 노출하는 typed facade.
pub mod kernel_handle;

// ─────────────────────────────────────────────────────────────────────
// Re-exports (같은 섹션 순서)
// ─────────────────────────────────────────────────────────────────────

// ─── Lifecycle ──────────────────────────────────────────────────────
pub use agent_group::{OxiosAgentGroup, OxiosAgentGroupStatus, OxiosGroupAgent};
pub use agent_lifecycle::AgentLifecycleManager;
pub use agent_runtime::AgentRuntime;
pub use daemon::{DaemonManager, DaemonStatus};
pub use supervisor::{BasicSupervisor, Supervisor};

// ─── Orchestration ──────────────────────────────────────────────────
pub use budget::{
    BudgetExceeded, BudgetInfo, BudgetKind, BudgetLimit, BudgetManager, FullBudgetInfo,
};
// Circuit breaker — delegates to oxi-sdk
pub use cron::{CronJob, CronJobResult, CronJobUpdate, CronScheduler, JobSource};
pub use orchestrator::{AgentRole, OrchestrationResult, Orchestrator, SubTask};
// CircuitBreaker removed — use oxi_sdk::ProviderCircuitBreaker directly (#11).
pub use scheduler::{AgentScheduler, Priority, ScheduledTask, SchedulerStats, TaskStatus};

// ─── Security ───────────────────────────────────────────────────────
pub use access_manager::{
    AccessManager, Action, AgentPermissions, ApprovalStatus, PendingApproval, RbacAuditEntry,
    RbacManager, RbacPolicy, Role, Subject,
};
// AuditTrail types are re-exported from oxi-sdk (Phase F: removed
// 1134-line duplicate). `AgentId as AuditAgentId` preserves the
// historical kernel-level type alias.
pub use auth::{AuthManager, KeyMeta};
pub use capability::template::CapabilityTemplate;
pub use capability::{CSpace, Capability, CapabilityId, Issuer, ResourceRef, Rights};
pub use credential::CredentialStore;
pub use oxi_sdk::observability::audit_trail::AgentId as AuditAgentId;
pub use oxi_sdk::observability::{
    AuditAction, AuditError, AuditPersistence, AuditTrail, HashDigest, TrailEntry,
};

// ─── Communication ──────────────────────────────────────────────────
pub use a2a::{
    A2ACircuitBreaker, A2AMessage, A2AProtocol, A2ARequest, A2AResponse, AgentCard,
    AgentCardRegistry, CircuitState, DelegationHandler, TaskPriority, TaskSpec,
};
pub use event_bus::{EventBus, KernelEvent};
pub use mcp::{
    McpBridge, McpCapabilities, McpServer, McpTool, McpToolCallResult as CallToolResult,
};

// ─── Intelligence ───────────────────────────────────────────────────
// Embedding types moved to oxios-memory (RFC-018 b.2). Re-exported below
// for back-compat — existing `use oxios_kernel::TfIdfEmbeddingProvider;`
// paths continue to work.

pub use memory::auto_memory_bridge::{
    AutoMemoryBridge, ExportResult, GuidancePattern, ImportResult, InsightCategory, MemoryInsight,
    SyncDirection, SyncResult,
};
// flash_attention + compaction + graph moved to oxios-memory in RFC-018 b.5.
// Re-exported below from oxios_memory for back-compat.
pub use memory::{
    content_hash, DreamCheckpoint, DreamProcess, DreamReport,
};

// ─── Memory core types (extracted to oxios-memory, RFC-018 b.1) ───
// Re-exported here for back-compat — existing `use oxios_kernel::chunk_fixed;`
// and friends continue to work without code changes.
pub use oxios_memory::{
    batch_euclidean_to_poincare, chunk_fixed, chunk_paragraphs, cosine_similarity_f32,
    euclidean_to_poincare, hyperbolic_distance, l2_normalize_f32, l2_normalize_f64, mobius_add,
    mobius_scalar_mul, ChunkConfig, HyperbolicConfig, HyperbolicEmbedding, TextChunk, TextVector,
};
pub use oxios_memory::memory::{
    AutoClassifier, AutoProtector, CompactionTree, CurationCandidate, CurationReport,
    DecayEngine, HnswIndex, HnswMemoryIndex, MemoryBudget, MemoryEntry, MemoryManager, MemoryTier,
    MemoryType, ProtectionLevel, RecallTiming, RootEntry, RootIndex, SemanticHit, SonaEngine,
    TopicEntry,
};
// ─── Embedding providers (RFC-018 b.2) ─────────────────────────────
#[cfg(feature = "embedding-gguf")]
pub use oxios_memory::memory::embedding::gguf::{EmbeddingDimension, GgufEmbeddingProvider, GgufModelLoader};
pub use oxios_memory::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};

// ─── SQLite Memory (RFC-012) ────────────────────────────────────────
// All SQLite types moved to oxios-memory in RFC-018 b.7.
#[cfg(feature = "sqlite-memory")]
pub use oxios_memory::memory::cache as sqlite_cache;
#[cfg(feature = "sqlite-memory")]
pub use oxios_memory::memory::database::{bytes_to_f32_slice, f32_slice_to_bytes, MemoryDatabase};
#[cfg(feature = "sqlite-memory")]
pub use oxios_memory::memory::migration::{self as sqlite_migration, MigrationReport};
#[cfg(feature = "sqlite-memory")]
pub use oxios_memory::memory::search::{reciprocal_rank_fusion, Bm25Hit, RankedMemory, VectorHit};
#[cfg(feature = "sqlite-memory")]
pub use oxios_memory::memory::sqlite_store::SqliteMemoryStore;
pub use persona::{default_personas, Persona, PersonaManager, PersonaStore};

// ─── Tools & Skills ────────────────────────────────────────────────
pub use skill::clawhub::{
    ClawHubClient, ClawHubInstaller, ClawHubLockEntry, ClawHubLockfile, ClawHubOrigin,
    ClawHubSearchResult, ClawHubSkillDetail, ClawHubSkillMeta, ClawHubVersion, DownloadedArchive,
    InstallResult, UpdateAvailable, UpdateResult,
};
pub use skill::skills_sh::{
    SkillsShAuditEntry, SkillsShAuditResponse, SkillsShClient, SkillsShFile, SkillsShInstallResult,
    SkillsShInstaller, SkillsShOrigin, SkillsShSearchResponse, SkillsShSkill, SkillsShSkillDetail,
};
pub use skill::{
    InstallKind, Requirements, RequirementsCheck, Skill, SkillConfig, SkillEntry, SkillFormat,
    SkillInstallSpec, SkillInvocationPolicy, SkillManager, SkillMeta, SkillMetadata, SkillRef,
    SkillSnapshot, SkillSource, SkillState, SkillStatus,
};
pub use tools::tool_types::{ArgumentDef, ToolDef};
pub use tools::{ExecTool, KnowledgeTool};
#[cfg(feature = "wasm-sandbox")]
pub use wasm_sandbox::{ResourceKind, WasmConfig, WasmError, WasmSandbox};

// ─── State & Config ─────────────────────────────────────────────────
pub use backup::{BackupManifest, BackupSection};
pub use config::{
    BrowserConfig, ChannelsConfig, CronConfig, DaemonConfig, EmbeddingConfig, EngineConfig,
    ExecConfig, ExecMode, GitConfig, InlineCronJob, LoggingConfig, MarketplaceConfig, McpConfig,
    McpServerDef, MemoryConfig, OrchestratorConfig, OxiosConfig, PersonaConfig, SkillsShConfig,
    SqliteMemoryConfig,
};
pub use git_layer::{
    CommitContext, CommitDiff, CommitInfo, DiffKind, DiffStats, FileDiff, GitLayer, LogEntry,
};
pub use project::{
    detect_project, extract_path, find_by_id, find_by_name, ConversationBuffer, ConversationTurn,
    DetectionResult, Project, ProjectId, ProjectSource,
};
#[cfg(feature = "sqlite-memory")]
pub use project::{ProjectManager, ProjectManagerError};
pub use resource_monitor::{OverloadThreshold, ResourceMonitor, ResourceSnapshot};
pub use state_store::{
    AgentResponse, PruneConfig, PruneThrottle, Session, SessionId, SessionSummary, StateStore,
};

// ─── Infrastructure ─────────────────────────────────────────────────
pub use engine::{EngineHandle, EngineProvider, OxiosEngine};
pub use error::{HttpStatus, KernelError, KernelResult};
pub use metrics::{get_metrics, register_builtin_metrics, registry};
pub use observability::{
    audit_log, cost_tracker, tracer, AuditEntry as SdkAuditEntry, AuditFilter, CostSnapshot,
    CostTracker, Span, SpanGuard, SpanKind, TokenUsage, Tracer as SdkTracer,
};
pub use types::{AgentId, AgentInfo, AgentStatus};

// ─── API Surface ────────────────────────────────────────────────────
pub use kernel_handle::KernelHandle;
pub use kernel_handle::MarketplaceApi;
pub use kernel_handle::{
    A2aApi, AgentApi, CopilotResponse, EngineApi, EngineConfigResponse, ExecApi, ExtensionApi,
    FallbackEvent, InfraApi, InputModality as EngineInputModality, KnowledgeContext, KnowledgeLens,
    KnowledgeNote, McpApi, MemoryNote, ModelInfo, PersonaApi, ProjectApi, ProjectInfo,
    ProviderCategory, ProviderInfo, RoutingConfigSnapshot, RoutingStats, RoutingStatsSnapshot,
    RoutingUpdate, SecurityApi, SharedExecConfig, StateApi, ValidateKeyResult,
};
pub use session_context::SessionContext;

// ─── oxi-sdk re-exports ─────────────────────────────────────────────
//
// Removed: dead re-exports (#11). Consumers should depend on oxi-sdk
// directly and use `oxi_sdk::` instead of going through oxios-kernel.
