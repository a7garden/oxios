# Oxios Memory System Upgrade — HNSW + Neural Learning

> **문서 계층**: 전체 설계 (Architecture-Level)
> **상태**: 2차 초안 (Ruflo v3 분석 후 개정)
> **날짜**: 2026-05-14
> **문서 관계**: `design/memory-subdesign.md` (하위 세부설계)

---

## 1. Ruflo v3 분석 요약

### 1.1 우리와 Ruflo의 격차

Ruflo의 메모리/학습 시스템은 **3개 레이어**로 구성됩니다:

```
┌─────────────────────────────────────────────────────────────┐
│                    Memory System                             │
│  AgentDB + HNSW + Memory Graph + RVF Event Log              │
├─────────────────────────────────────────────────────────────┤
│                  Learning System                              │
│  SONA + ReasoningBank + Flash Attention + Workers          │
├─────────────────────────────────────────────────────────────┤
│                  Embedding System                            │
│  Multi-Provider + Chunking + Hyperbolic + Neural Substrate  │
└─────────────────────────────────────────────────────────────┘
```

우리 현재 상태:

| Ruflo v3 | 우리 (현재) | 우리 (1차 설계) |
|----------|-----------|----------------|
| AgentDB (persistent HNSW) | None | HnswIndex (in-memory) |
| Memory Graph (PageRank) | None | None |
| RVF Learning Store | None | None |
| SONA (self-learning) | None | None |
| ReasoningBank | None | None |
| Flash Attention | None | None |
| Workers (12개 background) | cron (simple) | cron만 |
| Auto-memory Bridge | None | None |
| RVF Event Log | event_bus (JSON) | None |
| Multi-Provider Embedding | TfIdf only | OnnxEngine only |
| Document Chunking | None | None |
| Hyperbolic Embeddings | None | None |

### 1.2 핵심 발견 사항

**1. RVF (Rust Vector Format) — 영구 저장의 새 표준**
- Binary header + JSON lines 형식
- Event log, learning store, embedding cache 모두 같은 형식
- Fast append + rebuild without parsing entire file
- 우리 event_bus.rs를 RVF format으로 migration 가능

**2. Memory Graph — 검색 품질의 비밀이었음**
- Vector similarity만으로는 검색 품질이 제한적
- PageRank + community detection으로 **구조적 중요도** 추가
- "관련 있어 보이지만 인용되지 않은 것"보다 "적게 인용되지만 핵심 참조인 것"을 우선

**3. SONA — Self-Optimizing Neural Architecture**
- Trajectory tracking (과거 행동 기록)
- Pattern distillation (성공 패턴 추출)
- Micro-LoRA adapters (작업별 적응)
- EWC (Elastic Weight Consolidation) — catastrophic forgetting 방지
- <0.05ms adaptation target

**4. Workers System — Hooks의 백그라운드 실행**
- 12개 worker가 자동으로 background에서 학습/최적화
- 우리의 cron module을 workers로 확장 가능

**5. Auto-memory Bridge — Claude Code ↔ AgentDB**
- Claude Code의 auto-memory (markdown files)를 AgentDB로 import
- Bidirectional sync로 두 시스템 통합
- 우리 CLI에서 Claude Code를 사용할 때 synergy 가능

---

## 2. Architecture Overview (개정)

### 2.1 Three-Layer System

