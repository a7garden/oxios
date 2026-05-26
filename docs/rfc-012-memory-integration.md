# RFC-012: Memory System Full Integration

> **상태:** 구현 완료 (Phase 1–7)
> **날짜:** 2026-05-26
> **선행:** RFC-008 (Memory Consolidation)
> **범위:** `crates/oxios-kernel/src/memory/`, `crates/oxios-kernel/src/embedding/`, `src/kernel.rs`

---

## 0. 문제 요약

RFC-008에서 설계된 메모리 서브시스템 9개 중 **7개가 구현만 되고 런타임에 연결되지 않은 상태**다.
근본 원인 3가지:

1. **임베딩이 TF-IDF (Sparse)만 있어서** HNSW가 작동하지 않는다
2. **저장소가 분산되어 있다** — JSON 파일 + HNSW 인덱스 + TF-IDF 스냅샷 + 캐시 파일, 4개가 따로 논다
3. **BM25가 구현되어 있지 않다** — TF-IDF cosine이 키워드 검색의 전부다

### 해결: 2개의 레이어를 동시에 교체

```
기존:
  TF-IDF (Sparse)  → usearch HNSW → 직접 구현 BM25 → JSON 파일 저장
  ❌ Sparse → to_f32_dense() = None → HNSW에 데이터 없음
  ❌ 저장소 4개, 원자성 없음, BM25 없음

변경 후:
  EmbeddingGemma (GGUF Dense) → sqlite-vec (벡터 검색)
                             → FTS5 (BM25 키워드 검색)
                             → SQLite 단일 파일 (ACID, 백업 = 파일 하나)
```

---

## 1. 설계 원칙

1. **Cross-platform**: macOS, Linux, Windows 모두 지원. Apple Silicon 전용 아님.
2. **SQLite 단일 파일**: 모든 메모리 데이터가 하나의 `.db` 파일에 들어간다.
3. **Lazy Loading**: 임베딩 모델 ~329MB는 필요할 때만 로드, 유휴 시 해제.
4. **Pure Rust**: Python 없이 llama-gguf + rusqlite + sqlite-vec로 직접 구현.
5. **점진적 연결**: Feature flag로 단계 도입. 기존 API 변경 없음.
6. **Zero-maintenance**: RFC-008 원칙 유지. 사용자가 신경 쓸 것 없음.

---

## 2. 임베딩 모델: EmbeddingGemma-300m (GGUF)

### 2.1 선택 근거

| 항목 | 값 |
|------|-----|
| **사용 모델** | `unsloth/embeddinggemma-300m-GGUF` (Q4_K_M) |
| **원본 모델** | `google/embeddinggemma-300m` |
| **아키텍처** | Gemma 3 Text (`GemmaEmbedding` — llama-gguf 지원) |
| **Q4_K_M 디스크** | 329MB |
| **차원** | 768 (Matryoshka: 128 / 256 / 512 / 768) |
| **최대 입력** | 2048 tokens |
| **어텐션** | 양방향 (`use_bidirectional_attention: true`) |
| **언어** | 100+ (한국어 포함) |
| **MTEB 다국어** | 60.62 (Q4_0) |
| **라이선스** | Gemma Terms of Use (상업적 사용 허가, 재배포 시 terms 포함 필수) |

### 2.2 왜 GGUF인가

| | MLX (이전 설계) | GGUF (현재) |
|---|---|---|
| **빌드** | Xcode + Metal SDK 필요 | `cargo build` 즉시 |
| **플랫폼** | Apple Silicon 전용 | macOS / Linux / Windows |
| **GPU** | Metal 전용 | Metal / CUDA / Vulkan / DX12 |
| **모델 포팅** | Gemma 3 직접 포팅 (~530줄) | llama-gguf 내장 (0줄) |
| **토크나이저** | 별도 tokenizers 크레이트 | GGUF 파일에 내장 |
| **양자화** | mlx-rs Quantized 지원 필요 | Q4_K_M 네이티브 지원 |
| **CPU 최적화** | Metal GPU 위주 | SIMD (AVX2, NEON, WASM) |

### 2.3 라이선스 상세

EmbeddingGemma-300m은 **Gemma Terms of Use**를 따른다.

