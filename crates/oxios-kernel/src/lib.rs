//! Oxios kernel: supervisor, event bus, state store.
//!
//! The kernel is the core of the Oxios Agent OS. Everything passes
//! through here: agent lifecycle, inter-agent communication, and
//! persistent state management.

#![warn(missing_docs)]

// ─── Lifecycle ──────────────────────────────────────────────────────
// Agent 생성, 실행, 종료. OS의 init + process management.
pub mod a2a_circuit_breaker;
pub mod agent_group;
pub mod agent_lifecycle;
pub mod agent_runtime;
pub mod daemon;
pub mod supervisor;

// ─── Orchestration ──────────────────────────────────────────────────
// 작업 조율, 스케줄링, 예산 관리.
pub mod budget;
pub mod circuit_breaker;
pub mod cron;
pub mod orchestrator;
pub mod scheduler;

// ─── Security ───────────────────────────────────────────────────────
// 접근 제어, 인증, 권한, 감사.
pub mod access_manager;
pub mod audit_trail;
pub mod auth;
pub mod capability;
pub mod credential;

// ─── Communication ──────────────────────────────────────────────────
// 이벤트, 메시징, 외부 프로토콜.
pub mod a2a;
pub mod event_bus;
pub mod mcp;

// ─── Intelligence ───────────────────────────────────────────────────
// 메모리, 임베딩, 페르소나, 온보딩.
pub mod embedding;
pub mod memory;
pub mod onboarding;
pub mod persona;
pub mod persona_manager;
pub mod persona_store;

// ─── Tools & Skills ───────────────────────────────────────────────
// 에이전트가 사용하는 도구, 스킬.
pub mod clawhub;
pub mod skill;
pub mod tools;
#[cfg(feature = "wasm-sandbox")]
pub mod wasm_sandbox;

// ─── State & Config ─────────────────────────────────────────────────
// 영속 상태, 설정, 백업, 리소스 모니터링.
pub mod backup;
pub mod config;
pub mod git_layer;
pub mod resource_monitor;
pub mod space;
pub mod state_store;

// ─── Infrastructure ─────────────────────────────────────────────────
// 엔진, 에러, 타입, 메트릭, 텔레메트리.
pub mod engine;
pub mod error;
pub mod metrics;
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
pub use budget::{BudgetExceeded, BudgetInfo, BudgetKind, BudgetLimit, BudgetManager};
pub use circuit_breaker::CircuitBreaker;
pub use cron::{CronJob, CronJobResult, CronJobUpdate, CronScheduler, JobSource};
pub use orchestrator::{AgentRole, OrchestrationResult, Orchestrator, SubTask};
pub use scheduler::{AgentScheduler, Priority, ScheduledTask, SchedulerStats, TaskStatus};

// ─── Security ───────────────────────────────────────────────────────
pub use access_manager::{
    AccessManager, Action, AgentPermissions, ApprovalStatus, PendingApproval, RbacAuditEntry,
    RbacManager, RbacPolicy, Role, Subject,
};
pub use audit_trail::{
    AgentId as AuditAgentId, AuditAction, AuditEntry, AuditError, AuditTrail, HashDigest,
};
pub use auth::{AuthManager, KeyMeta};
pub use capability::template::CapabilityTemplate;
pub use capability::{CSpace, Capability, CapabilityId, Issuer, ResourceRef, Rights};
pub use credential::CredentialStore;

// ─── Communication ──────────────────────────────────────────────────
pub use a2a::{
    A2AMessage, A2AProtocol, A2ARequest, A2AResponse, AgentCard, AgentCardRegistry,
    DelegationHandler, TaskPriority, TaskSpec,
};
pub use event_bus::{EventBus, KernelEvent};
pub use mcp::{
    McpBridge, McpCapabilities, McpServer, McpTool, McpToolCallResult as CallToolResult,
};

// ─── Intelligence ───────────────────────────────────────────────────
pub use embedding::{EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider};
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
    AutoClassifier, chunk_fixed, chunk_paragraphs, content_hash, cosine_similarity_f32,
    l2_normalize_f32, l2_normalize_f64, ChunkConfig, CompactionTree, CurationCandidate,
    CurationReport, DecayEngine, DreamCheckpoint, DreamProcess, DreamReport, HnswIndex,
    HnswMemoryIndex, HistoricalPeriod, MemoryBudget, MemoryEntry, MemoryGraph, MemoryManager,
    MemoryTier, MemoryType, ProtectionLevel, ProactiveRecall, RootEntry, RootIndex,
    SemanticHit, TextChunk, TextVector, TopicEntry,
};

