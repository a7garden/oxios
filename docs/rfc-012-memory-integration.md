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
  EmbeddingGemma (MLX Dense) → sqlite-vec (벡터 검색)
                            → FTS5 (BM25 키워드 검색)
                            → SQLite 단일 파일 (ACID, 백업 = 파일 하나)
```

---

## 1. 설계 원칙

1. **MLX-First**: Apple Silicon + Metal GPU가 기본. TF-IDF는 제거.
2. **SQLite 단일 파일**: 모든 메모리 데이터가 하나의 `.db` 파일에 들어간다.
3. **Lazy Loading**: 임베딩 모델 173MB는 필요할 때만 로드, 유휴 시 해제.
4. **Pure Rust**: Python 없이 mlx-rs + rusqlite + sqlite-vec로 직접 구현.
5. **점진적 연결**: Feature flag로 단계 도입. 기존 API 변경 없음.
6. **Zero-maintenance**: RFC-008 원칙 유지. 사용자가 신경 쓸 것 없음.

---

## 2. 임베딩 모델: EmbeddingGemma-300m

### 2.1 선택 근거

| 항목 | 값 |
|------|-----|
| **사용 모델** | `mlx-community/embeddinggemma-300m-4bit` |
| **원본 모델** | `google/embeddinggemma-300m` |
| **아키텍처** | Gemma 3 Text (`Gemma3TextModel`) |
| **Q4 디스크** | 173MB (model.safetensors) |
| **차원** | 768 (Matryoshka: 128 / 256 / 512 / 768) |
| **최대 입력** | 2048 tokens |
| **어텐션** | 양방향 (`use_bidirectional_attention: true`) |
| **언어** | 100+ (한국어 포함) |
| **MTEB 다국어** | 60.62 (Q4_0) |
| **라이선스** | Gemma Terms of Use (상업적 사용 허가, 재배포 시 terms 포함 필수) |

### 2.2 라이선스 상세

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

### 2.3 실제 스펙

`mlx-community/embeddinggemma-300m-4bit`의 `config.json` 기준:

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
hidden_activation:            gelu_pytorch_tanh
sliding_window:               512
query_pre_attn_scalar:        256   (= head_dim)
rope_theta:                   1000000.0
rms_norm_eps:                 1e-6
quantization:                 { group_size: 64, bits: 4 }
layer_types:                  [sliding_attention × 5, full_attention, ...]
                              → 6번째, 12번째, 18번째, 24번째 레이어만 full attention
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
    id          TEXT PRIMARY KEY,           -- UUID
    memory_type TEXT NOT NULL,              -- fact, episode, knowledge, ...
    content     TEXT NOT NULL,              -- 원본 텍스트
    summary     TEXT,                       -- 요약 (있으면)
    importance  REAL NOT NULL DEFAULT 0.5,  -- 0.0 ~ 1.0
    tier        TEXT NOT NULL DEFAULT 'warm', -- hot, warm, cold
    protection  TEXT NOT NULL DEFAULT 'none', -- none, low, medium, high
    session_id  TEXT,                       -- 생성된 세션
    space_id    TEXT,                       -- 소속 스페이스
    metadata    TEXT,                       -- JSON (tags, source, etc.)
    access_count INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,              -- ISO 8601
    updated_at  TEXT NOT NULL,
    accessed_at TEXT,                       -- 마지막 접근 시간
    decay_rate  REAL NOT NULL DEFAULT 0.01
);

-- 타입별 조회
CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(memory_type);
-- 세션별 조회
CREATE INDEX IF NOT EXISTS idx_memories_session ON memories(session_id);
-- 중요도순 정렬
CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance);

-- ─────────────────────────────────────────────
-- 2. FTS5 전문 검색 (BM25)
-- ─────────────────────────────────────────────
-- content="unicode61" → CJK/한국어 유니코드 토크나이제이션 내장
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    id,
    content,
    summary,
    memory_type,
    content='memories',
    content_rowid='rowid',
    tokenize="unicode61"
);

-- FTS와 memories 테이블 동기화 트리거
CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, id, content, summary, memory_type)
    VALUES (new.rowid, new.id, new.content, new.summary, new.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, id, content, summary, memory_type)
    VALUES ('delete', old.rowid, old.id, old.content, old.summary, old.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, id, content, summary, memory_type)
    VALUES ('delete', old.rowid, old.id, old.content, old.summary, old.memory_type);
    INSERT INTO memories_fts(rowid, id, content, summary, memory_type)
    VALUES (new.rowid, new.id, new.content, new.summary, new.memory_type);
END;

-- ─────────────────────────────────────────────
-- 3. 벡터 저장 (sqlite-vec)
-- ─────────────────────────────────────────────
-- EmbeddingGemma 768d (또는 Matryoshka 128/256)
-- sqlite-vec은 가상 테이블로 벡터 KNN 검색 제공
CREATE VIRTUAL TABLE IF NOT EXISTS memory_vectors USING vec0(
    embedding float[768]   -- Matryoshka 128/256 쓸 경우 float[128] 또는 float[256]
);

-- ─────────────────────────────────────────────
-- 4. 임베딩 캐시 (같은 텍스트 재임베딩 방지)
-- ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS embedding_cache (
    content_hash TEXT PRIMARY KEY,           -- 텍스트 해시
    embedding    BLOB NOT NULL,             -- f32 벡터 (768 × 4 bytes = 3KB)
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
    embedding    BLOB,                       -- 패턴 임베딩 (선택적)
    data         TEXT NOT NULL,              -- JSON (전체 패턴 데이터)
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
```

