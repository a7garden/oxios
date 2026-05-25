# Memory System — Session Prompts (Final)

> 각 Session은 **git worktree**에서 독립적으로 진행합니다.
> Session 2는 Session 1이 main에 merge된 후에 시작합니다.

---

## Worktree 설정 (시작 전)

```bash
# Session 1용 worktree
git worktree add ../oxios-memory-s1 -b feat/memory-session1

# Session 2용 worktree (Session 1 merge 후)
git worktree add ../oxios-memory-s2 -b feat/memory-session2

# Session 3용 worktree (Session 2 merge 후, optional)
git worktree add ../oxios-memory-s3 -b feat/memory-session3
```

---

## Session 1 프롬프트

```
Oxios Memory System Session 1: HNSW 기반 벡터 메모리 인프라 구축

## 작업 환경
- worktree: ../oxios-memory-s1 (branch: feat/memory-session1)
- 커밋 메시지 규칙: feat(memory): ...

## 설계 문서
다음 파일을 먼저 읽고 시작하세요:
- docs/design/memory-main-design.md (전체 아키텍처)
- docs/design/memory-session1.md (이번 작업 상세)

## 작업 범위 (건드리는 파일)

### 신규 생성 (겹침 없음)
- crates/oxios-kernel/src/memory/error.rs
- crates/oxios-kernel/src/memory/hnsw.rs
- crates/oxios-kernel/src/memory/graph.rs
- crates/oxios-kernel/src/memory/chunking.rs
- crates/oxios-kernel/src/memory/normalizer.rs
- channels/oxios-web/src/routes/memory_routes.rs

### 수정 (Session 1에서만 건드림)
- crates/oxios-kernel/Cargo.toml (dependencies 추가)
- crates/oxios-kernel/src/embedding.rs (DenseVector 추가)
- crates/oxios-kernel/src/memory/mod.rs (MemoryManager 확장)
- crates/oxios-kernel/src/memory/store.rs (SqliteIndex 추가)
- crates/oxios-kernel/src/memory/engine.rs (OnnxEngine + OpenAI + Hybrid 전부 구현)
- crates/oxios-ouroboros/src/seed.rs (enrich_with_memory 추가)

### 절대 건드리지 않음
- memory/reasoning_bank.rs (Session 2)
- memory/rvf_store.rs (Session 2)
- memory/sona.rs (Session 2)
- memory/migrate.rs (Session 2)
- workers/ (Session 2)
- event_bus.rs (Session 2)

## 구현 순서
1. Cargo.toml에 dependencies 추가 (usearch, tract-onnx, rusqlite, reqwest, ndarray)
2. memory/error.rs — MemoryError enum
3. memory/hnsw.rs — HnswIndex (usearch wrapper, CRUD, persistence)
4. memory/graph.rs — MemoryGraph (PageRank, communities, rank_results)
5. memory/store.rs — SqliteIndex 추가 (entries + embeddings 테이블)
6. memory/engine.rs — EmbeddingEngine trait + OnnxEngine + OpenAiEngine + HybridEngine
7. memory/chunking.rs — chunk_text, ChunkingConfig
8. memory/normalizer.rs — l2_normalize, f32_to_fp16, fp16_to_f32
9. embedding.rs — DenseVector 추가, EmbeddingVector::Dense variant
10. memory/mod.rs — MemoryManager 확장 (new_with_hnsw, store, semantic_search, forget)
    - mod.rs의 pub mod 선언에 새 파일들 추가
    - 기존 TF-IDF 코드는 제거하지 말고 보존
11. ouroboros/seed.rs — enrich_with_memory 메서드 추가
12. routes/memory_routes.rs — /api/memory/search, /api/memory/graph/stats
    - routes/mod.rs에 memory_routes 등록
13. 테스트 작성

## 완료 기준
- cargo test -p oxios-kernel — memory 관련 전부 통과
- cargo build -p oxios-kernel — 컴파일 성공
- semantic_search()가 HNSW + Graph ranking으로 동작

## 중요
- Multi-machine/federation 코드 절대 만들지 말 것
- 기존 TF-IDF 코드는 제거하지 말 것 (migration 때문에 보존)
- 모든 공개 API에 문서 주석 작성
- usearch, rusqlite API는 실제 crate 문서를 확인해서 사용할 것
```

---

## Session 2 프롬프트