| 조항 | 내용 |
|------|------|
| **상업적 사용** | ✅ 허가 |
| **수정/파생** | ✅ 허가 (수정 파일에 명시 필수) |
| **재배포** | ✅ 허가 (Gemma Terms 사본 포함) |
| **Output 권리** | Google이 Output에 권리 주장하지 않음 |
| **금지用途** | Prohibited Use Policy 준수 (해킹, 불법, 차별 등 금지) |
| **보증** | AS-IS, 무보증 |

> 배포 시 NOTICE 파일에 Gemma Terms of Use 출처 명시. Oxios(오픈소스)에서 사용하는 데 문제없다.

### 2.4 GGUF 파일 스펙

`unsloth/embeddinggemma-300m-GGUF`의 `embeddinggemma-300m-Q4_K_M.gguf` 기준:

```
model_type:                   gemma3_text
architectures:                ["Gemma3TextModel"]
hidden_size:                  768
intermediate_size:            1152  (1.5× hidden)
num_hidden_layers:            24
num_attention_heads:          3
num_key_value_heads:          1     (extreme GQA, ratio=3)
head_dim:                     256
vocab_size:                   262144
max_position_embeddings:      2048
use_bidirectional_attention:  true  ← 임베딩 전용, causal mask 없음
quantization:                 Q4_K_M (group_size=32, 4-bit)
```

---

## 3. SQLite 아키텍처

### 3.1 왜 SQLite인가

| | 기존 (JSON + usearch + TF-IDF) | SQLite |
|---|---|---|
| **BM25** | 직접 구현 필요 | FTS5 내장, CJK 지원, 프로덕션 10년+ |
| **벡터 검색** | usearch HNSW | sqlite-vec (brute force KNN) |
| **저장소** | JSON 파일 + 인덱스 + 캐시 = 4개 | **단일 파일** |
| **백업** | 여러 파일 복사 | 파일 하나 복사 |
| **원자성** | 없음 (부분 손상 가능) | ACID 트랜잭션 |
| **검색/필터** | 직접 구현 | SQL |
| **규모** | HNSW: 백만 개 이상 의미 | 1만 개 이하에 충분 (Oxios 메모리 규모) |

> **sqlite-vec pre-v1 리스크**: Alex Garcia 제작, Mozilla 후원, 88 릴리즈, Rust 바인딩 있음.
> Oxios 메모리는 개인 에이전트 기준 ~1만 개. brute force KNN도 1ms 이하.

### 3.2 데이터베이스 스키마

```sql
-- ~/.oxios/workspace/memory.db (단일 파일)

-- ─────────────────────────────────────────────
-- 1. 메모리 엔트리 (기존 StateStore 대체)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS memories (
    id          TEXT PRIMARY KEY,
    memory_type TEXT NOT NULL,
    content     TEXT NOT NULL,
    importance  REAL NOT NULL DEFAULT 0.5,
    tier        TEXT NOT NULL DEFAULT 'warm',
    protection  TEXT NOT NULL DEFAULT 'none',
    source      TEXT NOT NULL DEFAULT 'unknown',
    session_id  TEXT,
    space_id    TEXT,
    tags        TEXT,                       -- JSON array
    metadata    TEXT,                       -- JSON object
    access_count    INTEGER NOT NULL DEFAULT 0,
    pinned          INTEGER NOT NULL DEFAULT 0,
    auto_classified INTEGER NOT NULL DEFAULT 0,
    session_appearances INTEGER NOT NULL DEFAULT 0,
    decay_score     REAL NOT NULL DEFAULT 1.0,
    compaction_level INTEGER NOT NULL DEFAULT 0,
    content_hash    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    accessed_at TEXT,
    decay_rate  REAL NOT NULL DEFAULT 0.01
);

CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
CREATE INDEX IF NOT EXISTS idx_memories_session ON memories(session_id);
CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance);
CREATE INDEX IF NOT EXISTS idx_memories_tier ON memories(tier);

-- ─────────────────────────────────────────────
-- 2. FTS5 전문 검색 (BM25) + 한국어/CJK 지원
-- ─────────────────────────────────────────────
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    id,
    content,
    memory_type,
    content='memories',
    content_rowid='rowid',
    tokenize="unicode61"
);

-- 자동 동기화 트리거 (INSERT/UPDATE/DELETE)

-- ─────────────────────────────────────────────
-- 3. 벡터 저장 (sqlite-vec)
-- ─────────────────────────────────────────────
-- 차원은 런타임에 설정 (Matryoshka: 128/256/512/768)
CREATE VIRTUAL TABLE IF NOT EXISTS memory_vectors USING vec0(
    embedding float[256]    -- 실제 차원은 config에 따라 결정
);

-- ─────────────────────────────────────────────
-- 4. 임베딩 캐시
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS embedding_cache (
    content_hash TEXT PRIMARY KEY,
    embedding    BLOB NOT NULL,
    created_at   TEXT NOT NULL
);

-- ─────────────────────────────────────────────
-- 5. Dream 상태 (DreamProcess 영속화)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS dream_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ─────────────────────────────────────────────
-- 6. 학습 패턴 (SONA + ReasoningBank + RVF)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS patterns (
    id           TEXT PRIMARY KEY,
    strategy     TEXT NOT NULL,
    domain       TEXT,
    quality      REAL NOT NULL DEFAULT 0.5,
    use_count    INTEGER NOT NULL DEFAULT 0,
    success_rate REAL NOT NULL DEFAULT 0.0,
    is_long_term INTEGER NOT NULL DEFAULT 0,
    embedding    BLOB,
    data         TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
```

