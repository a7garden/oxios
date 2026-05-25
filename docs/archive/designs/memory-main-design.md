# Oxios Memory System — Final Design

> **문서 계층**: 메인 설계서 (Master Design)
> **목적**: 전체 아키텍처 + Session 간 참조
> **날짜**: 2026-05-14

---

## 1. Vision

```
┌─────────────────────────────────────────────────────────────┐
│                    OUR OXI OS                               │
│                                                             │
│  ┌───────────────────────┐    ┌───────────────────────┐   │
│  │   O U R O B O R O S   │    │      S W A R M        │   │
│  │                       │    │                       │   │
│  │  단일 에이전트의       │    │  멀티 에이전트의       │   │
│  │  작업 접근 방식       │    │  오케스트레이션        │   │
│  └───────────────────────┘    └───────────────────────┘   │
│                    ×                                      │
│                    ┌───────────────────┐                   │
│                    │ Ouroboros Worker │                   │
│                    │ = spec-first     │                   │
│                    │   + coordinated │                   │
│                    └───────────────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

**Memory System의 역할**: Ouroboros Worker를 더 똑똑하게 만들어 Swarm 전체를 더 똑똑하게 한다.

---

## 2. System Architecture (3-Layer)

```
Layer 1: Memory (벡터 검색)
├── HnswIndex (usearch)      — O(log n) ANN 검색
├── MemoryGraph (PageRank)    — 구조적 중요도로 검색 품질 향상
├── SqliteStore               — Hybrid 저장 (metadata + vector)
└── MemoryManager             — Orchestrator

Layer 2: Learning (패턴 학습)
├── ReasoningBank             — 패턴 저장/검색/라우팅
├── RVF Learning Store        — Binary 영속 (.rvls)
├── SONA Engine               — Trajectory 추적, self-learning
└── Workers Manager           — Background 최적화

Layer 3: Embedding (벡터 생성)
├── OnnxEngine                — Local ONNX (all-MiniLM-L6-v2, 384차원)
├── OpenAI Engine             — API fallback
├── Hybrid Engine            — Local + API 자동 전환
├── Chunking                  — 长文档 분할
└── Normalizer                — L2/FP16/INT8
```

---

## 3. Feature Table (20개, 전부 단일 머신)

| # | 기능 | 파일 | Priority | Multi-machine |
|---|------|------|----------|---------------|
| 1 | HNSW 인덱스 | `memory/hnsw.rs` | P0 | ❌ |
| 2 | MemoryGraph | `memory/graph.rs` | P0 | ❌ |
| 3 | SqliteStore | `memory/store.rs` | P0 | ❌ |
| 4 | MemoryManager | `memory/mod.rs` | P0 | ❌ |
| 5 | OnnxEngine | `memory/engine.rs` | P0 | ❌ |
| 6 | Web API | `routes/memory_routes.rs` | P0 | ❌ |
| 7 | ReasoningBank | `memory/reasoning_bank.rs` | P1 | ❌ |
| 8 | RVF Learning Store | `memory/rvf_store.rs` | P1 | ❌ |
| 9 | OpenAI Engine | `memory/engine.rs` | P1 | ❌ |
| 10 | Hybrid Engine | `memory/engine.rs` | P1 | ❌ |
| 11 | Document Chunking | `memory/chunking.rs` | P1 | ❌ |
| 12 | Normalizer | `memory/normalizer.rs` | P1 | ❌ |
| 13 | Kernel Events | `event_bus.rs` | P1 | ❌ |
| 14 | Seed Enrichment | `ouroboros/seed.rs` | P1 | ❌ |
| 15 | TF-IDF Migration | `memory/migrate.rs` | P1 | ❌ |
| 16 | SONA Engine | `memory/sona.rs` | P2 | ❌ |
| 17 | Workers Manager | `workers/mod.rs` | P2 | ❌ |
| 18 | Flash Attention | `memory/flash_attention.rs` | P3 | ❌ |
| 19 | Hyperbolic Embeddings | `memory/hyperbolic.rs` | P3 | ❌ |
| 20 | Auto-Memory Bridge | `memory/auto_memory_bridge.rs` | P4 | ❌ |

**Multi-machine/Federation 관련 기능 없음** — Ruflo의 agent federation 등은 배제

---

## 4. Module Structure

```
crates/oxios-kernel/src/
├── embedding.rs          # 수정: DenseVector 추가
├── memory/
│   ├── mod.rs           # MemoryManager (Orchestrator)
│   ├── store.rs         # SqliteIndex (Hybrid Storage)
│   ├── hnsw.rs          # HNSW Index (usearch)
│   ├── graph.rs         # MemoryGraph (PageRank + Communities)
│   ├── engine.rs        # Embedding Engines (Onnx/OpenAI/Hybrid)
│   ├── chunking.rs      # Document Chunking
│   ├── normalizer.rs    # L2/FP16/INT8 Normalization
│   ├── hyperbolic.rs    # Poincaré Ball Embeddings
│   ├── reasoning_bank.rs # ReasoningBank (Pattern Learning)
│   ├── rvf_store.rs    # RVF Learning Store
│   ├── sona.rs          # SONA Engine
│   ├── flash_attention.rs # Flash Attention
│   ├── migrate.rs      # TF-IDF → HNSW Migration
│   └── error.rs        # Memory Errors
├── workers/
│   ├── mod.rs           # WorkerManager
│   ├── types.rs         # Worker Types
│   └── handlers.rs      # Worker Implementations
└── events/
    └── rvf_event_log.rs # Binary Event Log (optional)
