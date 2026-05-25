# Oxios Memory System Upgrade — HNSW 기반 벡터 메모리

> **문서 계층**: 전체 설계 (Architecture-Level)
> **상태**: 초안
> **날짜**: 2026-05-14
> **문서 관계**: `design/hnsw-memory-subdesign.md` (하위 세부설계)

---

## 1. Motivation

### 1.1 현재 상태

우리 메모리 시스템은 **TF-IDF 기반 sparse vector**를 사용합니다:

```rust
// 현재 embedding.rs
pub enum EmbeddingVector {
    Sparse(HashMap<String, f64>),  // TF-IDF (사용 중)
    Dense(Vec<f64>),               // 미사용
}
```

**문제점:**
- TF-IDF는 **단어 기반 매칭**이라 의미론적 유사도 파악 불가
  - "authentication" ↔ "auth" → 관련인데 다르게 인식
  - "날씨" ↔ "기상" → 같은 의미인데 다르게 인식
- **Brute-force 검색** (O(n)) — 데이터 증가 시 성능 저하
- Recall (재현율) ~70-80% — 자주 쓰이지 않는 패턴 놓침
- **Neural learning (ReasoningBank, SONA)**이 제대로 동작하려면 dense embedding 필수

### 1.2 목표

**최첨단 기술 스택으로 업그레이드:**

| 항목 | 현재 | 목표 |
|------|------|------|
| Embedding | TF-IDF (sparse) | **ONNX (dense, 384-1536차원)** |
| 인덱스 | 없음 (brute-force) | **HNSW (O(log n))** |
| 검색 속도 | O(n) 느림 | **150x~12,500x 향상** |
| Recall | ~70-80% | **~95%+** |
| 메모리 최적화 | 없음 | **INT8/FP16 양자화** |
| Self-learning | 미구현 | **ReasoningBank + SONA** |

---

## 2. Architecture Overview

### 2.1 System Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                      Application Layer                          │
│  Chat API │ Web UI │ CLI │ Telegram                            │
└──────────────────────────────┬──────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Kernel Memory API                          │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  semantic_search(query, limit, threshold) → Vec<MemoryEntry>  │
│  │  store(entry) → String (id)                                   │
│  │  recall(session_id) → Session                                 │
│  │  neural_learn(patterns) → ()                                 │
│  │  curate(budget) → CurationReport                             │
│  └──────────────────────────────────────────────────────────┘  │
└──────────────────────────────┬──────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Memory Subsystem                             │
│                                                                  │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐  │
│  │ MemoryManager  │  │ HnswIndex      │  │ EmbeddingEngine │  │
│  │ (orchestrator) │  │ (vector index) │  │ (ONNX/WASM)     │  │
│  └───────┬────────┘  └───────┬────────┘  └───────┬────────┘  │
│          │                   │                   │              │
│          ▼                   ▼                   ▼              │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐  │
│  │ StateStore     │  │ SQLite         │  │ ONNX Runtime   │  │
│  │ (JSON files)  │  │ (HNSW + meta)  │  │ (WASM bundle)  │  │
│  └────────────────┘  └────────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Component Responsibilities

| Component | Responsibility |
|-----------|-----------------|
| **MemoryManager** | Orchestrator — coordinates embedding + index + storage |
| **HnswIndex** | Vector index — HNSW graph structure for fast ANN search |
| **EmbeddingEngine** | ONNX WASM runtime — generates dense embeddings |
| **StateStore** | File-based persistence — JSON entries + metadata |
| **SqliteIndex** | Hybrid storage — HNSW graph + metadata in SQLite |

### 2.3 Data Flow

```
[Store Flow]
User input
    │
    ▼
EmbeddingEngine.embed(text) → DenseVector (384dim)
    │
    ▼
MemoryManager.store(entry)
    │
    ├─► StateStore.save_json() ──► memory/{type}/{id}.json
    │
    └─► SqliteIndex.insert() ──► vector_memory.db (HNSW + metadata)
    │
    └─► EventBus.publish(MemoryStored)

[Recall Flow]
semantic_search(query, limit, threshold)
    │
    ▼
EmbeddingEngine.embed(query) → query_vector
    │
    ▼
HnswIndex.search(query_vector, k, threshold) → Vec<(id, score)>
    │
    ▼
SqliteIndex.lookup_metadata(ids) → Vec<MemoryEntry>
    │
    ▼
Ranked by HNSW distance + importance boost
    │
    ▼
Return Vec<MemoryEntry>
```

---

## 3. Component Specifications

### 3.1 EmbeddingEngine

**역할**: 텍스트 → dense vector 변환

**기술 선택:**
- **ONNX Runtime WASM** (pure Rust integration via `ort-wasm`)
- **Model**: all-MiniLM-L6-v2 (384차원, 83MB)
  - HuggingFace에서 ONNX로 내보내둔 버전 사용
  - 한국어 포함 다국어 지원 (mteb benchmarks 참고)

**구성 옵션:**
| Mode | Embedding | Use Case |
|------|-----------|----------|
| `Local` | ONNX WASM (built-in) | 오프라인,隐私 |
| `Hybrid` | Local + API fallback | Local 실패 시 OpenAI fallback |