### 3.3 구현된 구조

```
crates/oxios-kernel/src/
├── embedding/
│   ├── mod.rs                  # EmbeddingProvider trait
│   ├── tfidf.rs                # TfIdfEmbeddingProvider (legacy fallback)
│   └── gguf/                   # ← GGUF 기반 (MLX 대체)
│       ├── mod.rs              # GgufEmbeddingProvider (lazy load + TTL)
│       └── loader.rs           # HfClient 다운로드 + llama-gguf 로드
│
├── memory/
│   ├── mod.rs                  # MemoryManager (SQLite 델리게이션)
│   ├── store.rs                # remember/search → SQLite 위임
│   ├── sqlite_store.rs         # SqliteMemoryStore (CRUD + 검색 + 패턴)
│   ├── search/
│   │   ├── mod.rs              # 통합 검색 (KNN + BM25 → RRF)
│   │   ├── vector.rs           # sqlite-vec KNN 검색
│   │   ├── bm25.rs             # FTS5 BM25 검색 (CJK 지원)
│   │   └── rrf.rs              # Reciprocal Rank Fusion
│   ├── database.rs             # SQLite 연결 + 스키마 초기화
│   ├── cache.rs                # 임베딩 캐시 (SQLite 기반)
│   ├── migration.rs            # 기존 JSON → SQLite 마이그레이션
│   │
│   │  ── 연결 완료된 모듈 ──
│   ├── dream.rs                # DreamProcess (+ PageRank + Hyperbolic)
│   ├── decay.rs                # DecayEngine (+ PageRank boost)
│   ├── proactive.rs            # ProactiveRecall (SQLite search 기반)
│   ├── graph.rs                # MemoryGraph (co-access → PageRank)
│   ├── root_index.rs           # RootIndex
│   ├── hyperbolic.rs           # HyperbolicEmbedding (SQLite 영속화)
│   ├── flash_attention.rs      # FlashAttention (Recall 재랭킹)
│   ├── sona.rs                 # SonaEngine (SQLite 패턴 영속화)
│   ├── reasoning_bank.rs       # ReasoningBank (SQLite 패턴 영속화)
│   ├── auto_memory_bridge.rs   # AutoMemoryBridge (SQLite ↔ MEMORY.md)
│   └── ...                     # 기타 기존 모듈
```

### 3.4 의존성

```toml
# crates/oxios-kernel/Cargo.toml

[dependencies]
rusqlite = { version = "0.34", features = ["bundled"], optional = true }
sqlite-vec = { version = "0.1", optional = true }
llama-gguf = { version = "0.14", optional = true }
hf-hub = { version = "0.3", optional = true }

[features]
default = ["browser", "sqlite-memory"]
sqlite-memory = ["dep:rusqlite", "dep:sqlite-vec"]
embedding-gguf = ["dep:llama-gguf", "dep:hf-hub"]
```