```
┌─────────────────────────────────────────────────────────────────┐
│                    Layer 1: Memory                              │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ HnswIndex    │  │ MemoryGraph  │  │ SqliteStore  │       │
│  │ (usearch)    │  │ (PageRank)   │  │ (metadata)  │       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
│         │                  │                  │                │
│         └──────────────────┼──────────────────┘                │
│                            ▼                                   │
│                   ┌────────────────┐                          │
│                   │ MemoryManager  │                          │
│                   └────────────────┘                          │
└──────────────────────────────┬────────────────────────────────┘
                               │
┌──────────────────────────────▼────────────────────────────────┐
│                    Layer 2: Learning                            │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │ ReasoningBank│  │   SONA       │  │   Workers    │         │
│  │ (patterns)   │  │ (neural)     │  │ (background) │         │
│  └──────────────┘  └──────────────┘  └──────────────┘         │
└──────────────────────────────┬────────────────────────────────┘
                               │
┌──────────────────────────────▼────────────────────────────────┐
│                    Layer 3: Embedding                           │
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ EmbedEngine  │  │  Chunking    │  │ Normalizer   │        │
│  │ (ONNX)       │  │ (长文档)      │  │ (L2/FP16)    │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Component Map (Our System ↔ Ruflo)

| Our Module | Ruflo Equivalent | Gap |
|-----------|------------------|-----|
| `memory/mod.rs` | `memory/index.ts` | +Graph, +RVF Store |
| `embedding.rs` | `embeddings/index.ts` | +MultiProvider, +Chunking, +Hyperbolic |
| (none) | `neural/sona-integration.ts` | **Missing** |
| (none) | `hooks/reasoningbank/` | **Missing** |
| (none) | `hooks/workers/` | **Missing** |
| (none) | `memory/memory-graph.ts` | **Missing** |
| (none) | `memory/rvf-learning-store.ts` | **Missing** |
| `event_bus.rs` | `rvf-event-log.ts` | Similar concept, different format |
| `cron.rs` | `workers/` | Need extension |

### 2.3 Data Flow (Complete)

```
[Store Flow — Enhanced]
User input
    │
    ▼
ChunkingService.chunk(text) → Vec<Chunk>  (if long document)
    │
    ▼
EmbeddingEngine.embed(chunks) → Vec<DenseVector>
    │
    ├──► HnswIndex.insert()     → vector_index.usearch
    ├──► MemoryGraph.addNode()   → graph structure (PageRank)
    └──► SqliteStore.insert()   → memory.sqlite (metadata + chunks)

[Recall Flow — Graph-Aware]
semantic_search(query, limit, threshold)
    │
    ▼
EmbeddingEngine.embed(query) → query_vector
    │
    ▼
HnswIndex.search() → Vec<(id, score)>
    │
    ▼
MemoryGraph.rankResults() → Vec<RankedResult>
    │   (blends: vector_score × PageRank × Community boost)
    ▼
SqliteStore.lookup() → Vec<MemoryEntry>
    │
    ▼
Return Vec<SearchResult> (with PageRank scores)

[Learning Flow]
Task completed (success/failure)
    │
    ▼
Workers system triggers:
    │
    ├──► ReasoningBank.storePattern()  — short-term memory
    ├──► SONA.recordTrajectory()       — trajectory tracking
    ├──► MemoryGraph.updateEdge()      — update relationships
    │
    ▼ (after N uses or threshold)
Promotion: short-term → long-term
    │
    ▼
RVFLearningStore.persist() — save to .rvls file
```

---

## 3. New Components to Add

### 3.1 MemoryGraph (High Priority)

**역할**: 벡터 유사도 + 구조적 중요도 결합

```rust
// memory/graph.rs (new)

/// Memory entry references become graph edges.
/// PageRank computes structural importance.
pub struct MemoryGraph {
    nodes: HashMap<String, GraphNode>,
    edges: HashMap<String, Vec<GraphEdge>>,
    page_ranks: HashMap<String, f64>,
    communities: HashMap<String, String>,
}

impl MemoryGraph {
    /// Build graph from memory entries.
    pub async fn build_from_backend(&mut self, backend: &dyn IMemoryBackend);
    
    /// Add edge based on entry reference.
    pub fn add_reference_edge(&mut self, from: &str, to: &str);
    
    /// Compute PageRank (damping=0.85, iter=50).
    pub fn compute_pagerank(&mut self);
    
    /// Detect communities via label propagation.
    pub fn detect_communities(&mut self);
    
    /// Rank search results combining vector + graph scores.
    pub fn rank_results(&self, hnsw_results: Vec<(String, f32)>) -> Vec<RankedResult>;
}
```

**Algorithm**:
```
CombinedScore = α × VectorScore + β × PageRank + γ × CommunityBoost