**API:**
```rust
pub trait EmbeddingEngine: Send + Sync {
    /// Generate embedding for text. Returns 384-dim vector.
    async fn embed(&self, text: &str) -> Result<DenseVector>;
    
    /// Batch embed for efficiency.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<DenseVector>>;
    
    /// Engine name (e.g., "onnx-mini-lm", "openai-ada-3").
    fn name(&self) -> &str;
    
    /// Check if engine is ready.
    async fn health_check(&self) -> Result<()>;
}
```

### 3.2 HnswIndex

**역할**: Approximate Nearest Neighbor (ANN) 검색

**기술 선택:**
- **usearch** (MichaelMGit/usearch) — Rust port of FAISS HNSW
  - Pure Rust (no C++ bindings needed)
  - INT8/FP16 양자화 내장
  - WASM 지원
  - MIT licensed

**인덱스 파라미터:**
| Parameter | Value | Notes |
|-----------|-------|-------|
| `M` (connections per node) | 16 | default, tunable |
| `ef_construction` | 128 | search width during build |
| `ef_search` | 128 | search width during query |
| `layers` | auto | HNSW levels (log scale) |
| `quantization` | `scalar_fp16` | 메모리 50% 절감 |

**API:**
```rust
pub struct HnswIndex {
    index: usearch::Index,
    metric: Metric,
}

impl HnswIndex {
    /// Create new index.
    pub fn new(dimensions: usize) -> Result<Self>;
    
    /// Add vector with metadata.
    pub fn insert(&mut self, id: &str, vector: &[f32]) -> Result<()>;
    
    /// Remove vector.
    pub fn remove(&mut self, id: &str) -> Result<()>;
    
    /// Search ANN.
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>>;
    
    /// Bulk load from existing vectors.
    pub fn load(&mut self, entries: Vec<(String, Vec<f32>)>) -> Result<()>;
    
    /// Persist to disk.
    pub fn save(&self, path: &Path) -> Result<()>;
    
    /// Load from disk.
    pub fn load_from(&mut self, path: &Path) -> Result<()>;
}
```

### 3.3 SqliteIndex (Hybrid Storage)

**역할**: HNSW 그래프 + 메타데이터 통합 저장

**Database Schema:**
```sql
-- HNSW 메타데이터
CREATE TABLE entries (
    id          TEXT PRIMARY KEY,
    memory_type TEXT NOT NULL,
    content     TEXT NOT NULL,
    source      TEXT NOT NULL,
    session_id  TEXT,
    tags        TEXT,           -- JSON array
    importance  REAL DEFAULT 0.5,
    created_at  TEXT NOT NULL,
    accessed_at TEXT NOT NULL,
    access_count INTEGER DEFAULT 0,
    vector_id   TEXT NOT NULL,   -- usearch internal ID
    metadata    TEXT             -- JSON additional data
);

CREATE TABLE embeddings (
    entry_id    TEXT PRIMARY KEY REFERENCES entries(id),
    vector      BLOB NOT NULL,   -- FP16 serialized
    dimensions  INTEGER NOT NULL
);

CREATE INDEX idx_memory_type ON entries(memory_type);
CREATE INDEX idx_session_id ON entries(session_id);
CREATE INDEX idx_created_at ON entries(created_at);

-- Full-text search hybrid (FTS5)
CREATE VIRTUAL TABLE entries_fts USING fts5(
    content,
    tags,
    content=entries,
    content_rowid=rowid
);
```

### 3.4 MemoryManager (Orchestrator)

**역할**: 모든 하위 시스템을 조율

**API:**
```rust
pub struct MemoryManager {
    engine: Arc<dyn EmbeddingEngine>,
    index: Arc<RwLock<HnswIndex>>,
    db: Arc<SqliteIndex>,
    state_store: Arc<StateStore>,
    git_layer: Option<Arc<GitLayer>>,
    config: MemoryConfig,
}

impl MemoryManager {
    /// Store with automatic embedding.
    pub async fn store(&self, input: MemoryEntryInput) -> Result<String>;
    
    /// Semantic search with threshold.
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SearchResult>>;
    
    /// Multi-namespace search.
    pub async fn search_namespaced(
        &self,
        namespaces: &[&str],
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>>;
    
    /// Update entry access stats.
    pub async fn touch(&self, id: &str) -> Result<()>;
    
    /// Bulk import from legacy format.
    pub async fn migrate_from_tfidf(&self) -> Result<MigrationReport>;
    
    /// Neural learning integration.
    pub async fn neural_learn(&self, pattern: &LearnedPattern) -> Result<()>;
    
    /// Budget-aware curation.
    pub async fn curate(&self, budget: &MemoryBudget) -> Result<CurationReport>;
}
```

---

## 4. Integration Points

### 4.1 Kernel Integration

```rust
// kernel.rs - KernelHandle
impl KernelHandle {
    /// Get memory subsystem.
    pub fn memory(&self) -> &MemoryManager;
    
    /// Direct semantic search shortcut.
    pub async fn recall(&self, query: &str) -> Result<Vec<MemoryEntry>>;
}
```