---

## 4. SQLite MemoryDatabase

### 4.1 초기화

```rust
// memory/database.rs — 이미 구현됨

pub struct MemoryDatabase {
    conn: Mutex<Connection>,
    embedding_dim: usize,
}

impl MemoryDatabase {
    pub fn open(db_path: &Path, embedding_dim: usize) -> Result<Self> {
        // sqlite-vec 확장 로드, WAL 모드, 스키마 초기화
    }

    pub fn open_in_memory(embedding_dim: usize) -> Result<Self> {
        // 테스트용 in-memory DB
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock()  // parking_lot::Mutex
    }
}
```

### 4.2 쓰기: remember()

```rust
// memory/sqlite_store.rs — 이미 구현됨

impl SqliteMemoryStore {
    pub async fn remember(&self, entry: &MemoryEntry) -> Result<String> {
        // 1. INSERT OR REPLACE INTO memories
        // 2. FTS5 트리거가 자동 동기화
        // 3. Dense embedding 계산 → INSERT INTO memory_vectors
        // 4. Cache embedding
    }
}
```

### 4.3 검색: search()

```rust
// memory/sqlite_store.rs — 이미 구현됨

impl SqliteMemoryStore {
    pub async fn search(&self, query: &str, memory_type: Option<MemoryType>, limit: usize)
        -> Result<Vec<MemoryEntry>>
    {
        // 1. 쿼리 임베딩 계산 (캐시 활용)
        // 2. search::search() 호출
        //    - Tier 1: sqlite-vec KNN (Dense cosine)
        //    - Tier 2: FTS5 BM25 (키워드, CJK 지원)
        //    - RRF Fusion
        // 3. MemoryEntry로 변환하여 반환
    }
}
```

### 4.4 RRF (Reciprocal Rank Fusion)

```rust
// memory/search/rrf.rs — 이미 구현됨

pub fn reciprocal_rank_fusion(results: Vec<Vec<(i64, f64)>>, k: f64) -> Vec<(i64, f64)> {
    // K=60 표준값. 각 tier의 rank 위치로 점수를 계산하여 합산.
}
```

---

## 5. GGUF 임베딩 구현

### 5.1 GgufEmbeddingProvider

```rust
// embedding/gguf/mod.rs — 이미 구현됨

pub struct GgufEmbeddingProvider {
    model_dir: PathBuf,
    dimension: EmbeddingDimension,
    inner: Mutex<Option<LoadedModel>>,
    model_ttl: Duration,
    last_used: Mutex<Instant>,
}

impl GgufEmbeddingProvider {
    fn ensure_loaded(&self) -> Result<()> {
        // 첫 호출 시 모델 다운로드 + 로드
        // 이후에는 캐시된 모델 재사용
    }

    fn encode(&self, text: &str) -> Result<Vec<f32>> {
        // llama-gguf EmbeddingExtractor 사용
        // Mean pooling + L2 normalize + Matryoshka truncate
    }

    pub fn maybe_unload(&self) {
        // TTL 만료 시 모델 해제
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for GgufEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<EmbeddingVector> {
        self.ensure_loaded()?;
        let vec = self.encode(text)?;
        Ok(EmbeddingVector::DenseF32(vec))
    }

    fn name(&self) -> &str { "gguf-embeddinggemma-300m" }
}
```

### 5.2 모델 로더

```rust
// embedding/gguf/loader.rs — 이미 구현됨

pub struct GgufModelLoader;

impl GgufModelLoader {
    pub fn ensure_model(model_dir: &Path) -> Result<PathBuf> {
        // llama-gguf HfClient로 다운로드 (캐시 지원)
        // 모델: unsloth/embeddinggemma-300m-GGUF
        // 파일: embeddinggemma-300m-Q4_K_M.gguf (~329MB)
    }
}
```

### 5.3 추론 파이프라인

