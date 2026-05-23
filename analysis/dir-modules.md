# Oxios-Kernel Directory Module Analysis

> Generated: 2026-05-23
> Scope: All 9 subdirectories under `crates/oxios-kernel/src/`
> Total: ~24,009 lines across 72 `.rs` files

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Directory Reports](#directory-reports)
   - [memory/](#1-memory-6843-loc)
   - [tools/](#2-tools-6394-loc)
   - [access_manager/](#3-access_manager-2128-loc)
   - [space/](#4-space-1948-loc)
   - [kernel_handle/](#5-kernel_handle-1908-loc)
   - [program/](#6-program-1701-loc)
   - [mcp/](#7-mcp-1536-loc)
   - [capability/](#8-capability-961-loc)
   - [workers/](#9-workers-590-loc)
3. [Cross-Cutting Analysis](#cross-cutting-analysis)
4. [Oxios-Web Usage Map](#oxios-web-usage-map)
5. [Extraction Candidates](#extraction-candidates)

---

## Executive Summary

| Directory | LOC | Files | Pub Types | Internal Coupling | External Crates | Extraction Score |
|-----------|-----|-------|-----------|-------------------|-----------------|-----------------|
| `memory/` | 6,843 | 16 | 38 | Low (3 deps) | 7 | ⭐⭐⭐⭐⭐ |
| `tools/` | 6,394 | 19 | 22 | High (12+ deps) | 5 | ⭐ |
| `access_manager/` | 2,128 | 3 | 12 | Very Low (1 dep) | 3 | ⭐⭐⭐⭐⭐ |
| `space/` | 1,948 | 4 | 8 | Medium (4 deps) | 4 | ⭐⭐⭐ |
| `kernel_handle/` | 1,908 | 14 | 18 | Very High (22 deps) | 0 | N/A (Facade) |
| `program/` | 1,701 | 4 | 10 | Very Low (1 dep) | 3 | ⭐⭐⭐⭐⭐ |
| `mcp/` | 1,536 | 3 | 15 | Very Low (1 dep) | 2 | ⭐⭐⭐⭐⭐ |
| `capability/` | 961 | 4 | 7 | Low (2 deps) | 1 | ⭐⭐⭐⭐ |
| `workers/` | 590 | 1 | 8 | Zero (0 deps) | 2 | ⭐⭐⭐⭐⭐ |

**Key findings:**
- **5 modules are strong extraction candidates** (memory, access_manager, program, mcp, workers) — high LOC, low internal coupling, clear API boundary.
- **kernel_handle** is intentionally a facade; it references everything. Not extractable by design.
- **tools** is the most coupled module — it depends on almost every other kernel module. Must remain in-kernel.
- **workers** is the cleanest module — zero crate-internal dependencies, fully self-contained.

---

## Directory Reports

### 1. memory/ (6,843 LOC)

**Purpose:** Agent memory system — persistent storage, semantic search, vector embeddings, learning, and curation.

#### Files (16)

| File | LOC | Role |
|------|-----|------|
| `mod.rs` | 645 | `MemoryManager`, `MemoryEntry`, `MemoryType`, `TextVector`, vector index |
| `store.rs` | 887 | `HnswMemoryIndex`, persistent JSON storage, rebuild, search, recall |
| `auto_memory_bridge.rs` | 883 | `AutoMemoryBridge` — automatic memory capture from conversations |
| `hyperbolic.rs` | 590 | `HyperbolicEmbedding`, `HyperbolicConfig` — Poincaré ball embeddings |
| `flash_attention.rs` | 591 | `FlashAttention`, `FlashAttentionConfig` — attention-weighted retrieval |
| `sona.rs` | 535 | `SonaEngine`, `SonaMode` — learning engine with EWC/trajectory |
| `rvf_store.rs` | 571 | `RvfStore` — reinforcement value function store |
| `reasoning_bank.rs` | 630 | `ReasoningBank` — Chain-of-thought reasoning storage |
| `hnsw.rs` | 317 | `HnswIndex` — HNSW approximate nearest neighbor (via usearch) |
| `graph.rs` | 268 | `MemoryGraph` — relationship graph between memory entries |
| `embedding_cache.rs` | 251 | `EmbeddingCache`, `CacheStats` — LRU embedding cache |
| `chunking.rs` | 262 | `TextChunk`, `ChunkConfig` — text splitting strategies |
| `migrate.rs` | 249 | `MigrationReport`, `MigrationProgress` — memory format migration |
| `normalizer.rs` | 122 | `l2_normalize_f32/f64`, `cosine_similarity_f32` — math utilities |
| `budget.rs` | 42 | `MemoryBudget`, `CurationReport`, `CurationCandidate` — memory budgeting |

#### mod.rs Declarations

```rust
pub mod auto_memory_bridge;
mod budget;
mod chunking;
pub mod embedding_cache;
pub mod flash_attention;
mod graph;
mod hnsw;
pub mod hyperbolic;
pub mod normalizer;
pub(crate) mod store;
```

#### Public Types (38)

**Structs:** `MemoryManager`, `MemoryEntry`, `TextVector`, `MemoryBudget`, `CurationReport`, `CurationCandidate`, `AutoMemoryBridge`, `HnswMemoryIndex`, `HnswIndex`, `HyperbolicEmbedding`, `HyperbolicConfig`, `FlashAttention`, `FlashAttentionConfig`, `SonaEngine`, `ReasoningBank`, `RvfStore`, `RvfStoreStats`, `EmbeddingCache`, `CacheStats`, `MemoryGraph`, `TextChunk`, `ChunkConfig`, `MigrationReport`, `MigrationProgress`, `SemanticHit`, `BenchmarkResult`, `MemoryEstimate`, `MemoryInsight`, `PatternMatch`, `PatternRecord`, `LearnedPattern`, `GuidancePattern`, `EwcState`, `Trajectory`, `TrajectoryRecord`, `TrajectoryStep`, `RoutingResult`, `SyncResult`, `ExportResult`, `ImportResult`

**Enums:** `MemoryType` (5 variants), `SonaMode`, `InsightCategory`, `Verdict`, `SyncDirection`

#### Internal Dependencies (`use crate::`)

| Module | Used In | Purpose |
|--------|---------|---------|
| `embedding` | `mod.rs` | `EmbeddingProvider`, `EmbeddingVector`, `TfIdfEmbeddingProvider` |
| `git_layer` | `mod.rs` | `GitLayer` for version-controlled memory |
| `state_store` | `mod.rs`, `store.rs` | `StateStore` for JSON persistence |

**Coupling: LOW** — Only 3 internal deps. Memory only needs embeddings, git, and state storage.

#### External Crates

`anyhow`, `chrono`, `parking_lot`, `serde`, `uuid`, `lru`, `usearch`

#### State Patterns

- `parking_lot::RwLock<HashMap<String, EmbeddingVector>>` — vector index
- `parking_lot::RwLock<Option<Arc<HnswMemoryIndex>>>` — HNSW index
- `Arc<StateStore>` — shared state storage
- `Arc<dyn EmbeddingProvider>` — trait-object embedding
- `Option<Arc<GitLayer>>` — optional git integration

#### Coupling Assessment

**Self-contained.** Memory has a clear API boundary. It only depends on `StateStore` for persistence and `EmbeddingProvider` for vectors. The sub-modules (hyperbolic, flash_attention, sona, rvf_store, reasoning_bank) are independent subsystems with no cross-dependencies between them.

---

### 2. tools/ (6,394 LOC)

**Purpose:** Agent tool implementations — execution, memory operations, kernel domain tools, browser, MCP, A2A, program, and retrieval.

#### Files (19)

| File | LOC | Role |
|------|-----|------|
| `exec_tool.rs` | 958 | `ExecTool` — shell and structured command execution |
| `retrieval.rs` | 628 | `ToolRetriever`, `ScoredTool`, `ToolEntry` — tool discovery |
| `browser/browser_tool.rs` | 487 | `BrowserTool` — headless browser automation |
| `memory_tools.rs` | 482 | `MemoryReadTool`, `MemorySearchTool`, `MemoryWriteTool` |
| `a2a_tools.rs` | 600 | `A2aDelegateTool`, `A2aSendTool`, `A2aQueryTool` |
| `program_tool.rs` | 229 | `ProgramTool` — program-defined tool routing |
| `mcp_tool.rs` | 201 | `McpToolWrapper` — MCP tool bridge |
| `kernel/knowledge_tool.rs` | 658 | `KnowledgeTool` — knowledge base operations |
| `kernel/space_tool.rs` | 274 | `SpaceTool` — Space management |
| `kernel/agent_tool.rs` | 236 | `KernelAgentTool` — agent lifecycle |
| `kernel/cron_tool.rs` | 255 | `CronTool` — cron job management |
| `kernel/budget_tool.rs` | 223 | `BudgetTool` — budget management |
| `kernel/resource_tool.rs` | 204 | `ResourceTool` — resource monitoring |
| `kernel/persona_tool.rs` | 205 | `PersonaTool` — persona management |
| `kernel/security_tool.rs` | 195 | `SecurityTool` — security/RBAC operations |
| `kernel_bridge.rs` | 194 | `OxiosKernelBridge` — SDK tool registration bridge |
| `registration.rs` | 214 | CSpace-driven tool registration |
| `kernel/mod.rs` | 90 | Kernel domain tool module |
| `browser/mod.rs` | 28 | Browser tool module |
| `mod.rs` | 33 | Module declarations |

#### mod.rs Declarations

```rust
pub mod a2a_tools;
pub mod exec_tool;
pub mod kernel;
pub mod kernel_bridge;
pub mod mcp_tool;
pub mod memory_tools;
pub mod program_tool;
pub mod registration;
pub mod retrieval;
#[cfg(feature = "browser")]
pub mod browser;
```

#### Public Types (22)

**Structs:** `ExecTool`, `ExecResult`, `ToolRetriever`, `ScoredTool`, `ToolEntry`, `BrowserTool`, `MemoryReadTool`, `MemorySearchTool`, `MemoryWriteTool`, `A2aDelegateTool`, `A2aSendTool`, `A2aQueryTool`, `ProgramTool`, `McpToolWrapper`, `OxiosKernelBridge`, `KnowledgeTool`, `SpaceTool`, `KernelAgentTool`, `CronTool`, `BudgetTool`, `ResourceTool`, `PersonaTool`, `SecurityTool`

#### Internal Dependencies

| Module | Tools That Use It |
|--------|------------------|
| `kernel_handle` | All kernel tools (via `KernelHandle`) |
| `memory` | `MemoryReadTool`, `MemorySearchTool`, `MemoryWriteTool` |
| `mcp` | `McpToolWrapper` |
| `program` | `ProgramTool` |
| `a2a` | `A2aDelegateTool`, `A2aSendTool`, `A2aQueryTool` |
| `access_manager` | `ExecTool` |
| `capability` | `registration.rs` |
| `config` | `ExecTool` |
| `types` | Multiple tools (`AgentId`) |
| `event_bus` | Several tools |
| `budget` | `BudgetTool` |
| `cron` | `CronTool` |
| `embedding` | `retrieval.rs` |
| `supervisor` | `KernelAgentTool` |
| `space` | `SpaceTool` |

**Coupling: VERY HIGH** — Tools is the most coupled module. It depends on 12+ internal modules because each tool wraps a different kernel subsystem.

#### External Crates

`oxi_sdk`, `async_trait`, `serde`, `serde_json`, `tokio`, `uuid`, `chrono`, `parking_lot`, `oxios_markdown`

#### State Patterns

- `Arc<KernelHandle>` — shared kernel handle in every tool
- `Arc<AccessManager>` (via Mutex) — in `ExecTool`
- `Arc<SearchCache>` — in registration
- `parking_lot::Mutex<HashMap>` — exec tool process tracking

#### Coupling Assessment

**Tightly coupled.** Tools are the "hands" of the kernel — by nature they must touch every subsystem. Not extractable without massive refactoring. The `OxiosKernelBridge` implements the `oxi_sdk::KernelToolProvider` trait, binding tools to the SDK.

---

### 3. access_manager/ (2,128 LOC)

**Purpose:** Least-privilege security for agents — RBAC, path sandboxing, audit logging, workspace boundaries.

#### Files (3)

| File | LOC | Role |
|------|-----|------|
| `mod.rs` | 1,339 | `AccessManager` — main access control, workspace sandbox, audit |
| `rbac.rs` | 566 | `RbacManager`, `Role`, `Action`, `Subject`, HitL approvals |
| `permissions.rs` | 223 | `AgentPermissions`, `AuditEntry`, `PermissionUpdate` |

#### mod.rs Declarations

```rust
mod permissions;
mod rbac;
pub use permissions::{AgentPermissions, AuditEntry, PermissionUpdate};
pub use rbac::{Action, ApprovalStatus, PendingApproval, RbacAuditEntry, RbacManager, RbacPolicy, Role, Subject};
```

#### Public Types (12)

**Structs:** `AccessManager`, `AgentPermissions`, `AuditEntry`, `PermissionUpdate`, `RbacManager`, `RbacPolicy`, `RbacAuditEntry`, `PendingApproval`

**Enums:** `Role` (3: User/Superuser/Admin), `Action` (8 variants), `Subject` (3: User/Agent/System), `ApprovalStatus` (4: Pending/Approved/Rejected/Expired)

#### Internal Dependencies

| Module | Purpose |
|--------|---------|
| `types` | `AgentId` for subject identification |

**Coupling: VERY LOW** — Only depends on `types::AgentId`.

#### External Crates

`chrono`, `glob`, `serde`

#### State Patterns

- `HashMap<String, AgentPermissions>` — per-agent permissions
- `Vec<AuditEntry>` — bounded audit log
- `tokio::sync::mpsc::channel(1000)` — async audit log persistence
- `RbacManager` — embedded RBAC with policies, role assignments, approvals
- `HashMap<String, PathBuf>` — workspace path registry
- `HashMap<String, String>` — agent → workspace mapping

#### Coupling Assessment

**Highly self-contained.** Clear API boundary. Only needs `AgentId` from types. The RBAC system is fully self-contained with its own policy engine, audit trail, and approval workflow. Could be extracted to `oxios-security` crate.

---

### 4. space/ (1,948 LOC)

**Purpose:** Logical work partitions for context isolation — Space CRUD, conversation buffering, topic detection, cross-Space memory transfer.

#### Files (4)

| File | LOC | Role |
|------|-----|------|
| `manager.rs` | 906 | `SpaceManager`, `SpaceManagerError` — Space CRUD, activation |
| `conversation_buffer.rs` | 364 | `ConversationBuffer`, `ConversationTurn` — rolling conversation |
| `detection.rs` | 416 | `PathMatcher`, `extract_filesystem_path`, `match_keywords` |
| `space_bridge.rs` | 262 | `SpaceBridge`, `CrossRefEntry`, `MemoryFlow` — cross-Space transfer |

Also: `space.rs` (249 LOC, parent file) declares `Space`, `SpaceId`, `SpaceSource`, `SpaceConfig`.

#### mod.rs Declarations (from space.rs)

```rust
pub mod conversation_buffer;
pub mod detection;
pub mod manager;
pub mod space_bridge;
```

#### Public Types (8)

**Structs:** `SpaceManager`, `ConversationBuffer`, `ConversationTurn`, `SpaceBridge`, `CrossRefEntry`, `PathMatcher`, `Topic`

**Enums:** `SpaceManagerError`, `MemoryFlow`

Plus from parent `space.rs`: `Space`, `SpaceId` (type alias = Uuid), `SpaceSource`, `SpaceConfig`

#### Internal Dependencies

| Module | Used In | Purpose |
|--------|---------|---------|
| `state_store` | `manager.rs` | `StateStore` for persistence |
| `event_bus` | `manager.rs` | `EventBus` for Space events |
| `memory` | `space_bridge.rs` | `MemoryManager` for cross-Space transfer |
| `audit_trail` | `manager.rs` | Audit logging for Space ops |

**Coupling: MEDIUM** — Depends on 4 internal modules.

#### External Crates

`anyhow`, `chrono`, `parking_lot`, `serde`, `tokio`

#### State Patterns

- `Arc<StateStore>` — shared persistence
- `parking_lot::RwLock<HashMap<SpaceId, Space>>` — Space registry
- `EventBus` — event publishing on Space lifecycle changes
- `Arc<MemoryManager>` — cross-Space memory bridge

#### Coupling Assessment

**Moderately coupled.** Space management needs state persistence, event bus, and (for cross-Space transfer) memory. The detection subsystem (`detection.rs`) is pure logic with no dependencies. Could potentially be extracted if `StateStore` and `EventBus` were abstracted.

---

### 5. kernel_handle/ (1,908 LOC)

**Purpose:** Kernel facade — 13 domain APIs composing the system call interface. Each API wraps a specific subsystem.

#### Files (14)

| File | LOC | Role |
|------|-----|------|
| `mod.rs` | 323 | `KernelHandle` — facade composing 13 APIs |
| `knowledge_lens.rs` | 383 | `KnowledgeLens` — semantic HNSW overlay on KnowledgeBase |
| `infra_api.rs` | 162 | `InfraApi` — Git, scheduler, cron, resources, events, system |
| `agent_api.rs` | 165 | `AgentApi` — agent lifecycle, budgets, memory |
| `space_api.rs` | 201 | `SpaceApi` — Space management, knowledge flow |
| `state_api.rs` | 112 | `StateApi` — data persistence, sessions |
| `security_api.rs` | 128 | `SecurityApi` — auth, audit trail, RBAC, approvals |
| `extension_api.rs` | 103 | `ExtensionApi` — programs, skills, host tools |
| `knowledge_lens.rs` | 383 | KnowledgeLens — HNSW semantic search overlay |
| `mcp_api.rs` | 80 | `McpApi` — MCP server bridge |
| `persona_api.rs` | 61 | `PersonaApi` — multi-persona management |
| `browser_api.rs` | 114 | `BrowserApi` — browser backend |
| `a2a_api.rs` | 39 | `A2aApi` — agent-to-agent communication |
| `exec_api.rs` | 37 | `ExecApi` — execution config + access management |

#### mod.rs Declarations

```rust
pub mod a2a_api;
pub mod agent_api;
pub mod browser_api;
pub mod exec_api;
pub mod extension_api;
pub mod infra_api;
pub mod knowledge_lens;
pub mod mcp_api;
pub mod persona_api;
pub mod security_api;
pub mod space_api;
pub mod state_api;
```

#### Public Types (18)

**Structs:** `KernelHandle`, `StateApi`, `AgentApi`, `SecurityApi`, `PersonaApi`, `ExtensionApi`, `McpApi`, `InfraApi`, `SpaceApi`, `ExecApi`, `BrowserApi`, `A2aApi`, `KnowledgeLens`, `KnowledgeContext`, `KnowledgeNote`, `MemoryNote`, `CopilotResponse`, `MemoryFlowInfo`, `SpaceInfo`

#### Internal Dependencies (22 modules)

`a2a`, `access_manager`, `audit_trail`, `auth`, `budget`, `config`, `cron`, `event_bus`, `git_layer`, `host_tools`, `mcp`, `memory`, `persona`, `persona_manager`, `program`, `resource_monitor`, `scheduler`, `skill`, `space`, `state_store`, `supervisor`, `types`

**Coupling: MAXIMUM** — By design. KernelHandle is the facade that touches every subsystem. It's the "system call layer" of the OS.

#### External Crates

None directly — all deps are via internal modules. Uses `oxios_markdown` for `KnowledgeBase`.

#### State Patterns

- `Arc<...>` for every subsystem reference
- `parking_lot::Mutex<AccessManager>`, `parking_lot::Mutex<AuthManager>`
- `Arc<oxios_markdown::KnowledgeBase>` — direct markdown knowledge
- `Arc<KnowledgeLens>` — semantic overlay

#### Coupling Assessment

**Not extractable.** This is the unified facade. It exists to provide a single entry point to all kernel operations. Each sub-API (`StateApi`, `AgentApi`, etc.) could potentially be extracted individually, but the `KernelHandle` composite must stay.

---

### 6. program/ (1,701 LOC)

**Purpose:** OS-level installable applications for AI agents — install, uninstall, upgrade, discovery, and skill loading.

#### Files (4)

| File | LOC | Role |
|------|-----|------|
| `mod.rs` | 1,388 | `ProgramManager` — install, uninstall, upgrade, bootstrap |
| `types.rs` | 167 | `Program`, `ProgramMeta`, `ToolDef`, `ArgumentDef`, etc. |
| `parser.rs` | 121 | TOML parsing for `program.toml` |
| `installer.rs` | 25 | `copy_dir_all` — recursive directory copy |

#### mod.rs Declarations

```rust
mod installer;
mod parser;
mod types;
pub use types::{ArgumentDef, HostRequirementsCheck, InstallSource, ...};
```

#### Public Types (10)

**Structs:** `ProgramManager`, `Program`, `ProgramMeta`, `ProgramState`, `ToolDef`, `ArgumentDef`, `McpServerConfig`, `ProgramHostRequirements`, `HostRequirementsCheck`

**Enums:** `InstallSource` (3: Local/Git/Tarball)

#### Internal Dependencies

| Module | Purpose |
|--------|---------|
| `host_tools` | `HostToolValidator` for checking required tools |

**Coupling: VERY LOW** — Only 1 internal dependency.

#### External Crates

`anyhow`, `tokio`, `serde`

#### State Patterns

- `tokio::sync::RwLock<HashMap<String, Program>>` — installed programs cache
- `PathBuf` — programs directory
- Filesystem-based persistence (state.json per program)

#### Coupling Assessment

**Highly self-contained.** The only internal dependency is `host_tools` for checking if required tools exist on the host. Could easily be extracted to an `oxios-program` crate. The `InstallSource` enum and `ToolDef`/`ArgumentDef` types are used by other modules (mcp, capability, tools), so they'd need to move to a shared crate or be re-exported.

---

### 7. mcp/ (1,536 LOC)

**Purpose:** Model Context Protocol integration — stdio-based JSON-RPC 2.0 communication with MCP server processes.

#### Files (3)

| File | LOC | Role |
|------|-----|------|
| `client.rs` | 580 | `McpClient` — process lifecycle, JSON-RPC over stdio |
| `mod.rs` | 554 | `McpBridge` — multi-server manager, tool cache |
| `protocol.rs` | 402 | `McpRequest`, `McpResponse`, `McpTool`, `McpServer`, JSON-RPC types |

#### mod.rs Declarations

```rust
mod client;
mod protocol;
pub use client::McpClient;
pub use protocol::*;
```

#### Public Types (15)

**Structs:** `McpBridge`, `McpClient`, `McpServer`, `McpRequest`, `McpResponse`, `McpError`, `McpTool`, `McpToolCallResult`, `McpToolsResult`, `McpCapabilities`, `ClientInfo`, `ServerInfo`, `InitializeParams`, `InitializeResult`, `MappedResource`

**Enums:** `McpContentBlock`

**Type Aliases:** `McpServerConfig = McpServer`

#### Internal Dependencies

| Module | Purpose |
|--------|---------|
| `program` | `ToolDef` for tool schema conversion |

**Coupling: VERY LOW** — Only 1 internal dependency for `ToolDef` type conversion.

#### External Crates

`anyhow`, `tokio`, `serde`

#### State Patterns

- `parking_lot::RwLock<Vec<McpServer>>` — server configs
- `tokio::sync::RwLock<HashMap<String, Arc<McpClient>>>` — active clients
- `tokio::sync::RwLock<HashMap<String, Vec<ToolDef>>>` — tool cache
- `tokio::process::Command` — child process spawning
- Stdin/stdout pipes for JSON-RPC communication

#### Coupling Assessment

**Highly self-contained.** The MCP protocol is a complete, self-contained implementation. Only depends on `program::ToolDef` for type conversion. Could be extracted to an `oxios-mcp` crate with minimal effort — just need to move or share the `ToolDef` type.

---

### 8. capability/ (961 LOC)

**Purpose:** Capability-based access control — unforgeable tokens encoding authority over resources (inspired by seL4).

#### Files (4)

| File | LOC | Role |
|------|-----|------|
| `types.rs` | 426 | Core types: `Capability`, `CSpace`, `Rights`, `ResourceRef`, `Issuer` |
| `template.rs` | 315 | `CapabilityTemplate` — preset CSpace configurations for agent roles |
| `resolve.rs` | 193 | CSpace resolution from Seed + Config |
| `mod.rs` | 27 | Module declarations and re-exports |

#### mod.rs Declarations

```rust
pub mod resolve;
pub mod template;
pub mod types;
pub use types::{CSpace, Capability, CapabilityId, Issuer, ResourceRef, Rights};
```

#### Public Types (7)

**Structs:** `CSpace`, `Capability`, `CapabilityId`, `CapabilityTemplate`, `Rights`

**Enums:** `ResourceRef` (8+ variants: Exec, Browser, KernelDomain, Program, Space, Agent, Mcp, etc.), `Issuer` (3: System, Seed, Granted)

#### Internal Dependencies

| Module | Purpose |
|--------|---------|
| `types` | `AgentId` |
| `space` | Space-related capability resolution |

**Coupling: LOW** — 2 internal deps.

#### External Crates

`serde`

#### State Patterns

- No internal mutable state. Capabilities are value types.
- `CSpace` is a `Vec<Capability>` — simple collection.
- `Rights` is a bitflag (`u8`).

#### Coupling Assessment

**Mostly self-contained.** The capability system is fundamentally a type system with no runtime state. The `resolve.rs` module needs `space` for Space-specific resolution, which creates a mild coupling. Could be extracted if the `ResourceRef` type were made generic or moved to a shared types crate.

---

### 9. workers/ (590 LOC)

**Purpose:** Background worker management — 12 periodic optimization, analysis, and learning workers.

#### Files (1)

| File | LOC | Role |
|------|-----|------|
| `mod.rs` | 590 | Everything: `WorkerManager`, `WorkerType`, `WorkerConfig`, dispatch |

#### mod.rs Declarations

Self-contained single-file module.

#### Public Types (8)

**Structs:** `WorkerManager`, `WorkerConfig`, `WorkerResult`, `WorkerManagerStatus`

**Enums:** `WorkerType` (12 variants), `WorkerPriority` (4: Critical/High/Normal/Low)

#### Internal Dependencies

**NONE.** Zero `use crate::` references.

#### External Crates

`parking_lot`, `serde`

#### State Patterns

- `parking_lot::RwLock<HashMap<WorkerType, WorkerConfig>>` — configs
- `Arc<RwLock<HashSet<WorkerType>>>` — running workers
- `parking_lot::RwLock<HashMap<WorkerType, WorkerResult>>` — results

#### Coupling Assessment

**Perfectly self-contained.** The worker system has zero internal dependencies. Worker implementations are currently stubs that return summary strings. In production, they would call into memory, code analysis, etc. — but currently this is the cleanest extraction candidate.

---

## Cross-Cutting Analysis

### Dependency Graph (Internal Coupling)

```
kernel_handle ──→ (22 modules: everything)
tools ──→ (12+ modules: kernel_handle, memory, mcp, program, a2a, access_manager, capability, ...)
space ──→ state_store, event_bus, memory, audit_trail
memory ──→ embedding, git_layer, state_store
capability ──→ types, space
access_manager ──→ types
program ──→ host_tools
mcp ──→ program
workers ──→ (nothing)
```

### Shared Type Dependencies

| Type | Defined In | Used By |
|------|-----------|---------|
| `AgentId` | `types.rs` | access_manager, capability, tools, kernel_handle |
| `ToolDef` | `program/types.rs` | mcp, tools |
| `KernelHandle` | `kernel_handle/mod.rs` | tools (all kernel tools), agent_runtime |
| `StateStore` | `state_store.rs` | memory, space, kernel_handle |
| `EventBus` | `event_bus.rs` | space, kernel_handle, tools |
| `MemoryManager` | `memory/mod.rs` | space_bridge, tools, kernel_handle |

### Concurrency Patterns

| Pattern | Where Used |
|---------|-----------|
| `parking_lot::RwLock` | memory (vector index), workers (configs/results), mcp (servers) |
| `tokio::sync::RwLock` | program (installed cache), mcp (clients/cache), space (registry) |
| `parking_lot::Mutex` | kernel_handle (auth, access_manager) |
| `tokio::sync::mpsc` | access_manager (audit log persistence) |
| `Arc<...>` | Everywhere — shared ownership across tools and APIs |

**Notable:** The kernel uses both `parking_lot` and `tokio` locks. `parking_lot::RwLock` is used for short-lived, non-async reads (vector index, configs). `tokio::sync::RwLock` is used for potentially blocking async operations (file I/O in program manager, process management in MCP).

---

## Oxios-Web Usage Map

The `oxios-web` channel (`channels/oxios-web/src/`) imports the following from `oxios_kernel`:

| Import | Source File | Used For |
|--------|-----------|----------|
| `config` | `server.rs` | Configuration loading |
| `KernelHandle` | `server.rs` | Kernel facade |
| `OxiosConfig` | `server.rs` | Main config struct |
| `Persona` | `persona_routes.rs` | Persona API routes |
| `state_store::SessionId` | `routes/events.rs` | SSE event streaming |
| `event_bus::KernelEvent` | `routes/events.rs` | Event types |
| `memory::{MemoryEntry, MemoryType}` | `routes/workspace.rs` | Memory display |
| `access_manager::AuditEntry` | `routes/infra.rs` | Audit log display |
| `metrics::registry` | `routes/infra.rs` | Metrics endpoint |
| `ArgumentDef` | `routes/infra.rs` | Tool argument definitions |
| `budget::BudgetLimit` | `routes/budget_routes.rs` | Budget management UI |
| `types::AgentId` | `routes/budget_routes.rs` | Agent identification |
| `InstallSource` | `routes/resources.rs` | Program install UI |
| `CronJob, Priority` | `routes/cron_jobs.rs` | Cron job management |

**Key observations:**
- Web uses `KernelHandle` as the primary interface (as intended)
- Web directly imports types from `memory`, `access_manager`, `budget` for rendering
- Web does NOT import `tools`, `mcp`, `capability`, or `workers` directly
- The `space` module is accessed through `KernelHandle.spaces`, not directly

---

## Extraction Candidates

Ranked by feasibility (highest first):

### Tier 1: Immediate Extraction (zero/minimal coupling)

| Module | LOC | Internal Deps | Extraction Path | Notes |
|--------|-----|--------------|-----------------|-------|
| **workers/** | 590 | 0 | `oxios-workers` | Perfect extraction. Zero deps. Stub implementations need real wiring. |
| **access_manager/** | 2,128 | 1 (`types::AgentId`) | `oxios-security` | Clean API. Move `AgentId` to a shared `oxios-types` crate. |
| **program/** | 1,701 | 1 (`host_tools`) | `oxios-program` | Move `ToolDef`/`ArgumentDef` to shared types or keep as re-export. |
| **mcp/** | 1,536 | 1 (`program::ToolDef`) | `oxios-mcp` | Complete protocol implementation. Just needs `ToolDef` shared. |

### Tier 2: Feasible with moderate refactoring

| Module | LOC | Internal Deps | Extraction Path | Notes |
|--------|-----|--------------|-----------------|-------|
| **capability/** | 961 | 2 (`types`, `space`) | `oxios-capability` | Pure type system. `resolve.rs` coupling to `space` is mild. |
| **memory/** | 6,843 | 3 (`embedding`, `git_layer`, `state_store`) | `oxios-memory` | Largest module. Very clean internal architecture. Needs abstraction over `StateStore` and `EmbeddingProvider` (already trait-based). |

### Tier 3: Not extractable by design

| Module | LOC | Internal Deps | Reason |
|--------|-----|--------------|--------|
| **kernel_handle/** | 1,908 | 22 | Facade by design. Must reference all subsystems. |
| **tools/** | 6,394 | 12+ | "Hands" of the kernel. Each tool wraps a different subsystem. |
| **space/** | 1,948 | 4 | Moderate coupling to core infrastructure (StateStore, EventBus). |

### Recommended Extraction Order

1. **`workers/` → `oxios-workers`** — Zero effort, proves the pattern
2. **`access_manager/` → `oxios-security`** — Create `oxios-types` for `AgentId`
3. **`mcp/` → `oxios-mcp`** — Move `ToolDef` to `oxios-types`
4. **`program/` → `oxios-program`** — Same `ToolDef` sharing
5. **`capability/` → `oxios-capability`** — After `space` coupling resolved
6. **`memory/` → `oxios-memory`** — Largest prize (6.8K LOC). Needs `StateStore` and `EmbeddingProvider` traits in a shared crate.

### Potential Shared Crates

If extraction proceeds, these shared crates would resolve cross-dependencies:

| Crate | Contains | Used By |
|-------|----------|---------|
| `oxios-types` | `AgentId`, `SpaceId`, `ToolDef`, `ArgumentDef` | All extracted crates |
| `oxios-traits` | `EmbeddingProvider`, `Supervisor` trait | memory, tools |

---

*End of report.*