### 4.2 Event Bus Integration

```rust
// event_bus.rs - KernelEvent
pub enum KernelEvent {
    // ...existing variants...
    
    MemoryStored {
        id: String,
        memory_type: MemoryType,
        source: String,
    },
    MemoryRecalled {
        query: String,
        count: usize,
        top_score: f32,
    },
    MemoryIndexed {
        id: String,
        vector_id: usize,
    },
    EmbeddingGenerated {
        engine: String,
        latency_ms: u64,
    },
    NeuralPatternLearned {
        pattern_id: String,
        success_rate: f32,
    },
    MemoryCurated {
        removed_count: usize,
        budget_remaining: f32,
    },
}
```

### 4.3 Orchestrator Integration (Ouroboros)

```rust
// ouroboros/seed.rs - Seed lifecycle
impl Seed {
    /// Before LLM call: retrieve relevant patterns from HNSW memory.
    pub async fn enrich_with_memory(&self) -> Result<()> {
        let query = self.phase().context();
        let results = self.memory().semantic_search(&query, 5, 0.7).await?;
        
        let context = results
            .iter()
            .map(|r| format!("- {}\n{}", r.entry.source, r.entry.content))
            .join("\n\n");
        
        self.add_context(&format!("## Relevant Memory\n\n{context}"));
        Ok(())
    }
}
```

### 4.4 Web UI Integration

**New API Endpoints:**
```
GET  /api/memory/search?q={query}&limit={n}&threshold={t}
GET  /api/memory/entries?type={type}&page={n}
GET  /api/memory/stats
POST /api/memory/migrate
GET  /api/memory/stats/embeddings
```

**SSE Events:**
```
memory_stored      → New entry saved
memory_recalled   → Search results
memory_curated    → Pruning completed
embedding_latency → Performance metric
```

### 4.5 WebSocket Chat Integration

현재 `handle_chat`이 채팅 세션을 관리합니다. HNSW 메모리 통합 시:

```rust
// chat.rs - handle_chat
pub(crate) async fn handle_chat(
    state: State<Arc<AppState>>,
    Json(body): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    // Before LLM call: enrich with memory context
    let memory_context = state.kernel.memory()
        .semantic_search(&body.content, 5, 0.6)
        .await?
        .into_iter()
        .map(|r| r.entry.content)
        .collect::<Vec<_>>()
        .join("\n\n");
    
    // Inject into prompt
    let enriched_content = format!(
        "## Relevant Memory\n{}\n\n## User Query\n{}",
        memory_context, body.content
    );
    
    // ... rest of flow
}
```

---

## 5. Migration Strategy

### 5.1 Zero-Downtime Migration

```
Phase 1: Dual Write
├── HNSW index + TF-IDF index both maintained
├── Writes go to both
├── Reads prefer HNSW, fallback to TF-IDF
└── Status: 2-phase writes

Phase 2: Read Migration
├── HNSW read = primary
├── TF-IDF = fallback only if HNSW fails
└── Status: HNSW-first

Phase 3: Legacy Cleanup
├── TF-IDF reads disabled
├── Background batch: re-embed all entries to HNSW
└── Status: HNSW-only

Phase 4: TF-IDF Removal
├── Remove TF-IDF code path
├── Optimize HNSW parameters based on production data
└── Status: Clean HNSW
```

### 5.2 Migration Script

```bash
# CLI migration command
oxios memory migrate --from tfidf --to hnsw

# Progress tracking
oxios memory migrate --status
# → Migrated: 150/1000 entries (15%)
# → Failed: 3 entries (retrying...)
# → ETA: 5 minutes
```

---

## 6. Performance Targets

| Metric | Target | Measurement |
|--------|--------|------------|
| Embedding latency (single) | <50ms | ONNX WASM warm |
| Embedding latency (batch, 32) | <500ms | Batch processing |
| Search latency (1K entries) | <10ms | P50 |
| Search latency (10K entries) | <20ms | P50 |
| Index memory (1K entries) | <10MB | FP16 quantized |
| Index memory (10K entries) | <100MB | FP16 quantized |
| Recall rate | >95% | Benchmark against ground truth |
| Write throughput | >100/sec | Concurrent writes |

---

## 7. Open Questions

| Question | Decision Needed | Priority |
|----------|---------------|---------|
| ONNX model delivery | Bundle in binary? Download on first run? | P0 |
| Quantization precision | FP16 vs INT8 vs mixed | P1 |
| HNSW parameters | Auto-tune based on data size? | P2 |
| Embedding cache | LRU cache with TTL? | P2 |
| Multi-tenant isolation | Namespace per user/project? | P2 |

---

## 8. Related Documents

- **하위 세부설계**: `design/hnsw-memory-subdesign.md`
- **Architecture**: `ARCHITECTURE.md`
- **Embedding**: `embedding.rs` (current)
- **Memory**: `memory/mod.rs` (current)
- **Ruflo Reference**: [ruvnet/ruflo](https://github.com/ruvnet/ruflo) — `v3/@claude-flow/memory/`, `v3/@claude-flow/embeddings/`