### 3.3 전체 구조

```
crates/oxios-kernel/src/
├── embedding/
│   ├── mod.rs                  # EmbeddingProvider trait (기존)
│   ├── mlx/
│   │   ├── mod.rs              # MlxEmbeddingProvider (lazy load)
│   │   ├── gemma.rs            # Gemma 3 encoder model
│   │   ├── loader.rs           # Safetensors + tokenizer loader
│   │   └── pooler.rs           # Mean pooling + L2 normalize
│   └── tfidf.rs                # TfIdfEmbeddingProvider (legacy, feature-gated)
│
├── memory/
│   ├── mod.rs                  # MemoryManager (확장)
│   ├── store.rs                # remember/search → SQLite 기반으로 재작성
│   ├── search/
│   │   ├── mod.rs              # 통합 검색 인터페이스
│   │   ├── vector.rs           # sqlite-vec KNN 검색
│   │   ├── bm25.rs             # FTS5 BM25 검색
│   │   └── rrf.rs              # Reciprocal Rank Fusion
│   ├── migration.rs            # 기존 JSON → SQLite 마이그레이션
│   ├── database.rs             # SQLite 연결 + 스키마 초기화
│   ├── cache.rs                # 임베딩 캐시 (SQLite 기반)
│   │
│   │  ── 기존 모듈 (연결 예정) ──
│   ├── dream.rs                # DreamProcess
│   ├── decay.rs                # DecayEngine
│   ├── auto_classify.rs        # AutoClassifier
│   ├── auto_protect.rs         # AutoProtector
│   ├── proactive.rs            # ProactiveRecall
│   ├── graph.rs                # MemoryGraph
│   ├── root_index.rs           # RootIndex
│   ├── hyperbolic.rs           # HyperbolicEmbedding
│   ├── flash_attention.rs      # FlashAttention
│   ├── sona.rs                 # SonaEngine
│   ├── reasoning_bank.rs       # ReasoningBank
│   ├── rvf_store.rs            # RvfLearningStore
│   ├── auto_memory_bridge.rs   # AutoMemoryBridge
│   ├── compaction.rs           # CompactionTree
│   ├── hnsw.rs                 # 기존 HNSW (legacy, 제거 예정)
│   ├── embedding_cache.rs      # 기존 캐시 (SQLite로 대체)
│   └── subsystems.rs           # MemorySubsystems container
```

### 3.4 의존성

```toml
# crates/oxios-kernel/Cargo.toml

[dependencies]
rusqlite = { version = "0.34", features = ["bundled"] }
sqlite-vec = "0.1"

[target.'cfg(target_arch = "aarch64")'.dependencies]
mlx-rs = { version = "0.25", optional = true }
tokenizers = { version = "0.21", optional = true }

[features]
default = ["embedding-mlx"]
embedding-mlx = ["mlx-rs", "tokenizers"]
embedding-tfidf = []                          # Zero-dependency legacy
```

---

## 4. SQLite MemoryDatabase

### 4.1 초기화