```
Oxios Memory System Session 2: Learning 레이어 구축

## 전제 조건
Session 1이 main에 merge되어 있어야 합니다.
git merge feat/memory-session1 먼저 실행 후 브랜치 생성하세요.

## 작업 환경
- worktree: ../oxios-memory-s2 (branch: feat/memory-session2)
- 커밋 메시지 규칙: feat(learning): ...

## 설계 문서
다음 파일을 먼저 읽고 시작하세요:
- docs/design/memory-main-design.md (전체 아키텍처)
- docs/design/memory-session2.md (이번 작업 상세)

## 작업 범위 (건드리는 파일)

### 신규 생성 (겹침 없음)
- crates/oxios-kernel/src/memory/reasoning_bank.rs
- crates/oxios-kernel/src/memory/rvf_store.rs
- crates/oxios-kernel/src/memory/sona.rs
- crates/oxios-kernel/src/memory/migrate.rs
- crates/oxios-kernel/src/workers/mod.rs
- crates/oxios-kernel/src/workers/types.rs
- crates/oxios-kernel/src/workers/handlers.rs

### 수정 (Session 2에서만 건드림)
- crates/oxios-kernel/src/event_bus.rs (Learning/Worker 이벤트 추가)
- crates/oxios-kernel/src/lib.rs (workers 모듈 등록)
- channels/oxios-web/src/routes/memory_routes.rs (patterns, learning, workers API 추가만)

### 절대 건드리지 않음
- memory/hnsw.rs (Session 1 결과)
- memory/graph.rs (Session 1 결과)
- memory/engine.rs (Session 1 결과)
- memory/mod.rs (Session 1 결과 — 단, migrate() 추가 시에만 메서드 추가)
- memory/store.rs (Session 1 결과)
- embedding.rs (Session 1 결과)

## 구현 순서
1. memory/reasoning_bank.rs — GuidancePattern, PatternMatch, RoutingResult
   - store_pattern, search, route_task, promote
   - 내부적으로 HnswIndex 사용 (Session 1에서 만든 것)
2. memory/rvf_store.rs — RvfLearningStore (RVLS binary format)
   - initialize, persist, save_pattern, save_trajectory, save_ewc
3. memory/sona.rs — SonalEngine (simplified)
   - record trajectory, distill patterns, adapt
4. memory/migrate.rs — TF-IDF → HNSW migration
   - MemoryManager에 migrate_from_tfidf() 메서드 추가
5. workers/types.rs — WorkerType, WorkerPriority, WorkerConfig
6. workers/handlers.rs — 각 worker 타입별 핸들러
7. workers/mod.rs — WorkerManager (register, dispatch, status)
8. lib.rs — pub mod workers 등록
9. event_bus.rs — PatternLearned, TrajectoryRecorded, WorkerDispatched 등 추가
10. routes/memory_routes.rs — 기존 파일에 patterns, learning, workers API만 추가
11. 테스트 작성

## 완료 기준
- cargo test -p oxios-kernel — reasoning, workers 테스트 통과
- ReasoningBank.search() 동작
- WorkerManager.dispatch() 동작
- TF-IDF migration 스크립트 동작

## 중요
- Session 1에서 만든 HnswIndex, MemoryGraph API를 호출해서 사용
- Session 1의 파일은 읽기만 하고 수정하지 말 것
  (단, memory/mod.rs에 migrate()만 추가하는 것은 허용)
- 모든 공개 API에 문서 주석 작성
```

---

## Session 3 프롬프트 (Optional)

```
Oxios Memory System Session 3: Polish + Advanced Features

## 전제 조건
Session 1 + Session 2가 main에 merge되어 있어야 합니다.

## 작업 환경
- worktree: ../oxios-memory-s3 (branch: feat/memory-session3)
- 커밋 메시지 규칙: feat(memory-polish): ...

## 설계 문서
다음 파일을 먼저 읽고 시작하세요:
- docs/design/memory-main-design.md (전체 아키텍처)
- docs/design/memory-session3.md (이번 작업 상세)

## 작업 범위 — 전부 신규 파일 (겹침 없음)
- crates/oxios-kernel/src/memory/flash_attention.rs
- crates/oxios-kernel/src/memory/hyperbolic.rs
- crates/oxios-kernel/src/memory/auto_memory_bridge.rs

## 구현 순서
1. memory/flash_attention.rs — block-wise attention
2. memory/hyperbolic.rs — Poincaré ball 모델
3. memory/auto_memory_bridge.rs — Claude Code ↔ Oxios sync
4. memory/mod.rs에 위 모듈 등록만 추가
5. Benchmark + Integration tests

## 완료 기준
- Flash Attention 2-5x speedup
- 기존 테스트 전부 통과 (회귀 없음)
```

---

## Merge 순서

```
main ─── feat/memory-session1 ─── (merge) ─── feat/memory-session2 ─── (merge) ─── feat/memory-session3
                                                                              ↑ optional
```

```bash
# Session 1 완료 후
cd /Volumes/MERCURY/PROJECTS/oxios
git merge feat/memory-session1

# Session 2 완료 후
git merge feat/memory-session2

# Session 3 완료 후 (optional)
git merge feat/memory-session3
```