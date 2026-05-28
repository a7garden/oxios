# RFC-017: 메모리 시스템 미사용 모듈 정리

> **상태:** 📝 설계
> **날짜:** 2026-05-27 (v2 개정)
> **우선순위:** P2
> **범위:** `crates/oxios-kernel/src/memory/`
> **선행:** 없음
> **후행:** RFC-020 (proactive recall + sona 활성화)

---

## 1. 동기

### 1.1 규모

메모리 시스템은 27개 파일, 12,208줄로 커널에서 가장 큰 서브시스템이다.
그 중 **2개 모듈(1,234줄)은 컴파일에서 완전히 제외된 죽은 코드**이며,
**2개 모듈(805줄)은 컴파일은 되나 프로덕션에서 한 번도 실행된 적 없다**.

이 RFC는 전자(죽은 코드)를 다루고, 후자(미실행 코드)는 후행 RFC-020으로 분리한다.

### 1.2 활성도 전체 조사

코드 내 `rg` 검색으로 모든 모듈의 실제 상태를 분류했다:

#### ✅ 활성 모듈 (23개, 10,404줄)

프로덕션 경로에서 실제로 호출되는 모듈. feature gate 불필요.

| 모듈 | 줄 수 | 핵심 호출 경로 |
|------|--------|---------------|
| `store` | 1,132 | `MemoryManager`의 모든 CRUD + search |
| `dream` | 1,230 | `spawn_dream_task()` → 백그라운드 통합 |
| `sqlite_store` | 905 | SQLite 영속 백엔드 |
| `mod.rs` (MemoryManager) | 912 | 커널 전체에서 사용 |
| `auto_memory_bridge` | 1,000 | lib.rs에서 re-export |
| `hyperbolic` | 689 | dream.rs에서 `HyperbolicEmbedding::restore_from_sqlite()` |
| `database` | 513 | SQLite 스키마 정의 |
| `search` | (디렉토리) | BM25 + vector + RRF 검색 |
| `flash_attention` | 592 | sqlite_store.rs에서 re-ranking |
| `hnsw` | 317 | 벡터 ANN 검색 |
| `auto_classify` | 291 | dream.rs에서 `AutoClassifier::infer_memory_type()` |
| `migration` | 246 | DB 스키마 마이그레이션 |
| `migrate` | 249 | 데이터 마이그레이션 |
| `auto_protect` | 369 | dream.rs에서 `compute_protection()` |
| `compaction` | 185 | dream.rs에서 `CompactionTree` 사용 |
| `embedding_cache` | 251 | cache.rs에서 SQLite embedding_cache 테이블 사용 |
| `graph` | 268 | sqlite_store.rs에서 `build_co_access_graph()` + `pagerank()` |
| `root_index` | 164 | dream.rs에서 `rebuild_root_index()` |
| `decay` | 227 | dream.rs에서 감쇠 적용 |
| `cache` | 202 | SQLite embedding 캐시 |
| `chunking` | 262 | lib.rs에서 re-export |
| `normalizer` | 122 | store.rs, embedding.rs에서 사용 |
| `budget` | 43 | `MemoryBudget` 큐레이션 |

#### ❌ 죽은 코드 (2개, 1,234줄) — 이 RFC의 대상

| 모듈 | 줄 수 | 상태 | 비고 |
|------|--------|------|------|
| `reasoning_bank` | 663 | `mod.rs`에 선언 없음. 컴파일 불가 | 키워드 기반 작업 라우팅. Ouroboros가 이미 담당 |
| `rvf_store` | 571 | `mod.rs`에 선언 없음. 컴파일 불가 | RL 기반 검색 가치 추정 + EWC. LLM 에이전트에 부적합 |

#### ⚠️ 미실행 코드 (2개, 805줄) — RFC-020으로 분리