```
입력 텍스트 "Rust programming language"
       │
       ▼
  Tokenizer::from_gguf()  ← GGUF 파일에 내장
       │  [1234, 5678, 9012, ...]
       ▼
  load_llama_model()      ← Q4_K_M 자동 양자화 해제
       │  forward(tokens)
       ▼
  Hidden states [1, seq_len, 768]
       │
       ▼
  EmbeddingExtractor      ← llama-gguf 내장
  (Mean pooling + L2 normalize)
       │  [768]
       ▼
  Matryoshka truncate     → [256] (설정 기준, config.rs)
```

### 5.4 llama-gguf가 처리하는 것

| 기능 | llama-gguf 내장 | Oxios 구현 |
|------|---------------|------------|
| GGUF 파일 파싱 | ✅ | - |
| 토크나이저 | ✅ (GGUF 내장) | - |
| Q4_K_M 양자화 해제 | ✅ | - |
| Gemma 3 forward pass | ✅ | - |
| CPU SIMD (AVX2, NEON) | ✅ | - |
| GPU 가속 (Metal/CUDA/Vulkan) | ✅ (선택적) | - |
| EmbeddingExtractor | ✅ (Mean pool + L2 norm) | - |
| Lazy load + TTL | - | ✅ `GgufEmbeddingProvider` |
| Matryoshka truncate | - | ✅ |
| 모델 다운로드 | - | ✅ `HfClient` |

---

## 6. Kernel 초기화

```rust
// src/kernel.rs — 이미 구현됨

fn create_embedding_provider(config: &OxiosConfig) -> Arc<dyn EmbeddingProvider> {
    match emb_config.provider.as_str() {
        "gguf" => {
            #[cfg(feature = "embedding-gguf")]
            {
                Arc::new(GgufEmbeddingProvider::new(
                    model_dir, dim, emb_config.model_ttl_secs,
                ))
            }
            #[cfg(not(feature = "embedding-gguf"))]
            {
                tracing::warn!("GGUF embedding requested but feature not enabled. Falling back to TF-IDF.");
                Arc::new(TfIdfEmbeddingProvider)
            }
        }
        _ => Arc::new(TfIdfEmbeddingProvider),
    }
}
```

---

## 7. 마이그레이션: 기존 JSON → SQLite

```rust
// memory/migration.rs — 이미 구현됨

pub fn migrate_json_to_sqlite(workspace_dir: &Path, db: &MemoryDatabase) -> Result<MigrationReport> {
    // 1. dream_state에서 migration_v1_complete 키 확인
    // 2. 각 MemoryType별 JSON 파일 읽기
    // 3. SQLite INSERT
    // 4. 원본 JSON 보존 (삭제 안 함)
}
```

---

## 8. Phase별 연결 계획 (모두 구현 완료)

### Phase 1: SQLite + Embedding + 검색 (기반 인프라) ✅

| 작업 | 파일 | 상태 |
|------|------|------|
| DB 초기화 + 스키마 | `memory/database.rs` | ✅ |
| SqliteMemoryStore | `memory/sqlite_store.rs` | ✅ |
| remember/search | `memory/store.rs` (델리게이션) | ✅ |
| KNN + BM25 → RRF | `memory/search/` | ✅ |
| 임베딩 캐시 | `memory/cache.rs` | ✅ |
| GGUF Provider | `embedding/gguf/` | ✅ |
| JSON→SQLite 마이그레이션 | `memory/migration.rs` | ✅ |
| Config | `config.rs` | ✅ |
| Kernel 초기화 | `src/kernel.rs` | ✅ |

### Phase 2: MemoryGraph → Dream 통합 (PageRank) ✅

| 작업 | 파일 | 내용 |
|------|------|------|
| co-access 그래프 빌드 | `sqlite_store.rs` | `build_co_access_graph()` |
| PageRank 계산 | `sqlite_store.rs` | `compute_pagerank()` |
| importance boost | `sqlite_store.rs` | `apply_pagerank_boost()` |
| Dream 시그널 | `dream.rs` | `MemorySignal::PageRankBoost` |
| Dream 적용 | `dream.rs` | `dream_prune_and_index()`에서 UPDATE |

### Phase 3: Proactive Recall → 세션 자동 주입 ✅

