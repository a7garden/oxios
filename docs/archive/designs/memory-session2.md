# Memory System — Session 2 작업 지시서

> **목적**: Learning 시스템 통합
> **前提**: Session 1 완료 (HNSW + MemoryGraph 작동 중)
> **Deliverable**: Pattern routing 작동, Workers dispatch 작동
> **순서**: 아래 번호순으로 진행

---

## 작업 순서 (번호 순)

### 1. memory/reasoning_bank.rs 생성

**Purpose**: Hook에서 패턴 학습/검색/라우팅

**데이터 구조**:
```rust
pub struct GuidancePattern {
    id: String,
    strategy: String,
    domain: String,        // "security", "testing", "performance"
    embedding: Vec<f32>,
    quality: f32,           // 0.0-1.0
    usage_count: u32,
    success_count: u32,
    created_at: DateTime<Utc>,
}

pub struct PatternMatch {
    pattern: GuidancePattern,
    similarity: f32,
}

pub struct RoutingResult {
    agent: String,         // "security-auditor", "tester", "coder"
    confidence: f32,       // 0.0-1.0
    reasoning: String,
}
```

**기능**:
- `ReasoningBank::store_pattern(pattern)` — 패턴 저장
- `ReasoningBank::search(query, limit)` → `Vec<PatternMatch>` — 패턴 검색
- `ReasoningBank::route_task(task)` → `RoutingResult` — Task → Agent 라우팅
- `ReasoningBank::promote(pattern_id)` — short-term → long-term
- HNSW 인덱스 사용

**라우팅 테이블** (초기값):
| Task Keyword | Recommended Agent |
|--------------|-------------------|
| security, auth, password, token | `security-auditor` |
| test, spec, mock, coverage | `tester` |
| perf, optimize, slow, memory | `performance-engineer` |
| fix, bug, error, debug | `researcher` |
| refactor, architect, design | `system-architect` |
| default | `coder` |

### 2. memory/rvf_store.rs 생성

**Purpose**: 패턴/LoRA/EWC binary 영속 저장

**RVF Format**:
```
4-byte magic "RVLS" + newline
{"type":"pattern","data":{...}}\n
{"type":"trajectory","data":{...}}\n
{"type":"ewc","data":{...}}\n
4-byte magic "REND"
```

**데이터 구조**:
```rust
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
    outcome: String,        // "success", "partial", "failure"
    duration_ms: u64,
}

pub struct TrajectoryStep {
    input: String,
    output: String,
    duration_ms: u64,
    confidence: f32,
}

pub struct EwcState {
    tasks_learned: u32,
    protection_strength: f32,
    task_weights: HashMap<String, Vec<f32>>,
}
```

**기능**:
- `RvfLearningStore::new(store_path)` — 생성
- `RvfLearningStore::initialize()` — 파일에서 로드
- `RvfLearningStore::persist()` — 파일에 저장
- `RvfLearningStore::save_pattern(pattern)` — 패턴 추가
- `RvfLearningStore::get_all_patterns()` → `Vec<PatternRecord>`
- `RvfLearningStore::save_trajectory(trajectory)` — Trajectory 추가
- `RvfLearningStore::save_ewc(ewc)` — EWC 상태 저장

### 3. memory/engine.rs 수정 — OpenAI + Hybrid Engine

**OpenAiEngine 추가**:
```rust
pub struct OpenAiEngine {
    client: reqwest::Client,
    api_key: String,
    model: String,          // "text-embedding-3-small"
    dimensions: usize,
}
```

- `OpenAiEngine::new(api_key, model, dimensions)` — 생성
- POST `/v1/embeddings` → embedding vector
- API key 없으면 `EmbeddingFailed`

**HybridEngine 추가**:
```rust
pub struct HybridEngine {
    primary: Arc<dyn EmbeddingEngine>,   // OnnxEngine
    fallback: Arc<dyn EmbeddingEngine>,  // OpenAiEngine
}
```

- `HybridEngine::embed(text)` — primary 시도, 실패 시 fallback
- `HybridEngine::embed_batch(texts)` — 동일

### 4. event_bus.rs 수정 — Learning Events 추가

```rust
pub enum KernelEvent {
    // ... existing ...
    
    // Learning events
    PatternLearned { pattern_id: String, quality: f32 },
    PatternPromoted { pattern_id: String },
    TrajectoryRecorded { trajectory_id: String, outcome: String },
    
    // Worker events
    WorkerDispatched { worker: String, priority: String },
    WorkerCompleted { worker: String, duration_ms: u64 },
    WorkerAlert { worker: String, severity: String, message: String },
}
```

### 5. memory/migrate.rs 생성

**Purpose**: TF-IDF → HNSW migration

**Migration Flow**:
```
1. Load legacy entries from StateStore
2. For each entry:
   a. Generate embedding via engine
   b. Insert into HNSW index
   c. Insert into SqliteIndex
3. Report progress (migrated, failed, total)
```