```rust
// memory/database.rs

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// SQLite-backed memory database.
/// Single file: ~/.oxios/workspace/memory.db
pub struct MemoryDatabase {
    conn: Mutex<Connection>,
    /// Embedding dimension (768, 256, or 128)
    embedding_dim: usize,
}

impl MemoryDatabase {
    /// Open (or create) the memory database at the given path.
    pub fn open(db_path: &Path, embedding_dim: usize) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode for concurrent reads
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")?;

        // Load sqlite-vec extension
        sqlite_vec::load(&conn)?;

        // Initialize schema
        conn.execute_batch(SCHEMA)?;

        Ok(Self {
            conn: Mutex::new(conn),
            embedding_dim,
        })
    }

    /// Backup = copy one file.
    pub fn backup(&self, backup_path: &Path) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(&format!(
            "VACUUM INTO '{}';",
            backup_path.display()
        ))?;
        Ok(())
    }
}
```

### 4.2 쓰기: remember()

```rust
// memory/store.rs

impl MemoryManager {
    /// Store a memory entry. Returns the entry ID.
    ///
    /// 1. Insert into `memories` table
    /// 2. FTS5 trigger automatically syncs `memories_fts`
    /// 3. Compute dense embedding → insert into `memory_vectors`
    /// 4. Cache the embedding in `embedding_cache`
    pub async fn remember(
        &self,
        memory_type: MemoryType,
        content: &str,
        importance: f32,
        session_id: Option<&str>,
    ) -> Result<MemoryEntry> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        // 1. Insert into memories table
        {
            let conn = self.db.conn.lock();
            conn.execute(
                "INSERT INTO memories (id, memory_type, content, importance, tier, session_id, created_at, updated_at, decay_rate)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    id,
                    memory_type.label(),
                    content,
                    importance,
                    memory_type.initial_tier().label(),
                    session_id,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    memory_type.base_decay_rate(),
                ],
            )?;
        }

        // 2. Compute dense embedding (lazy-loaded MLX model)
        if let Some(dense_vec) = self.embedding_provider.embed_dense(content).await? {
            let row_id = {
                let conn = self.db.conn.lock();
                let mut stmt = conn.prepare("SELECT rowid FROM memories WHERE id = ?1")?;
                stmt.query_row(rusqlite::params![id], |row| row.get::<_, i64>(0))?
            };

            // 3. Insert into sqlite-vec
            {
                let conn = self.db.conn.lock();
                let vec_bytes = f32_slice_to_bytes(&dense_vec);
                conn.execute(
                    "INSERT INTO memory_vectors (rowid, embedding) VALUES (?1, ?2)",
                    rusqlite::params![row_id, vec_bytes],
                )?;
            }

            // 4. Cache embedding
            self.cache_embedding(content, &dense_vec)?;
        }

        // Build and return the entry
        Ok(MemoryEntry { id, memory_type, content, importance, .. })
    }
}
```

### 4.3 검색: search()

```rust
// memory/search/mod.rs

/// Unified search: sqlite-vec KNN + FTS5 BM25 → RRF fusion.
pub async fn search(
    db: &MemoryDatabase,
    embedding_provider: &MlxEmbeddingProvider,
    query: &str,
    limit: usize,
) -> Result<Vec<RankedMemory>> {
    let mut tier_results: Vec<Vec<(i64, f64)>> = Vec::new();

    // ── Tier 1: sqlite-vec Dense KNN ──
    if let Some(query_vec) = embedding_provider.embed_dense(query).await? {
        let conn = db.conn.lock();
        let query_bytes = f32_slice_to_bytes(&query_vec);
        let mut stmt = conn.prepare(
            "SELECT rowid, distance
             FROM memory_vectors
             WHERE embedding MATCH ?1
             ORDER BY distance
             LIMIT ?2"
        )?;
        let rows = stmt.query_map(rusqlite::params![query_bytes, limit * 2], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
        })?;
        let vec_results: Vec<(i64, f64)> = rows.filter_map(|r| r.ok()).collect();
        tier_results.push(vec_results);
    }

    // ── Tier 2: FTS5 BM25 ──
    {
        let conn = db.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT rowid, bm25(memories_fts) AS score
             FROM memories_fts
             WHERE memories_fts MATCH ?1
             ORDER BY score
             LIMIT ?2"
        )?;
        let rows = stmt.query_map(rusqlite::params![query, limit * 2], |row| {
            Ok((row.get::<_, i64>(0)?, -row.get::<_, f64>(1)?)) // BM25 음수 → 양수로
        })?;
        let bm25_results: Vec<(i64, f64)> = rows.filter_map(|r| r.ok()).collect();
        tier_results.push(bm25_results);
    }

    // ── RRF Fusion ──
    let fused = reciprocal_rank_fusion(tier_results, 60.0);

    // ── Load memory entries by rowid ──
    let mut results = Vec::new();
    for (rowid, score) in fused.into_iter().take(limit) {
        if let Some(entry) = load_memory_by_rowid(db, rowid)? {
            results.push(RankedMemory { entry, score });
        }
    }

    Ok(results)
}
```