| 작업 | 파일 | 내용 |
|------|------|------|
| recall 재작성 | `proactive.rs` | HOT tier + SQLite search |
| list_by_tier | `store.rs` | SQLite 델리게이션 |

### Phase 4: SONA + ReasoningBank (학습 인프라) ✅

| 작업 | 파일 | 내용 |
|------|------|------|
| 패턴 CRUD | `sqlite_store.rs` | `save_pattern()`, `load_patterns()` |
| 패턴 사용 추적 | `sqlite_store.rs` | `record_pattern_usage()` |
| 자동 승격 | `sqlite_store.rs` | `auto_promote_patterns()` |
| SONA 영속화 | `sona.rs` | `persist_to_sqlite()`, `restore_from_sqlite()` |
| ReasoningBank 영속화 | `reasoning_bank.rs` | `persist_to_sqlite()`, `restore_from_sqlite()` |

### Phase 5: Hyperbolic Embedding (계층 인덱싱) ✅

| 작업 | 파일 | 내용 |
|------|------|------|
| SQLite 영속화 | `hyperbolic.rs` | `persist_to_sqlite()`, `restore_from_sqlite()` |
| SQLite에서 빌드 | `hyperbolic.rs` | `build_from_sqlite()` |
| Dream 통합 | `dream.rs` | `dream_prune_and_index()`에서 로드 |

### Phase 6: Flash Attention (Recall 재랭킹) ✅

| 작업 | 파일 | 내용 |
|------|------|------|
| Attention 재랭킹 | `sqlite_store.rs` | `recall_with_rerank()` |
| MemoryManager 위임 | `store.rs` | `recall_with_rerank()` |

### Phase 7: AutoMemoryBridge (외부 동기화) ✅

| 작업 | 파일 | 내용 |
|------|------|------|
| SQLite → MEMORY.md | `auto_memory_bridge.rs` | `sync_sqlite_to_auto()` |
| MEMORY.md → SQLite | `auto_memory_bridge.rs` | `sync_auto_to_sqlite()` |
| 마크다운 파서 | `auto_memory_bridge.rs` | `parse_insights()` |

---

## 9. 의존성 그래프

```
Phase 1: SQLite + GGUF + Search       ← 독립, 최우선   ✅
    │
    ├── Phase 3: Proactive Recall      ← Search 필요     ✅
    │       └── Phase 6: Flash Attn                      ✅
    │
    ├── Phase 2: MemoryGraph           ← Dream에 통합     ✅
    │
    ├── Phase 4: SONA + Reasoning      ← 학습 파이프라인  ✅
    │
    ├── Phase 5: Hyperbolic            ← RootIndex 보강   ✅
    │
    └── Phase 7: AutoMemoryBridge      ← 외부 연동        ✅
```

---

## 10. Config 확장

```toml
# config.toml — memory 섹션

[memory]
enabled = true
max_recall = 10

# SQLite
[memory.sqlite]
enabled = true
path = ""                     # 비워두면 ~/.oxios/workspace/memory.db
wal_mode = true

# Embedding
[memory.embedding]
provider = "gguf"             # "gguf" | "tfidf" (legacy)
dimension = 256               # Matryoshka: 128 | 256 | 512 | 768
model_ttl_secs = 300          # 모델 메모리 상주 시간

# Learning (Phase 4)
[memory.learning]
enabled = true
sona_mode = "balanced"
distill_interval_hours = 6
auto_promote_quality = 0.8

# Bridge (Phase 7)
[memory.bridge]
sync_enabled = false
interval_secs = 3600
```

---

## 11. 데이터 흐름 (전체 구현 완료)