Where:
  α = 0.6 (vector similarity weight)
  β = 0.3 (structural importance weight)
  γ = 0.1 (community cohesion boost)
  CommunityBoost = same_community ? 1.1 : 1.0
```

### 3.2 RVF Learning Store (High Priority)

**역할**: SONA 패턴, LoRA, EWC 상태의 영구 저장

```
File format (.rvls):
  4-byte magic "RVLS" + newline
  One JSON per line: {"type":"pattern"|"lora"|"ewc"|"trajectory","data":{...}}
```

```rust
// memory/rvf_learning_store.rs (new)

pub struct RvfLearningStore {
    patterns: HashMap<String, PatternRecord>,
    trajectories: Vec<TrajectoryRecord>,
    ewc_state: Option<EwcState>,
    store_path: PathBuf,
}

pub struct PatternRecord {
    id: String,
    strategy: String,
    embedding: Vec<f32>,
    success_rate: f32,
    use_count: u32,
    last_used: DateTime<Utc>,
}

pub struct TrajectoryRecord {
    id: String,
    steps: Vec<TrajectoryStep>,
    outcome: String,
    duration_ms: u64,
}
```

### 3.3 SONA Engine (Medium Priority — Phase 2)

**역할**: Self-Optimizing Neural Architecture

```rust
// memory/sona.rs (new)

/// SONA mode selection.
pub enum SonaMode {
    RealTime,   // <0.05ms adaptation
    Balanced,  // Default
    Research,  // Deep learning
    Edge,      // Resource-constrained
    Batch,     // Offline optimization
}

/// Trajectory for learning.
pub struct Trajectory {
    id: String,
    steps: Vec<TrajectoryStep>,
    verdict: Verdict,
}

pub struct TrajectoryStep {
    input: String,
    output: String,
    duration_ms: u64,
    confidence: f32,
}

pub enum Verdict {
    Success,
    PartialFailure,
    Failure,
}

impl SonalEngine {
    /// Record a task trajectory.
    pub async fn record(&mut self, trajectory: Trajectory);
    
    /// Adapt behavior based on pattern match.
    pub async fn adapt(&mut self, context: &Context) -> AdaptedBehavior;
    
    /// Extract and distill patterns from trajectories.
    pub async fn distill(&mut self) -> Vec<LearnedPattern>;
}
```

### 3.4 ReasoningBank (Medium Priority — Phase 2)

**역할**: Hook에서 패턴 학습/검색

```rust
// memory/reasoning_bank.rs (new)

/// Pattern with quality metrics.
pub struct GuidancePattern {
    id: String,
    strategy: String,
    domain: String,
    embedding: Vec<f32>,
    quality: f32,
    usage_count: u32,
    success_count: u32,
}

pub struct ReasoningBank {
    short_term: Vec<GuidancePattern>,
    long_term: Vec<GuidancePattern>,
    hnsw_index: HnswIndex,
}

impl ReasoningBank {
    /// Store a new pattern.
    pub fn store_pattern(&mut self, pattern: GuidancePattern);
    
    /// Search for relevant patterns.
    pub fn search(&self, query: &str, limit: usize) -> Vec<PatternMatch>;
    
    /// Promote short-term to long-term.
    pub fn promote(&mut self, pattern_id: &str);
    
    /// Route task to optimal agent.
    pub fn route_task(&self, task: &str) -> RoutingResult;
}
```

### 3.5 Workers System (Medium Priority — Phase 2)

**역할**: 12개 background worker로 자동 최적화

```rust
// workers.rs (new)

pub enum WorkerType {
    Ultralearn,    // Deep knowledge acquisition
    Optimize,       // Performance optimization
    Consolidate,   // Memory consolidation
    Predict,       // Predictive preloading
    Audit,         // Security analysis
    Map,           // Codebase mapping
    Deepdive,      // Deep code analysis
    Document,      // Auto-documentation
    Refactor,      // Refactoring suggestions
    Benchmark,     // Performance benchmarking
    Testgaps,      // Test coverage analysis
}