**기능**:
```rust
pub struct MigrationProgress {
    total: usize,
    migrated: usize,
    failed: usize,
    errors: Vec<(String, String)>,
}

pub struct MigrationReport {
    progress: MigrationProgress,
    duration_ms: u64,
}

impl MemoryManager {
    pub async fn migrate_from_tfidf(&self) -> Result<MigrationReport> {
        // 1. Load all entries from StateStore
        // 2. Batch embed (32 items at a time)
        // 3. Insert into HNSW + Sqlite
        // 4. Report results
    }
}
```

### 6. memory/sona.rs 생성 (Simplified)

**Purpose**: SONA — Self-Optimizing Neural Architecture

**Simplified Version** (핵심 개념만):
```rust
pub enum SonaMode {
    RealTime,  // <0.05ms adaptation
    Balanced,
    Research,
    Edge,
}

pub struct Trajectory {
    id: String,
    steps: Vec<TrajectoryStep>,
    verdict: Verdict,
}

pub enum Verdict {
    Success,
    PartialFailure,
    Failure,
}

pub struct SonalEngine {
    mode: SonaMode,
    trajectories: Vec<Trajectory>,
    learned_patterns: Vec<LearnedPattern>,
}

impl SonalEngine {
    pub fn new(mode: SonaMode) -> Self
    
    pub fn record(&mut self, trajectory: Trajectory)
    
    pub fn distill(&self) -> Vec<LearnedPattern> {
        // Extract patterns from successful trajectories
    }
    
    pub fn adapt(&self, query: &str) -> Option<LearnedPattern> {
        // Find most similar trajectory
    }
}
```

**Performance Target**: `<0.05ms` adaptation

### 7. workers/mod.rs 생성

**Purpose**: Background worker management

**Worker Types** (12개):
| Worker | Priority | Interval | Purpose |
|--------|----------|----------|---------|
| `ultralearn` | normal | 60s | Deep knowledge acquisition |
| `audit` | critical | 600s | Security analysis |
| `optimize` | high | 300s | Performance optimization |
| `consolidate` | low | 1800s | Memory consolidation |
| `predict` | normal | 300s | Predictive preloading |
| `map` | normal | 600s | Codebase mapping |
| `deepdive` | normal | 600s | Deep code analysis |
| `document` | normal | 1800s | Auto-documentation |
| `refactor` | normal | 600s | Refactoring suggestions |
| `benchmark` | normal | 600s | Performance benchmarking |
| `testgaps` | normal | 600s | Test coverage analysis |
| `learning` | normal | 900s | Neural pattern training |

**데이터 구조**:
```rust
pub enum WorkerType { /* 위表格 */ }

pub enum WorkerPriority { Critical, High, Normal, Low }

pub struct WorkerConfig {
    worker_type: WorkerType,
    priority: WorkerPriority,
    interval_ms: u64,
    enabled: bool,
}

pub struct WorkerResult {
    worker: WorkerType,
    success: bool,
    duration_ms: u64,
    output: String,
}

pub struct WorkerManager {
    workers: HashMap<WorkerType, WorkerConfig>,
    running: Arc<RwLock<HashSet<WorkerType>>>,
}
```

**기능**:
- `WorkerManager::new()` — 생성
- `WorkerManager::register(worker, config)` — Worker 등록
- `WorkerManager::dispatch(worker)` → `WorkerResult` — Worker 실행
- `WorkerManager::dispatch_all()` — 모든 worker 실행
- `WorkerManager::status()` → `WorkerManagerStatus`

**Dispatcher 구현**:
- `audit` worker: Security scan via `oxios-kernel/security`
- `optimize` worker: Performance analysis
- `ultralearn` worker: Pattern distillation from ReasoningBank
- `consolidate` worker: Memory curation

### 8. Web API 확장

`routes/memory_routes.rs` 추가:
```rust
GET /api/memory/patterns?q={query}&domain={domain}
GET /api/memory/learning/stats
POST /api/memory/migrate
GET /api/workers/status
POST /api/workers/dispatch?worker={name}
```

---

## 완료 기준

```bash
cargo test -p oxios-kernel reasoning
# → ReasoningBank 테스트 통과

cargo test -p oxios-kernel workers
# → Workers 테스트 통과

cargo test -p oxios-kernel memory
# → 전체 메모리 테스트 통과
```

---

## Session 2 → Session 3 넘어가기

Session 3은 optional — Phase 3 (Polish) 항목:
- Flash Attention (`memory/flash_attention.rs`)
- Hyperbolic Embeddings (`memory/hyperbolic.rs`)
- Auto-Memory Bridge (`memory/auto_memory_bridge.rs`)

Session 2 완료 후 진행 여부 결정.

---

## 참조 문서

- 메인 설계: `docs/design/memory-main-design.md`
- Session 1: `docs/design/memory-session1.md`
- 전체 OS: `docs/ARCHITECTURE.md`