// ─── SQLite Memory (RFC-012) ────────────────────────────────────────
#[cfg(feature = "sqlite-memory")]
pub use memory::database::{bytes_to_f32_slice, f32_slice_to_bytes, MemoryDatabase};
#[cfg(feature = "sqlite-memory")]
pub use memory::search::{Bm25Hit, RankedMemory, VectorHit, reciprocal_rank_fusion};
#[cfg(feature = "sqlite-memory")]
pub use memory::cache::{self as sqlite_cache};
#[cfg(feature = "sqlite-memory")]
pub use memory::migration::{self as sqlite_migration, MigrationReport};
pub use persona::{default_personas, Persona};
pub use persona_manager::PersonaManager;
pub use persona_store::PersonaStore;

// ─── Tools & Skills ────────────────────────────────────────────────
pub use clawhub::{
    ClawHubClient, ClawHubInstaller, ClawHubLockEntry, ClawHubLockfile, ClawHubOrigin,
    ClawHubSearchResult, ClawHubSkillDetail, ClawHubSkillMeta, ClawHubVersion,
    DownloadedArchive, InstallResult, UpdateAvailable, UpdateResult,
};
pub use skill::{
    InstallKind, Requirements, RequirementsCheck, Skill, SkillConfig, SkillEntry,
    SkillFormat, SkillInstallSpec, SkillInvocationPolicy, SkillManager, SkillMeta,
    SkillMetadata, SkillRef, SkillSnapshot, SkillSource, SkillState, SkillStatus,
};
pub use tools::tool_types::{ArgumentDef, ToolDef};
#[cfg(feature = "browser")]
pub use tools::BrowserTool;
pub use tools::{ExecTool, KnowledgeTool};
#[cfg(feature = "wasm-sandbox")]
pub use wasm_sandbox::{ResourceKind, WasmConfig, WasmError, WasmSandbox};

// ─── State & Config ─────────────────────────────────────────────────
pub use backup::{BackupManifest, BackupSection};
pub use config::{
    BrowserConfig, ChannelsConfig, CronConfig, DaemonConfig, EngineConfig, ExecConfig, ExecMode,
    GitConfig, InlineCronJob, LoggingConfig, MarketplaceConfig, McpConfig, McpServerDef,
    MemoryConfig, OrchestratorConfig, OxiosConfig, PersonaConfig, TelegramChannelConfig,
};
pub use git_layer::{CommitInfo, GitLayer, LogEntry};
pub use resource_monitor::{OverloadThreshold, ResourceMonitor, ResourceSnapshot};
pub use space::{
    extract_filesystem_path, match_keywords, ConversationBuffer, ConversationTurn, CrossRefEntry,
    MemoryFlow, PathMatcher, Space, SpaceBridge, SpaceId, SpaceManager, SpaceManagerError,
    SpaceSource,
};
pub use state_store::{AgentResponse, PruneConfig, PruneThrottle, Session, SessionId, SessionSummary, StateStore};

// ─── Infrastructure ─────────────────────────────────────────────────
pub use engine::{EngineProvider, OxiEngineProvider, OxiosEngine};
pub use error::{HttpStatus, KernelError, KernelResult};
pub use metrics::{get_metrics, register_builtin_metrics, registry};
pub use types::{AgentId, AgentInfo, AgentStatus};

// ─── API Surface ────────────────────────────────────────────────────
pub use kernel_handle::KernelHandle;
pub use kernel_handle::MarketplaceApi;
pub use kernel_handle::{
    A2aApi, AgentApi, BrowserApi, CopilotResponse, EngineApi, EngineConfigResponse, ExecApi,
    ExtensionApi, InfraApi, KnowledgeContext, KnowledgeLens, KnowledgeNote, McpApi, MemoryNote,
    ModelInfo, PersonaApi, ProviderInfo, SecurityApi, SpaceApi, StateApi, ValidateKeyResult,
};

// ─── oxi-sdk re-exports ─────────────────────────────────────────────
//
// Only types that are actually USED by kernel modules are re-exported.
// Dead re-exports were removed (see audit 2026-05-16).
//
// When oxi-sdk adds full re-exports of oxi-ai/oxi-agent types,
// we can drop direct oxi-ai/oxi-agent dependencies entirely.
// See ../oxi/docs/proposals/sdk-consumer-requirements.md
pub use oxi_sdk::{
    AgentEvent, AgentLoop, InterAgentMessage, KernelToolContext, KernelToolProvider, MessageBus,
    Model, Oxi, OxiBuilder, Provider, ProviderOptions, StreamOptions,
};