pub struct WorkerConfig {
    priority: WorkerPriority,
    interval_ms: u64,
    trigger_conditions: Vec<TriggerCondition>,
}

pub struct WorkerManager {
    workers: HashMap<WorkerType, Worker>,
    scheduler: Scheduler,
}

impl WorkerManager {
    /// Register a new worker.
    pub fn register(&mut self, worker: WorkerType, config: WorkerConfig);
    
    /// Dispatch worker execution.
    pub async fn dispatch(&self, worker: WorkerType) -> WorkerResult;
    
    /// Get worker status and alerts.
    pub fn status(&self) -> WorkerManagerStatus;
}
```

### 3.6 Flash Attention (Low Priority — Phase 3)

**역할**: 대량 벡터 연산 최적화

```rust
// memory/flash_attention.rs (new)

/// Block-wise attention for O(N) memory vs O(N²).
pub struct FlashAttention {
    block_size: usize,
    dimensions: usize,
}

impl FlashAttention {
    /// Compute attention with block-wise tiling.
    pub fn attention(
        &self,
        queries: &[Vec<f32>],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> AttentionResult;
    
    /// Benchmark naive vs flash attention.
    pub fn benchmark(&self, vectors: &[Vec<f32>]) -> BenchmarkResult;
}
```

### 3.7 Hyperbolic Embeddings (Low Priority — Phase 3)

**역할**: 계층적 데이터의 더 나은 표현

```rust
// memory/hyperbolic.rs (new)

/// Poincaré ball model for hierarchical embeddings.
pub mod hyperbolic {
    /// Convert Euclidean to Poincaré ball.
    pub fn euclidean_to_poincare(vector: &[f32], curvature: f32) -> Vec<f32>;
    
    /// Compute hyperbolic distance.
    pub fn hyperbolic_distance(a: &[f32], b: &[f32]) -> f32;
    
    /// Batch conversion.
    pub fn batch_euclidean_to_poincare(vectors: &[Vec<f32>], curvature: f32) -> Vec<Vec<f32>>;
}
```

### 3.8 Auto-Memory Bridge (Future — Phase 4)

**역할**: Claude Code auto-memory ↔ Oxios memory integration

```rust
// memory/auto_memory_bridge.rs (new)

/// Bidirectional sync between Claude Code and Oxios.
pub struct AutoMemoryBridge {
    auto_memory_dir: PathBuf,
    oxios_memory: Arc<MemoryManager>,
}

impl AutoMemoryBridge {
    /// Import Claude Code memories to Oxios.
    pub async fn import_from_auto(&self) -> ImportResult;
    
    /// Export Oxios patterns to Claude Code format.
    pub async fn export_to_auto(&self, patterns: &[GuidancePattern]) -> ExportResult;
    