### 4.4 RRF (Reciprocal Rank Fusion)

```rust
// memory/search/rrf.rs

use std::collections::HashMap;

/// Reciprocal Rank Fusion으로 여러 검색 결과를 병합한다.
///
/// K=60이 표준값. 각 tier의 rank 위치로 점수를 계산하여 합산.
pub fn reciprocal_rank_fusion(
    results: Vec<Vec<(i64, f64)>>,
    k: f64,
) -> Vec<(i64, f64)> {
    let mut scores: HashMap<i64, f64> = HashMap::new();

    for tier_results in &results {
        for (rank, (id, _)) in tier_results.iter().enumerate() {
            *scores.entry(*id).or_default() += 1.0 / (k + rank as f64 + 1.0);
        }
    }

    let mut ranked: Vec<_> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    ranked
}
```

---

## 5. EmbeddingGemma MLX 구현

### 5.1 Lazy Embedding Provider

```rust
// embedding/mlx/mod.rs

/// Lazy-loaded MLX embedding model.
///
/// Lifecycle:
///   1. 처음 embed() 호출 시 모델 로드 (~1-2초)
///   2. 로드 후 메모리에 상주
///   3. TTL 동안 사용 없으면 자동 해제
///   4. 다시 호출하면 다시 로드
pub struct MlxEmbeddingProvider {
    model_dir: PathBuf,
    dimension: EmbeddingDimension,
    query_prefix: String,     // "task: search result | query: "
    doc_prefix: String,       // "title: none | text: "
    inner: Mutex<Option<LoadedModel>>,
    ttl: Duration,
    last_used: Mutex<Instant>,
}

/// Matryoshka dimension truncation.
#[derive(Debug, Clone, Copy)]
pub enum EmbeddingDimension {
    Dim128,   // HNSW 메모리 최소, 품질 약간 저하
    Dim256,   // 균형점 (권장)
    Dim512,
    Dim768,   // 풀 차원
}

impl MlxEmbeddingProvider {
    /// Tokenize + forward + mean pool + L2 normalize + Matryoshka truncate.
    fn encode(&self, text: &str, prefix: &str) -> Result<Vec<f32>> {
        let inner = self.inner.lock();
        let loaded = inner.as_ref().ok_or_else(|| anyhow::anyhow!("Model not loaded"))?;

        let input = format!("{}{}", prefix, text);

        // Tokenize
        let encoding = loaded.tokenizer.encode(input, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let ids = encoding.get_ids();
        let input_ids = Array::from(ids).unsqueeze(0)?;

        // Attention mask (no padding for single input)
        let mask = Array::ones::<f32>(&[1, 1, 1, ids.len()])?;

        // Forward pass (bidirectional Gemma 3)
        let hidden = loaded.model.forward(&input_ids, &mask)?;

        // Mean pooling + L2 normalize
        let attn_mask = Array::ones::<f32>(&[1, ids.len()])?;
        let pooled = mean_pool(&hidden, &attn_mask);
        let normalized = l2_normalize(&pooled);

        // Matryoshka truncation
        let dim = self.dimension.size();
        let truncated = normalized.slice(&[0..1, 0..dim])?;

        mlx_rs::transforms::eval([&truncated])?;
        Ok(truncated.to_vec()?)
    }

    /// Unload model if TTL expired. Called periodically.
    pub fn maybe_unload(&self) {
        if self.last_used.lock().elapsed() > self.ttl {
            *self.inner.lock() = None;
            tracing::debug!("MLX embedding model unloaded (TTL expired)");
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingProvider for MlxEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<EmbeddingVector> {
        self.ensure_loaded()?;
        let vec = self.encode(text, &self.query_prefix)?;
        Ok(EmbeddingVector::DenseF32(vec))
    }

    fn name(&self) -> &str { "mlx-embeddinggemma-300m" }
}
```