```

---

## 5. Dependencies

```toml
# oxios-kernel/Cargo.toml

[dependencies]
usearch = { version = "0.16", features = ["simd"] }
tract-onnx = { version = "0.26", features = ["onnx"] }
rusqlite = { version = "0.34", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json"] }
ndarray = "0.16"

[features]
memory-hnsw = ["usearch", "tract-onnx", "rusqlite", "reqwest", "ndarray"]
```

---

## 6. Session Division

### Session 1 (Phase 1 + P1 핵심)
**목표**: 동작하는 HNSW 메모리 시스템

| # | 기능 | 파일 |
|---|------|------|
| 1 | HNSW 인덱스 | `memory/hnsw.rs` |
| 2 | MemoryGraph | `memory/graph.rs` |
| 3 | SqliteStore | `memory/store.rs` |
| 4 | MemoryManager | `memory/mod.rs` |
| 5 | OnnxEngine | `memory/engine.rs` |
| 6 | Web API | `routes/memory_routes.rs` |
| 11 | Document Chunking | `memory/chunking.rs` |
| 12 | Normalizer | `memory/normalizer.rs` |
| 14 | Seed Enrichment | `ouroboros/seed.rs` |

**Deliverable**: `cargo test memory` 통과, `semantic_search()` P50 <10ms

### Session 2 (Phase 2 + 남은 P1)
**목표**: Learning 시스템 통합

| # | 기능 | 파일 |
|---|------|------|
| 7 | ReasoningBank | `memory/reasoning_bank.rs` |
| 8 | RVF Learning Store | `memory/rvf_store.rs` |
| 9 | OpenAI Engine | `memory/engine.rs` |
| 10 | Hybrid Engine | `memory/engine.rs` |
| 13 | Kernel Events | `event_bus.rs` |
| 15 | TF-IDF Migration | `memory/migrate.rs` |
| 16 | SONA Engine | `memory/sona.rs` |
| 17 | Workers Manager | `workers/mod.rs` |

**Deliverable**: Pattern routing работет, Workers dispatch работет

### Session 3 (Phase 3+ — Future)
**목표**: Polish + Advanced features

| # | 기능 | 파일 |
|---|------|------|
| 18 | Flash Attention | `memory/flash_attention.rs` |
| 19 | Hyperbolic Embeddings | `memory/hyperbolic.rs` |
| 20 | Auto-Memory Bridge | `memory/auto_memory_bridge.rs` |

---

## 7. Key Algorithms

### 7.1 MemoryGraph Combined Score

```rust
CombinedScore = α × VectorScore + β × PageRank + γ × CommunityBoost

where:
  α = 0.6  // Vector similarity weight
  β = 0.3  // Structural importance weight
  γ = 0.1  // Community cohesion boost
```

### 7.2 HNSW Parameters

| Parameter | Value | Notes |
|-----------|-------|-------|
| `M` | 16 | Connections per node |
| `ef_construction` | 128 | Search width during build |
| `ef_search` | 128 | Search width during query |
| `metric` | Cosine | Cosine similarity |
| `quantization` | FP16 | Memory 50% savings |

### 7.3 RVF File Format

```
4-byte magic "RVLS" + newline
{"type":"pattern","data":{...}}\n
{"type":"trajectory","data":{...}}\n
{"type":"ewc","data":{...}}\n
4-byte magic "REND"
```

---

## 8. API Examples

### 8.1 Memory Search

```bash
GET /api/memory/search?q=authentication+patterns&limit=10&threshold=0.6
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
        "tags": ["auth", "security"]
      },
      "score": 0.87,
      "rank": 1,
      "page_rank": 0.15,
      "community": "security-patterns"
    }
  ],
  "latency_ms": 12
}
```

### 8.2 Pattern Search (ReasoningBank)

```bash
GET /api/memory/patterns?q=security+audit&domain=security
```

```json
{
  "patterns": [
    {
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

### 8.3 Learning Stats

```bash
GET /api/memory/learning/stats
```

```json
{
  "reasoning_bank": {
    "short_term_count": 127,
    "long_term_count": 892
  },
  "sona": {
    "trajectories_recorded": 456,
    "last_adaptation_ms": 0.03
  },
  "workers": {
    "active": 8
  }
}
```

---

## 9. Related Documents

| 문서 | 용도 |
|------|------|
| `memory-session1.md` | Session 1 작업 지시서 |
| `memory-session2.md` | Session 2 작업 지시서 |
| `ARCHITECTURE.md` | 전체 OS 아키텍처 |
| `AGENTS.md` | AI agent 가이드 |

---

## 10. Success Metrics

| Metric | Target | Phase |
|--------|--------|-------|
| Search latency (1K entries) | <10ms | P0 |
| Search latency (10K entries) | <20ms | P0 |
| Embedding latency (single) | <50ms | P0 |
| Recall rate | >95% | P0 |
| Memory index (1K entries) | <10MB | P0 |
| Pattern routing accuracy | >89% | P1 |
| SONA adaptation | <0.05ms | P2 |
| Worker dispatch latency | <100ms | P2 |