    /// Sync on session end.
    pub async fn sync_session(&self);
}
```

---

## 4. Revised Implementation Phases

### Phase 1: Foundation (Week 1-2)
**목표**: HNSW 기반 메모리 시스템 완성

| Module | Tasks |
|--------|-------|
| `memory/hnsw.rs` | usearch integration, CRUD, persistence |
| `memory/store.rs` | SqliteIndex with hybrid storage |
| `memory/engine.rs` | OnnxEngine (tract-onnx) |
| `memory/graph.rs` | **NEW** MemoryGraph with PageRank |
| `memory/error.rs` | Error types |

**Deliverables**:
- `semantic_search()` with HNSW + Graph ranking
- Benchmark: P50 <10ms for 1K entries

### Phase 2: Learning (Week 3-4)
**목표**: Neural learning 시스템 통합

| Module | Tasks |
|--------|-------|
| `memory/rvf_learning_store.rs` | RVF format persistence |
| `memory/reasoning_bank.rs` | Pattern store/search/route |
| `memory/sona.rs` | SONA engine (simplified) |
| `workers.rs` | WorkerManager with 12 workers |
| `memory/flash_attention.rs` | Block-wise attention |

**Deliverables**:
- ReasoningBank.search() with pattern matching
- Worker dispatch for auto-optimization
- <0.05ms adaptation target

### Phase 3: Polish (Week 5-6)
**목표**: Production-ready 최적화

| Module | Tasks |
|--------|-------|
| `memory/chunking.rs` | Document chunking for long texts |
| `memory/normalizer.rs` | L2/FP16/INT8 normalization |
| `memory/hyperbolic.rs` | Poincaré ball embeddings |
| Migration | TF-IDF → HNSW migration tool |

**Deliverables**:
- Batch embedding with chunking
- Quantization (FP16 default, INT8 optional)
- Migration CLI command

### Phase 4: Integration (Week 7-8)
**목표**: Kernel + Web integration

| Module | Tasks |
|--------|-------|
| `kernel.rs` | Memory subsystem integration |
| `orchestrator.rs` | Memory enrichment for Ouroboros |
| Web routes | `/api/memory/*` endpoints |
| Events | KernelEvent updates |

**Deliverables**:
- Seed context enrichment from memory
- Web UI memory search
- SSE events for memory updates

---

## 5. Configuration

### 5.1 TOML Config

```toml
[memory]
# Storage
data_dir = "./data/memory"
index_path = "./data/memory/hnsw.usearch"
db_path = "./data/memory/memory.sqlite"

# Limits
max_recall = 10
max_entries = 100_000

# HNSW parameters
hnsw_m = 16
hnsw_ef_construction = 128
hnsw_ef_search = 128

[memory.embedding]
# Provider: onnx | openai | hybrid
provider = "onnx"
model_path = "./models/all-MiniLM-L6-v2.onnx"
dimensions = 384

[memory.openai]
api_key = "${OPENAI_API_KEY}"
model = "text-embedding-3-small"
dimensions = 1536

[memory.learning]
# ReasoningBank
hnsw_m = 16
hnsw_ef_construction = 200
max_short_term = 1000
max_long_term = 5000
promotion_threshold = 5
quality_threshold = 0.7

# SONA
sona_mode = "balanced"
lora_rank = 4
ewc_lambda = 1000

[memory.graph]
# MemoryGraph
similarity_threshold = 0.8
pagerank_damping = 0.85
max_nodes = 5000
community_detection = true

[memory.workers]
# WorkerManager
enabled = true
max_concurrency = 4

[memory.workers.performance]
enabled = true
interval_ms = 300000  # 5 minutes

[memory.workers.audit]
enabled = true
interval_ms = 600000  # 10 minutes
```

---

## 6. API Endpoints (Revised)

### 6.1 Memory Search

```
GET /api/memory/search?q={query}&limit={n}&threshold={t}&graph_boost={bool}
```

```json
{
  "query": "authentication patterns",
  "results": [
    {
      "entry": {
        "id": "abc123",
        "content": "OAuth 2.0 implementation with JWT...",
        "source": "agent:architect",
        "tags": ["auth", "security"],
        "importance": 0.8
      },
      "score": 0.92,
      "rank": 1,
      "page_rank": 0.15,
      "combined_score": 0.87,
      "community": "security-patterns"
    }
  ],
  "latency_ms": 12,
  "graph_enabled": true
}
```

### 6.2 Pattern Search (ReasoningBank)

```
GET /api/memory/patterns?q={query}&domain={domain}&limit={n}
```

```json
{
  "patterns": [
    {
      "id": "p123",
      "strategy": "Use parameterized queries for SQL",
      "domain": "security",
      "quality": 0.95,
      "usage_count": 47,
      "similarity": 0.89
    }
  ],
  "route_suggestion": {
    "agent": "security-auditor",
    "confidence": 0.92
  }
}
```

### 6.3 Learning Status

```
GET /api/memory/learning/stats
```

```json
{
  "reasoning_bank": {
    "short_term_count": 127,
    "long_term_count": 892,
    "avg_search_time_ms": 2.3
  },
  "sona": {
    "trajectories_recorded": 456,
    "patterns_learned": 234,
    "last_learning_ms": 0.03
  },
  "workers": {
    "active": 8,
    "last_run": {
      "ultralearn": "2026-05-14T10:30:00Z",
      "audit": "2026-05-14T10:25:00Z"
    }
  }
}
```

---

## 7. Kernel Integration (Revised)

### 7.1 Kernel Event Updates

```rust
// event_bus.rs - New events

pub enum KernelEvent {
    // ... existing ...
    
    // Memory events
    MemoryStored { id: String, memory_type: MemoryType },
    MemoryRecalled { query: String, count: usize, top_score: f32 },
    MemoryGraphUpdated { node_count: usize, edge_count: usize },
    
    // Learning events
    PatternLearned { pattern_id: String, quality: f32 },
    PatternPromoted { pattern_id: String, from_short_term: bool },
    TrajectoryRecorded { trajectory_id: String, verdict: String },
    SonalAdapted { adaptation_ms: f32 },
    
    // Worker events
    WorkerDispatched { worker: String, priority: String },
    WorkerCompleted { worker: String, duration_ms: u64 },
    WorkerAlert { worker: String, severity: AlertSeverity, message: String },
    
    // Graph events
    PageRankComputed { max_iterations: usize, converged: bool },
    CommunityDetected { communities: usize },
}
```

### 7.2 Ouroboros Integration

```rust
// ouroboros/seed.rs - Memory enrichment

impl Seed {
    /// Enrich with relevant memory before LLM call.
    pub async fn enrich_with_memory(&self) -> Result<()> {
        let query = self.phase().context();
        
        // Semantic search
        let search_results = self.memory()
            .semantic_search(&query, 5, 0.6)
            .await?;
        
        // Pattern search
        let patterns = self.reasoning_bank()
            .search(&query, 3)
            .await?;
        
        // Build context
        let memory_context = search_results
            .iter()
            .map(|r| format!("- [{}] {}\n{}", r.entry.source, r.entry.content, r.entry.tags.join(", ")))
            .join("\n\n");
        
        let pattern_context = patterns
            .iter()
            .map(|p| format!("- {} (quality: {:.0}%)", p.pattern.strategy, p.pattern.quality * 100))
            .join("\n");
        
        self.add_context(&format!(
            "## Relevant Memory\n{}\n\n## Learned Patterns\n{}\n",
            memory_context, pattern_context
        ));
        
        Ok(())
    }
}
```

---

## 8. Related Documents

- **하위 세부설계**: `design/memory-subdesign.md`
- **Architecture**: `ARCHITECTURE.md`
- **Embedding**: `embedding.rs` (current)
- **Memory**: `memory/mod.rs` (current)
- **Ruflo Reference**: [ruvnet/ruflo](https://github.com/ruvnet/ruflo)

---

## 9. Appendix: Ruflo Modules to Study

| File | Purpose | Our Action |
|------|---------|-----------|
| `memory/index.ts` | Unified memory interface | Study API design |
| `memory/memory-graph.ts` | PageRank + communities | **Adopt directly** |
| `memory/rvf-learning-store.ts` | RVF persistence | **Adopt directly** |
| `memory/auto-memory-bridge.ts` | Claude Code sync | Future integration |
| `neural/sona-integration.ts` | SONA engine | Study algorithm |
| `hooks/reasoningbank/` | Pattern learning | **Adopt concept** |
| `hooks/workers/` | Background workers | **Adopt concept** |
| `embeddings/index.ts` | Multi-provider | **Adopt multi-provider** |
| `embeddings/chunking.ts` | Document chunking | **Adopt** |
| `embeddings/normalization.ts` | L2/FP16/INT8 | **Adopt** |
| `embeddings/hyperbolic.ts` | Poincaré embeddings | Future |
| `neural/flash-attention.ts` | Block attention | Future |
| `rvf-event-log.ts` | Binary event log | **Adopt for event_bus** |