| 모듈 | 줄 수 | 상태 | 비고 |
|------|--------|------|------|
| `sona` | 585 | 컴파일됨, 생성/호출 0건 | 실행 trajectory 학습 엔진 |
| `proactive` | 220 | 컴파일 + re-export됨, 인스턴스화 0건 | 사전 기억 회상 |

---

## 2. 죽은 코드 분석

### 2.1 `reasoning_bank` — 왜 죽은 코드인가

**원래 목적**: 에이전트가 성공적으로 수행한 전략(`GuidancePattern`)을 저장하고, 새 작업이 들어오면 임베딩 유사도로 과거 패턴을 검색하여 최적의 에이전트에게 라우팅.

**왜 필요 없어졌는가**:

| reasoning_bank가 하려던 것 | 현재 시스템이 이미 하는 것 |
|---|---|
| 작업 설명 → 적합한 에이전트 추천 | **Ouroboros** (Interview → Seed → 맞춤 에이전트 생성) |
| 키워드 → 에이전트 매핑 | 정적 `RoutingEntry` 테이블 (코드 내 하드코딩으로 충분) |
| 성공 패턴 저장/검색 | **Memory 시스템** (Skill/Decision 타입) + Dream 자동 분류 |

Ouroboros가 동적으로 에이전트를 생성하므로, 정적 라우팅 테이블 기반의 reasoning_bank는 아키텍처와 맞지 않음.

### 2.2 `rvf_store` — 왜 죽은 코드인가

**원래 목적**: Retrieval Value Function — 강화학습으로 "어떤 검색 결과가 가치 있는지" 추정. EWC(Elastic Weight Consolidation)로 파괴적 망각 방지. 커스텀 바이너리 포맷(`.rvls`)으로 영속화.

**왜 필요 없어졌는가**:

| rvf_store가 하려던 것 | 현재 시스템의 현실 |
|---|---|
| RL 기반 검색 가치 추정 | LLM 에이전트는 파인튜닝되지 않음. 가치 추정할 "가중치"가 없음 |
| EWC 파괴적 망각 방지 | Oxios는 LLM 파라미터를 업데이트하지 않음. 망각할 "모델"이 없음 |
| 커스텀 `.rvls` 바이너리 포맷 | SQLite store가 이미 동일 데이터를 `patterns` 테이블로 관리 |

EWC는 신경망 파인튜닝에서나 의미 있는 개념이다. LLM API를 호출하는 에이전트 OS에는 해결할 문제 자체가 존재하지 않음.

---

## 3. 설계

### 3.1 원칙

1. **죽은 코드는 삭제한다** (feature gate가 아닌 삭제)
2. **관련 설정은 정리한다** (config에서 미사용 필드 제거)
3. **주석만 남기는 것은 하지 않는다** (git history가 문서 역할)

### 3.2 파일 삭제

```
삭제:
  crates/oxios-kernel/src/memory/reasoning_bank.rs  (663줄)
  crates/oxios-kernel/src/memory/rvf_store.rs        (571줄)
```

총 1,234줄 삭제.

### 3.3 config 정리

`config.rs`에서 `ReasoningBank` 관련 설정을 정리한다:

```rust
// LearningConfig에서 ReasoningBank 참조 제거
// Before:
/// Controls SONA self-learning and ReasoningBank persistence.
// After:
/// Controls SONA self-learning persistence.
```

`database.rs` 주석에서 RVF 참조 정리:

```rust
// Before:
//! - `patterns` — learning patterns (SONA, ReasoningBank, RVF)
// After:
//! - `patterns` — learning patterns (SONA)
```

`sqlite_store.rs`에서 `"reasoning"`, `"rvf"` 전략명 참조는 허용 — `patterns` 테이블의 `strategy` 컬럼은 자유 문자열이므로 기존 데이터 호환성을 위해 스키마 변경 불필요.

`workers/mod.rs` 주석에서 `ReasoningBank` 참조 정리.

### 3.4 활성 모듈의 `#![allow(dead_code)]` 정리