### 5.2 Gemma 3 모델

```rust
// embedding/mlx/gemma.rs

/// Gemma 3 Transformer block.
pub struct GemmaBlock {
    self_attn: GemmaAttention,
    mlp: GemmaMlp,
    input_layernorm: nn::RmsNorm,
    post_attention_layernorm: nn::RmsNorm,
    layer_type: LayerType,
}

/// Layer attention type (from config.layer_types).
pub enum LayerType {
    SlidingAttention,  // window=512
    FullAttention,     // full sequence
}

/// Full Gemma 3 embedding model.
pub struct GemmaEmbeddingModel {
    config: GemmaConfig,
    embed_tokens: nn::Embedding,
    layers: Vec<GemmaBlock>,
    norm: nn::RmsNorm,
}

impl GemmaEmbeddingModel {
    /// Forward: input_ids → hidden states.
    ///
    /// Gemma 3 특이사항:
    /// 1. h = embed(tokens) * sqrt(768)  ← Gemma 전용
    /// 2. bidirectional attention
    /// 3. Mixed sliding/full layers
    /// 4. scale = 1/query_pre_attn_scalar (1/256)
    pub fn forward(&self, input_ids: &Array, attention_mask: &Array) -> Result<Array, Exception> {
        let scale = (self.config.hidden_size as f32).sqrt(); // sqrt(768) ≈ 27.7
        let mut h = self.embed_tokens.forward(input_ids)? * Array::from(scale);

        for layer in &self.layers {
            let normed = layer.input_layernorm.forward(&h)?;

            let mask = match layer.layer_type {
                LayerType::SlidingAttention => {
                    build_sliding_mask(attention_mask, self.config.sliding_window)?
                }
                LayerType::FullAttention => {
                    build_padding_mask(attention_mask)?
                }
            };

            let attn_out = layer.self_attn.forward(&normed, &mask)?;
            h = h.add(&attn_out)?;

            let normed = layer.post_attention_layernorm.forward(&h)?;
            let mlp_out = layer.mlp.forward(&normed)?;
            h = h.add(&mlp_out)?;
        }

        self.norm.forward(&h)
    }
}

/// Bidirectional GQA attention.
/// scale = 1/query_pre_attn_scalar (1/256), NOT standard 1/sqrt(head_dim).
impl GemmaAttention {
    pub fn forward(&self, x: &Array, mask: &Array) -> Result<Array, Exception> {
        // Q/K/V projections + reshape
        // GQA repeat (n_kv_heads=1 → n_heads=3)
        // Apply RoPE (theta=1M)
        // Scaled dot-product: scale = 1/256
        // Apply mask (sliding window or full bidirectional)
        // Softmax + matmul with V
        // Reshape + output projection
        // ... (구현 상세는 mlx-rs LLaMA 참고, ~80줄)
    }
}
```

### 5.3 모델 로더

```rust
// embedding/mlx/loader.rs

/// Source: https://huggingface.co/mlx-community/embeddinggemma-300m-4bit
/// Files: model.safetensors (173MB), config.json, tokenizer.json, etc.
impl MlxModelLoader {
    pub fn ensure_model(model_dir: &Path) -> Result<()> {
        if model_dir.join("model.safetensors").exists() {
            return Ok(());
        }
        // Download via hf-hub
        let api = hf_hub::api::sync::ApiBuilder::new()
            .with_cache_dir(model_dir.parent().unwrap().to_path_buf())
            .build()?;
        let repo = api.model("mlx-community/embeddinggemma-300m-4bit".to_string());
        for filename in &["model.safetensors", "config.json", "tokenizer.json",
                          "tokenizer.model", "tokenizer_config.json",
                          "special_tokens_map.json", "added_tokens.json"] {
            let _ = repo.get(filename)?;
        }
        Ok(())
    }
}
```

---

## 6. Kernel 초기화