```
                         사용자 메시지
                              │
                    ┌─────────▼──────────┐
                    │    Orchestrator     │
                    │                    │
                    │  ① Hot Context     │ ← list_by_tier(Hot)
                    │                    │
                    │  ② Proactive       │ ← recall_with_rerank()
                    │     Recall (Flash)  │   (BM25+KNN→RRF→Attention)
                    │                    │
                    │  ③ Agent Runtime   │
                    └─────────┬──────────┘
                              │
                    ┌─────────▼──────────┐
                    │  SqliteMemoryStore  │
                    │  memory.db          │
                    │                    │
                    │  remember()        │
                    │    ├ INSERT INTO   │ → memories 테이블
                    │    ├ FTS5 트리거   │ → memories_fts 자동 동기화
                    │    ├ GGUF Dense    │ → memory_vectors (sqlite-vec)
                    │    └ Cache         │ → embedding_cache 테이블
                    │                    │
                    │  search()          │
                    │    ├ Tier 1:       │ → sqlite-vec KNN (Dense cosine)
                    │    ├ Tier 2:       │ → FTS5 BM25 (키워드 + CJK)
                    │    └ RRF Fusion    │ → 최종 결과
                    │                    │
                    │  recall_with_rerank()               │
                    │    ├ standard recall                │
                    │    └ Flash Attention re-ranking    │
                    └─────────┬──────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
         Dream Process   Learning       AutoMemory
         (4-phase)       (SONA/RVF)     Bridge
         • PageRank      • Distill      • SQLite ↔ MD
         • Hyperbolic    • Persist      • Import/Export
         • Decay         • Auto-promote
         • Auto-classify
         • Auto-protect
```

---

## 12. 구현 규모

| 컴포넌트 | 파일 | 줄 수 |
|----------|------|-------|
| `database.rs` | SQLite 스키마 + 초기화 | ~510 |
| `search/mod.rs` | 통합 검색 (KNN+BM25→RRF) | ~300 |
| `search/bm25.rs` | FTS5 BM25 (CJK 지원) | ~150 |
| `search/vector.rs` | sqlite-vec KNN | ~200 |
| `search/rrf.rs` | Reciprocal Rank Fusion | ~90 |
| `cache.rs` | 임베딩 캐시 | ~200 |
| `migration.rs` | JSON → SQLite | ~250 |
| `sqlite_store.rs` | CRUD + 검색 + 패턴 | ~750 |
| `gguf/mod.rs` | GgufEmbeddingProvider | ~320 |
| `gguf/loader.rs` | 모델 다운로드 | ~110 |
| **총 Phase 1** | | **~2,880줄** |

Phase 2–7 추가분: ~500줄 (dream.rs, proactive.rs, sona.rs, reasoning_bank.rs, hyperbolic.rs, auto_memory_bridge.rs, store.rs 수정)

---

## 13. 테스트 결과

97 tests, 0 failures:

| 모듈 | 테스트 수 |
|------|----------|
| sqlite_store | 12 |
| database | 9 |
| search (bm25+vector+rrf+통합) | 17 |
| graph | 6 |
| proactive | 7 |
| dream | 5 |
| decay | 8 |
| sona | 10 |
| hyperbolic | 14 |
| flash_attention | 9 |

---

## 14. 마이그레이션

1. 첫 실행 시 `migration_v1_complete` 키 확인 → 없으면 JSON→SQLite 실행
2. 기존 JSON 데이터는 마이그레이션 후에도 보존 (삭제 안 함)
3. Dense embedding은 마이그레이션 후 백그라운드에서 재계산
4. config 새 필드는 `#[serde(default)]` → 기존 config.toml 그대로 작동
5. `embedding-gguf` feature 없으면 TF-IDF로 동작 (BM25만 사용)

---

## 15. 위험 및 완화

| 위험 | 완화 |
|------|------|
| sqlite-vec pre-v1 breaking change | 래퍼 레이어로 격리, API 변경 시 1파일만 수정 |
| sqlite-vec brute force 느림 | Oxios 메모리 ~1만 개, brute force 1ms 이하 |
| llama-gguf 버전 호환성 | `llama-gguf = "0.14"` 고정, breaking change 시 업데이트 |
| 첫 로드 시 329MB 다운로드 | HfClient 백그라운드 다운로드 + 캐시 |
| Gemma 라이선스 제약 | 상업적 사용 허가. NOTICE 파일에 출처 명시 |
| CI에서 GGUF 테스트 불가 | `sqlite-memory` feature로 CI 통과, SQLite는 모든 플랫폼 작동 |
| SQLite 파일 손상 | WAL mode + ACID. 백업 = 파일 하나 복사 |
| parking_lot::Mutex 데드락 | 테스트에서 conn() 블록 스코프 사용. 함수 호출 전 반드시 drop |