현재 활성 모듈 중 불필요한 `#![allow(dead_code)]`가 있는 것들:

| 파일 | allow(dead_code) | 실제 사용 | 조치 |
|------|-------------------|----------|------|
| `dream.rs` | ✅ 있음 | 활성 (spawn_dream_task) | 제거 |
| `compaction.rs` | ✅ 있음 | 활성 (dream에서 사용) | 제거 |
| `auto_protect.rs` | ✅ 있음 | 활성 (dream에서 사용) | 제거 |
| `auto_memory_bridge.rs` | ✅ 있음 | 활성 (lib.rs re-export) | 제거 |

제거 후 `cargo check`로 실제 dead code 경고가 없는지 확인.

### 3.5 모듈 활성도 문서화

`memory/mod.rs` 최상단에 활성도 요약 주석을 추가한다:

```rust
//! Agent memory system.
//! 
//! ## Module Activity Status (RFC-017, 2026-05)
//! 
//! 모든 모듈은 활성 경로에서 사용된다:
//! 
//! | 범주 | 모듈 | 핵심 역할 |
//! |------|------|----------|
//! | **핵심** | store, sqlite_store, search | CRUD + 영속화 + 검색 |
//! | **통합** | dream | 4-phase 백그라운드 통합 |
//! | **분석** | graph, hnsw, flash_attention | PageRank, ANN, re-ranking |
//! | **생명주기** | decay, auto_protect, auto_classify, compaction | 감쇠/보호/분류/압축 |
//! | **인프라** | cache, embedding_cache, database, migration, migrate | 캐시/스키마/마이그레이션 |
//! | **유틸** | budget, normalizer, chunking, root_index | 예산/정규화/청킹/인덱스 |
//! | **학습** | sona, proactive | ⚠️ 구현됨, RFC-020에서 활성화 예정 |
//! 
//! 삭제된 모듈 (git history에 보존):
//! - `reasoning_bank` (RFC-017): Ouroboros가 동일 역할 담당
//! - `rvf_store` (RFC-017): LLM 에이전트에 부적합한 RL/EWC 개념
```

---

## 4. 마이그레이션 계획

### 단일 Phase (0.5일)

| 작업 | 파일 | 비고 |
|------|------|------|
| 파일 삭제 | `reasoning_bank.rs`, `rvf_store.rs` | git history에 보존 |
| config 주석 정리 | `config.rs` | ReasoningBank 참조 제거 |
| 주석 정리 | `database.rs`, `workers/mod.rs` | RVF/ReasoningBank 참조 정리 |
| `#![allow(dead_code)]` 제거 | `dream.rs`, `compaction.rs`, `auto_protect.rs`, `auto_memory_bridge.rs` | 제거 후 cargo check |
| 활성도 주석 추가 | `memory/mod.rs` | 모듈별 활성도 표 |
| `cargo test --workspace` | — | 회귀 확인 |

---

## 5. 기대 효과

| 지표 | 변경 전 | 변경 후 |
|------|---------|---------|
| 메모리 모듈 파일 | 27개 | 25개 |
| 메모리 모듈 줄 수 | 12,208 | 10,974 |
| 죽은 코드 | 2개 파일 (1,234줄) | 0 |
| `#![allow(dead_code)]` | 4개 활성 모듈에 부정확하게 적용 | 실제 필요한 곳만 |
| 모듈 활성도 파악 | 코드 검색 필요 | mod.rs 주석으로 O(1) |

---

## 6. 성공 기준

- [ ] `reasoning_bank.rs`, `rvf_store.rs` 삭제됨
- [ ] `cargo test --workspace` 통과
- [ ] `#![allow(dead_code)]`가 활성 모듈 4개에서 제거됨 (cargo check 경고 없음)
- [ ] `memory/mod.rs`에 활성도 요약 주석 추가됨
- [ ] config/workers 주석에 ReasoningBank/RVF 참조 정리됨