```rust
// src/kernel.rs — KernelBuilder::build()

// 1. SQLite database
let db_path = PathBuf::from(&config.kernel.workspace).join("memory.db");
let embedding_dim = config.memory.embedding.dimension; // 256 (권장)
let db = Arc::new(MemoryDatabase::open(&db_path, embedding_dim)?);

// 2. MLX embedding provider (lazy load)
let embedding_provider = Arc::new(MlxEmbeddingProvider::new(
    PathBuf::from(&config.kernel.workspace).join("models").join("embeddinggemma-300m-4bit"),
    match embedding_dim {
        128 => EmbeddingDimension::Dim128,
        256 => EmbeddingDimension::Dim256,
        512 => EmbeddingDimension::Dim512,
        _ => EmbeddingDimension::Dim768,
    },
));

// 3. MemoryManager with SQLite backend
let memory_manager = Arc::new(MemoryManager::new(db, embedding_provider));

// 4. Periodic MLX model unload check
let mlx = embedding_provider.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        mlx.maybe_unload();
    }
});
```

---

## 7. 마이그레이션: 기존 JSON → SQLite

```rust
// memory/migration.rs

/// One-time migration from JSON StateStore to SQLite.
/// Runs automatically on first launch after upgrade.
pub fn migrate_json_to_sqlite(
    workspace_dir: &Path,
    db: &MemoryDatabase,
) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();

    for mt in MemoryType::all() {
        let category_dir = workspace_dir.join(mt.category());
        if !category_dir.exists() {
            continue;
        }

        for entry in std::fs::read_dir(&category_dir)? {
            let path = entry?.path();
            if path.extension() == Some(OsStr::new("json")) {
                let json_str = std::fs::read_to_string(&path)?;
                if let Ok(mem) = serde_json::from_str::<MemoryEntry>(&json_str) {
                    // Insert into SQLite
                    db.insert_memory(&mem)?;
                    report.migrated += 1;

                    // Compute and store dense embedding (best effort)
                    // Will be done in background after migration
                }
            }
        }
    }

    // Mark migration as complete
    db.set_dream_state("migration_v1_complete", "true")?;

    tracing::info!(
        migrated = report.migrated,
        "JSON → SQLite migration complete"
    );

    Ok(report)
}
```

---

## 8. Phase별 연결 계획

### Phase 1: SQLite + Embedding + 검색 (기반 인프라)

**목표**: SQLite 단일 파일에 메모리 저장 + Dense 벡터 + BM25 검색이 작동한다.

| 작업 | 파일 | 내용 |
|------|------|------|
| DB 초기화 + 스키마 | `memory/database.rs` | SQLite + sqlite-vec + FTS5 |
| remember() 재작성 | `memory/store.rs` | SQLite INSERT + FTS5 + sqlite-vec |
| search() 재작성 | `memory/search/mod.rs` | KNN + BM25 → RRF |
| BM25 검색 | `memory/search/bm25.rs` | FTS5 쿼리 래퍼 |
| 벡터 검색 | `memory/search/vector.rs` | sqlite-vec KNN 래퍼 |
| RRF | `memory/search/rrf.rs` | Reciprocal Rank Fusion |
| 임베딩 캐시 | `memory/cache.rs` | SQLite embedding_cache 테이블 |
| Gemma 모델 | `embedding/mlx/gemma.rs` | Gemma 3 24-layer encoder |
| Lazy provider | `embedding/mlx/mod.rs` | MlxEmbeddingProvider |
| 모델 로더 | `embedding/mlx/loader.rs` | hf-hub 다운로드 + safetensors |
| Pooling | `embedding/mlx/pooler.rs` | Mean pool + L2 norm |
| JSON→SQLite 마이그레이션 | `memory/migration.rs` | 기존 데이터 이관 |
| Config | `config.rs` | embedding.dimension, sqlite.path 등 |
| Feature flags | `Cargo.toml` | embedding-mlx, rusqlite, sqlite-vec |
| Kernel 초기화 | `src/kernel.rs` | DB + provider wiring |

**완료 기준**:
- `remember("한국어 테스트")` → SQLite INSERT + FTS5 + sqlite-vec insert
- `search("테스트")` → sqlite-vec KNN + FTS5 BM25 → RRF 결과 반환
- 첫 호출 시 ~1-2초 (모델 로드), 이후 ~5-15ms
- 5분 사용 없으면 모델 자동 해제
- 기존 JSON 데이터 자동 마이그레이션

### Phase 2: MemoryGraph → Dream 통합

| 작업 | 파일 | 내용 |
|------|------|------|
| graph wiring | `memory/dream.rs` | Phase 2에서 co-access 그래프 → PageRank |
| decay 확장 | `memory/decay.rs` | PageRank boost 반영 |

