# Memory System — Session 1 작업 지시서

> **목적**: 동작하는 HNSW 메모리 시스템 완성
> **Deliverable**: `semantic_search()` P50 <10ms, `cargo test memory` 통과
> **순서**: 아래 번호순으로 진행

---

## 작업 순서 (번호 순)

### 1. dependencies 추가

`crates/oxios-kernel/Cargo.toml`의 `[dependencies]` 섹션에 추가:

```toml
usearch = { version = "0.16", features = ["simd"] }
tract-onnx = { version = "0.26", features = ["onnx"] }
rusqlite = { version = "0.34", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json"] }
ndarray = "0.16"
```

### 2. memory/error.rs 생성

`crates/oxios-kernel/src/memory/error.rs`:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("embedding failed: {0}")]
    EmbeddingFailed(String),
    #[error("index error: {0}")]
    IndexError(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    #[error("invalid dimensions: expected {expected}, got {actual}")]
    InvalidDimensions { expected: usize, actual: usize },
}
```

### 3. memory/hnsw.rs 생성

**핵심 기능**:
- `HnswIndex::new(dimensions)` — 새 인덱스 생성
- `HnswIndex::insert(id, vector)` — 벡터 추가
- `HnswIndex::search(query, k)` — ANN 검색, `Vec<(String, f64)>` 반환
- `HnswIndex::remove(id)` — 삭제
- `HnswIndex::save(path)` / `load(path)` — Persistence
- usearch 사용, `MetricKind::Cosine`, `ScalarKind::F16`

### 4. memory/graph.rs 생성

**핵심 기능**:
- `MemoryGraph::add_node(entry)` — 노드 추가
- `MemoryGraph::add_reference_edge(from, to)` — 참조 엣지
- `MemoryGraph::add_similarity_edge(from, to, similarity)` — 유사도 엣지
- `MemoryGraph::compute_pagerank()` — PageRank 계산 (damping=0.85)
- `MemoryGraph::detect_communities()` — Community 감지 (label propagation)
- `MemoryGraph::rank_results(hnsw_results, alpha, beta)` — Combined scoring

**Algorithm**:
```rust
CombinedScore = 0.6 * vector_score + 0.3 * page_rank + 0.1 * community_boost
```

### 5. memory/store.rs 수정

**SqliteIndex 추가**:
- 테이블: `entries` (id, memory_type, content, source, session_id, tags, importance, created_at, accessed_at, access_count)
- 테이블: `embeddings` (entry_id, vector_f32)
- `SqliteIndex::insert(entry, vector)` — 저장
- `SqliteIndex::lookup(ids)` — ID로 조회
- `SqliteIndex::touch(id)` — access_count 증가
- `SqliteIndex::delete(id)` — 삭제
- `SqliteIndex::get_all()` — 전체 조회 (migration용)

### 6. memory/engine.rs 수정

**OnnxEngine 추가**:
- trait: `EmbeddingEngine` (embed, embed_batch, name, dimensions, health_check)
- `OnnxEngine::load(model_path, dimensions)` — ONNX 모델 로드
- `OnnxEngine::embed(text)` → `EmbeddingResult`
- 모델: `all-MiniLM-L6-v2` (384차원)
- 모델 파일 없으면 `ModelNotFound` 에러

**OpenAiEngine 추가** (Phase 1에서는 stub만):
- API key 미구성 시 `EngineNotConfigured` 에러

### 7. memory/chunking.rs 생성

**Document Chunking**:
- `ChunkingConfig`: chunk_size=512, overlap=50
- `chunk_text(text, config)` → `ChunkedDocument`
- 长文档 分할, overlap 포함
- `estimate_tokens(text)` → usize

### 8. memory/normalizer.rs 생성

**Normalization**:
- `l2_normalize(vector)` — Unit vector
- `l2_norm(vector)` → f32
- `f32_to_fp16(f32)` → `Vec<u8>` — 저장용
- `fp16_to_f32(fp16)` → `Vec<f32>` — 복원용

### 9. memory/mod.rs 수정

**MemoryManager 확장**:
- `MemoryManager::new_with_hnsw(store, config, data_dir)` — 초기화
- `MemoryManager::store(input)` — 저장 + embedding + graph 추가
- `MemoryManager::semantic_search(query, limit, threshold)` — HNSW + Graph 검색
- `MemoryManager::forget(id, memory_type)` — 삭제
- Graph ranking 통합

**기존 API 유지** (하위 호환):
- `store(input)` → `Result<String>`
- `semantic_search(...)` → `Result<Vec<SearchResult>>`

### 10. embedding.rs 수정

**DenseVector 추가**:
```rust
pub enum EmbeddingVector {
    Sparse(HashMap<String, f64>),  // 기존
    Dense(Vec<f32>),               // 새로 추가
}
```

### 11. ouroboros/seed.rs 수정

**Memory Enrichment**:
```rust
impl Seed {
    pub async fn enrich_with_memory(&self) -> Result<()> {
        // 1. semantic_search(query, 5, 0.6)
        // 2. pattern_search(query, 3)
        // 3. add_context("## Relevant Memory\n...")
    }
}
```

### 12. routes/memory_routes.rs 생성 (Web API)

```rust
GET /api/memory/search?q={query}&limit={n}&threshold={t}
GET /api/memory/graph/stats
```

### 13. 테스트 작성

`crates/oxios-kernel/src/memory/tests.rs`:
- `test_hnsw_insert_and_search`
- `test_graph_pagerank`
- `test_memory_store_and_search`
- `test_semantic_search_ranking`

---

## 완료 기준

```bash
cargo test -p oxios-kernel memory
# → 전부 통과

cargo build -p oxios-kernel --features memory-hnsw
# → 컴파일 성공
```

---

## 참조 문서

- 메인 설계: `docs/design/memory-main-design.md`
- 전체 OS: `docs/ARCHITECTURE.md`