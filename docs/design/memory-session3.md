# Memory System — Session 3 작업 지시서 (Optional)

> **목적**: Polish + Advanced Features
> **前提**: Session 1 + Session 2 완료
> **Status**: Optional — 필요시 진행
> **순서**: 아래 번호순으로 진행

---

## 작업 순서 (번호 순)

### 1. memory/flash_attention.rs 생성

**Purpose**: Block-wise attention, O(N²) → O(N) memory

**Algorithm**: Flash Attention (Triton-inspired CPU implementation)

```rust
pub struct FlashAttention {
    block_size: usize,
    dimensions: usize,
}

impl FlashAttention {
    pub fn attention(
        &self,
        queries: &[Vec<f32>],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Vec<Vec<f32>> {
        // Block-wise computation for L1 cache efficiency
        // O(N) memory instead of O(N²)
    }
    
    pub fn benchmark(&self, vectors: &[Vec<f32>]) -> BenchmarkResult {
        // naive vs flash comparison
    }
}

pub struct BenchmarkResult {
    naive_time_ms: f64,
    flash_time_ms: f64,
    speedup: f64,
    memory_reduction: f64,
}
```

**Performance Target**: 2-5x speedup, 75% memory reduction

### 2. memory/hyperbolic.rs 생성

**Purpose**: Poincaré ball model for hierarchical embeddings

```rust
pub mod hyperbolic {
    /// Convert Euclidean to Poincaré ball.
    pub fn euclidean_to_poincare(vector: &[f32], curvature: f32) -> Vec<f32>
    
    /// Compute hyperbolic distance.
    pub fn hyperbolic_distance(a: &[f32], b: &[f32]) -> f32
    
    /// Möbius addition.
    pub fn mobius_add(a: &[f32], b: &[f32], c: f32) -> Vec<f32>
    
    /// Möbius scalar multiplication.
    pub fn mobius_scalar_mul(scalar: f32, v: &[f32], c: f32) -> Vec<f32>
    
    /// Batch conversion.
    pub fn batch_euclidean_to_poincare(vectors: &[Vec<f32>], curvature: f32) -> Vec<Vec<f32>>
}

pub struct HyperbolicConfig {
    curvature: f32,  // default: -1.0
    dimensions: usize,
    epsilon: f32,     // for numerical stability
}
```

**Use Case**: 계층적 데이터 (persona hierarchy, skill graph) 표현

### 3. memory/auto_memory_bridge.rs 생성

**Purpose**: Claude Code auto-memory ↔ Oxios memory synchronization

**Sync Direction**:
- `to-auto`: Oxios patterns → Claude Code MEMORY.md
- `from-auto`: Claude Code memories → Oxios MemoryStore
- `bidirectional`: 양방향 동기화

```rust
pub struct AutoMemoryBridge {
    auto_memory_dir: PathBuf,
    oxios_memory: Arc<MemoryManager>,
}

pub enum SyncDirection { ToAuto, FromAuto, Bidirectional }

pub struct MemoryInsight {
    category: InsightCategory,  // "project-patterns", "debugging", "architecture"
    summary: String,
    detail: Option<String>,
    source: String,
    confidence: f32,
}

impl AutoMemoryBridge {
    /// Import Claude Code memories to Oxios.
    pub async fn import_from_auto(&self) -> Result<ImportResult> {
        // 1. Read MEMORY.md files from ~/.claude/projects/
        // 2. Parse entries
        // 3. Store via MemoryManager
    }
    
    /// Export Oxios patterns to Claude Code format.
    pub async fn export_to_auto(&self, patterns: &[GuidancePattern]) -> Result<ExportResult> {
        // 1. Convert patterns to Markdown
        // 2. Update MEMORY.md with confidence-sorted entries
        // 3. Update topic files (patterns.md, debugging.md, etc.)
    }
    
    /// Sync on session end.
    pub async fn sync_session(&self, direction: SyncDirection) -> Result<SyncResult>
}
```

### 4. memory/graph.rs — Hyperbolic Embedding Integration

```rust
impl MemoryGraph {
    /// Add edge with hyperbolic distance.
    pub fn add_hyperbolic_edge(&mut self, from: &str, to: &str) {
        // Compute hyperbolic distance
        // Add as "hierarchical" edge type
    }
    
    /// Rank results with hyperbolic boost.
    pub fn rank_results_hyperbolic(
        &self,
        hnsw_results: Vec<(String, f64)>,
        alpha: f32,
        beta: f32,
        gamma: f32,
    ) -> Vec<RankedResult> {
        // CombinedScore = α * VectorScore + β * PageRank + γ * HyperbolicBoost
    }
}
```

### 5. Benchmark suite

`memory/benchmark.rs`:
```rust
#[cfg(test)]
mod benchmarks {
    use test::Bencher;
    
    #[bench]
    fn bench_search_1k_entries(b: &mut Bencher)
    
    #[bench]
    fn bench_search_10k_entries(b: &mut Bencher)
    
    #[bench]
    fn bench_embedding_latency(b: &mut Bencher)
    
    #[bench]
    fn bench_pagerank(b: &mut Bencher)
}
```

### 6. Integration tests

```rust
#[tokio::test]
async fn test_full_memory_workflow() {
    // 1. Store 100 entries
    // 2. Search with various queries
    // 3. Verify ranking with PageRank
    // 4. Check graph statistics
    // 5. Verify persistence
}
```

---

## 완료 기준

```bash
cargo benchmark -p oxios-kernel memory
# → Performance targets 달성:

# Phase 1 targets:
# - Search latency (1K entries): <10ms ✓
# - Search latency (10K entries): <20ms ✓
# - Embedding latency: <50ms ✓

# Phase 2 targets:
# - Pattern routing accuracy: >89%
# - SONA adaptation: <0.05ms
# - Worker dispatch: <100ms

# Phase 3 targets:
# - Flash Attention speedup: 2-5x
# - Memory reduction: 75%
```

---

## Session 3 완료 후

모든 Session 완료 시 Memory System 완성:

```
✅ Layer 1: Memory (HNSW + MemoryGraph + SqliteStore)
✅ Layer 2: Learning (ReasoningBank + SONA + Workers)
✅ Layer 3: Embedding (Onnx + OpenAI + Hybrid + Chunking + Normalizer)
✅ Integration: Kernel + Web + Ouroboros
✅ Migration: TF-IDF → HNSW
✅ Polish: Flash Attention + Hyperbolic
```

**Memory System → Ouroboros Worker → Swarm = 똑똑한 멀티 에이전트 OS**

---

## 참조 문서

- 메인 설계: `docs/design/memory-main-design.md`
- Session 1: `docs/design/memory-session1.md`
- Session 2: `docs/design/memory-session2.md`
- 전체 OS: `docs/ARCHITECTURE.md`