### Phase 3: Proactive Recall → 세션 자동 주입

| 작업 | 파일 | 내용 |
|------|------|------|
| recall wiring | `orchestrator.rs` | 세션 시작/토픽 전환 시 recall |
| hot context | `orchestrator.rs` | Hot Tier 자동 주입 |

### Phase 4: SONA + ReasoningBank + RVF (학습 인프라)

| 작업 | 파일 | 내용 |
|------|------|------|
| patterns 테이블 활용 | `memory/subsystems.rs` | SQLite에 패턴 저장 |
| 궤적 기록 | `agent_runtime.rs` | SONA에 전달 |
| Dream 통합 | `memory/dream.rs` | Distill + auto-promote |

### Phase 5: Hyperbolic Embedding (계층 인덱싱)

### Phase 6: Flash Attention (Recall 재랭킹)

### Phase 7: AutoMemoryBridge (외부 동기화)

**권장 구현 순서**: 1 → 2 → 3 → 4 → 5 → 6 → 7

---

## 9. 의존성 그래프

```
Phase 1: SQLite + Embedding + Search    ← 독립, 최우선
    │
    ├── Phase 3: Proactive Recall       ← Search 필요
    │       └── Phase 6: Flash Attn
    │
    ├── Phase 2: MemoryGraph            ← Dream에 통합
    │
    ├── Phase 4: SONA + Reasoning       ← 학습 파이프라인
    │
    ├── Phase 5: Hyperbolic             ← RootIndex 보강
    │
    └── Phase 7: AutoMemoryBridge       ← 외부 연동
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
path = ""                     # 비워두면 ~/.oxios/workspace/memory.db
wal_mode = true

# Embedding
[memory.embedding]
provider = "mlx"              # "mlx" | "tfidf" (legacy)
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

## 11. 데이터 흐름 (Phase 1 완성 후)

```
                         사용자 메시지
                              │
                    ┌─────────▼──────────┐
                    │    Orchestrator     │
                    │                    │
                    │  ① Hot Context     │ ← memories WHERE tier='hot'
                    │                    │
                    │  ② Proactive       │ ← search() 자동 호출
                    │     Recall         │
                    │                    │
                    │  ③ Agent Runtime   │
                    └─────────┬──────────┘
                              │
                    ┌─────────▼──────────┐
                    │  MemoryManager     │
                    │                    │
                    │  remember()        │
                    │    ├ INSERT INTO   │ → memories 테이블
                    │    ├ FTS5 트리거   │ → memories_fts 자동 동기화
                    │    ├ Gemma Dense   │ → memory_vectors (sqlite-vec)
                    │    └ Cache         │ → embedding_cache 테이블
                    │                    │
                    │  search()          │
                    │    ├ Tier 1:       │ → sqlite-vec KNN (Dense cosine)
                    │    │  SELECT FROM  │
                    │    │  memory_vectors│
                    │    │  WHERE MATCH  │
                    │    │  ORDER BY dist │
                    │    │               │
                    │    ├ Tier 2:       │ → FTS5 BM25 (키워드)
                    │    │  SELECT FROM  │
                    │    │  memories_fts │
                    │    │  WHERE MATCH  │
                    │    │  ORDER BY bm25│
                    │    │               │
                    │    └ RRF Fusion    │ → 최종 결과
                    └─────────┬──────────┘
                              │
                    ┌─────────▼──────────┐
                    │  SQLite 단일 파일   │ ← ~/.oxios/workspace/memory.db
                    │                    │
                    │  memories          │ ← 엔트리
                    │  memories_fts      │ ← BM25 전문 인덱스
                    │  memory_vectors    │ ← 벡터 KNN 인덱스
                    │  embedding_cache   │ ← 임베딩 캐시
                    │  dream_state       │ ← Dream 영속 상태
                    │  patterns          │ ← 학습 패턴
                    └────────────────────┘
```

---

## 12. 모델 구현 상세

### 12.1 GGUF 추론 파이프라인

`llama-gguf` 크레이트가 모든 것을 처리한다. Oxios에서 구현할 것은 `GgufEmbeddingProvider` 래퍼뿐이다.

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
  Mean pooling            ← 마지막 토큰 또는 전체 평균
       │  [768]
       ▼
  L2 normalize
       │  [768]
       ▼
  Matryoshka truncate     → [256] (설정 기준)
```

### 12.2 llama-gguf가 처리하는 것

| 기능 | llama-gguf 내장 | Oxios 구현 |
|------|---------------|------------|
| GGUF 파일 파싱 | ✅ | - |
| 토크나이저 | ✅ (GGUF 내장) | - |
| Q4_K_M 양자화 해제 | ✅ | - |
| Gemma 3 forward pass | ✅ | - |
| CPU SIMD (AVX2, NEON) | ✅ | - |
| GPU 가속 | ✅ (선택적) | - |
| Lazy load + TTL | - | ✅ `GgufEmbeddingProvider` |
| Mean pooling | - | ✅ (또는 llama-gguf embed API) |
| Matryoshka truncate | - | ✅ |
| 모델 다운로드 | - | ✅ `hf-hub` |

### 12.3 구현 추정치

| 컴포넌트 | 줄 수 | 복잡도 |
|----------|-------|--------|
| `database.rs` (SQLite 초기화) | ~80 | 낮음 |
| `store.rs` (remember/search 재작성) | ~200 | 중간 |
| `search/bm25.rs` | ~40 | 낮음 |
| `search/vector.rs` | ~50 | 낮음 |
| `search/rrf.rs` | ~30 | 낮음 |
| `cache.rs` (SQLite 기반) | ~60 | 낮음 |
| `migration.rs` | ~80 | 낮음 |
| `gguf/mod.rs` (provider) | ~120 | 낮음 |
| `gguf/loader.rs` | ~80 | 낮음 |
| `kernel.rs` 수정 | ~40 | 낮음 |
| `config.rs` 수정 | ~40 | 낮음 |
| **총 Phase 1** | **~820줄** | |

> MLX 대비 ~390줄 감소 (Gemma 3 직접 포팅 불필요)

---

## 13. 테스트 전략

### Phase 1

- `test_db_schema_init`: DB 열면 모든 테이블/인덱스/트리거 존재
- `test_remember_inserts_all`: remember() → memories + FTS5 + memory_vectors 행 존재
- `test_fts5_korean`: FTS5로 한국어 검색 결과 반환
- `test_sqlite_vec_knn`: KNN이 코사인 유사도 순으로 정렬
- `test_rrf_fusion`: 두 tier 결과가 RRF로 병합
- `test_embedding_cache_hit`: 같은 텍스트 두 번째 임베딩 시 캐시에서 로드
- `test_lazy_load_unload`: 첫 호출 시 로드, TTL 후 해제
- `test_matryoshka_truncation`: 128/256 차원 잘림 확인
- `test_migration_json_to_sqlite`: 기존 JSON 파일이 SQLite로 이관
- `test_backup_single_file`: VACUUM INTO로 단일 파일 백업

---

## 14. 마이그레이션

1. 첫 실행 시 `migration_v1_complete` 키 확인 → 없으면 JSON→SQLite 실행
2. 기존 JSON 데이터는 마이그레이션 후에도 보존 (삭제 안 함)
3. Dense embedding은 마이그레이션 후 백그라운드에서 재계산
4. config 새 필드는 `#[serde(default)]` → 기존 config.toml 그대로 작동
5. `embedding-mlx` feature 없으면 MLX 없이 동작 (sqlite-vec만 사용, embedding 없이 BM25만)

---

## 15. 위험 및 완화

| 위험 | 완화 |
|------|------|
| sqlite-vec pre-v1 breaking change | 래퍼 레이어로 격리, API 변경 시 1파일만 수정 |
| sqlite-vec brute force 느림 | Oxios 메모리 ~1만 개, brute force 1ms 이하. 정 필요하면 usearch 병행 |
| mlx-rs에 Gemma 모델 없음 | LLaMA 구현 참고 직접 포팅 (~350줄) |
| Safetensors Q4 로딩 | mlx-rs `load_safetensors` + Quantized 지원 확인 |
| 첫 로드 시 173MB 다운로드 | hf-hub 백그라운드 다운로드 + 진행 표시 |
| Gemma 라이선스 제약 | 상업적 사용 허가. NOTICE 파일에 출처 명시 |
| CI에서 MLX 테스트 불가 | `embedding-tfidf` feature로 CI 통과, SQLite는 모든 플랫폼 작동 |
| SQLite 파일 손상 | WAL mode + ACID. 백업 = 파일 하나 복사 |
