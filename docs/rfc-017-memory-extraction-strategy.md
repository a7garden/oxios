# RFC-017: Memory Extraction Strategy — RFC-016 Phase B 완료

> **날짜**: 2026-06-04
> **상태**: 초안
> **선행 RFC**: [RFC-016: Kernel Boundary Cleanup](rfc-016-kernel-boundary-cleanup.md)
> **범위**: `oxios-kernel`의 memory subsystem을 `oxios-memory`로 단계적 추출

---

## 1. 배경/동기

### 1.1 RFC-016의 잔여 작업

RFC-016은 5개 Phase 중 4개 (A, C, D부분, E부분)을 완료했으나 **Phase B (memory 추출)는 미완**으로 남았다. 3번의 시도 끝에 다음이 확인됐다:

- **시도 1** (2026-06-04): 30+ 컴파일 에러 → trait 추상화로 17까지 → revert
- **시도 2** (2026-06-04): *facade* 패턴 채택 — `oxios-memory` 크레이트 신설, kernel의 memory 타입들을 re-export. **main에 머지됨** (`58427d5`).
- **시도 3** (2026-06-04): 진짜 추출 시도 — 82→0 에러 도달 → kernel 통합 단계에서 `crate::memory::*` import 100+ site 깨짐 → revert

### 1.2 본질적 blocker

memory subsystem은 `oxios-kernel`의 다른 핵심 컴포넌트와 *양방향 강결합*:

| Blocker | 영향 범위 | 추출 우선순위 |
|------|------|------|
| `MemoryManager.state_store: Arc<StateStore>` | `kernel_handle/agent_api.rs`, `tools/memory_tools.rs`, `tools/kernel_bridge.rs` 등 15+ 파일 | 높음 |
| `MemoryManager.git_layer: Option<Arc<GitLayer>>` | `auto_memory_bridge.rs` 외 | 중간 |
| `with_config(&MemoryConfig)` | `lib.rs`의 facade | 높음 |
| `database.rs: save_project/list_projects` | `project/manager.rs`와 결합 | 중간 |
| `migrate.rs #[cfg(test)]` | 테스트 코드 | 낮음 |

**결론**: 단순 file move로 불가능. **설계 결정이 필요**.

---

## 2. 목표 / 비목표

### 2.1 목표

1. **`MemoryManager`와 그 의존성을 `oxios-memory`로 이동** — kernel과 memory의 경계 명확화
2. **단계적으로 추출** — 각 단계가 *독립적으로 컴파일 가능*하고 *모든 테스트 통과* 상태를 유지
3. **AGENTS.md §10 "kernel is intentionally monolithic"** 원칙과의 *점진적* 양립
4. **외부 API 호환성** — `oxios_kernel::MemoryManager` 같은 기존 re-export 경로는 그대로 동작

### 2.2 비목표 (의식적 보류)

- **kernel의 *다른* 모듈 (supervisor, tools, a2a, etc.) 분할** — 이번 RFC 범위 밖
- **`oxios-ouroboros` 이름 변경** — attribution 보존 (RFC-016 §2 비목표)
- **memory *구현*의 완전한 교체** — 예: `MemoryManager`를 `RocksDB`로 백킹. 이번 RFC는 *추출*만 다룸
- **`oxios-mcp` 흡수** — 별도 결정

---

## 3. 설계 — 3가지 옵션 비교

### 3.1 옵션 (a): Trait 추상화 (1-2주, *Big Bang*)

```rust
// oxios-memory/src/memory/storage.rs
#[async_trait]
pub trait MemoryStorage: Send + Sync {
    async fn save_json_value(&self, cat: &str, key: &str, value: &Value) -> Result<()>;
    async fn load_json_value(&self, cat: &str, key: &str) -> Result<Option<Value>>;
    async fn list_category(&self, cat: &str) -> Result<Vec<String>>;
    async fn delete_file_value(&self, cat: &str, key: &str) -> Result<()>;
}

#[async_trait]
pub trait MemoryGit: Send + Sync {
    async fn commit_file(&self, path: &str, message: &str) -> Result<()>;
    fn is_enabled(&self) -> bool;
}

// oxios-kernel/src/state_store.rs
#[async_trait]
impl MemoryStorage for StateStore { ... }
```

**장점**:
- 정식 분리, type-safe
- `MemoryManager`가 `dyn MemoryStorage` 보유 → 진정한 의존성 역전
- 테스트 시 mock 구현 가능

**단점**:
- *Dyn trait 한계*: generic 메서드 (`save_json<T>`)는 `dyn` 불가능 → `serde_json::Value` 변환 필요
- `serde_json::Value` ↔ typed 변환이 호출자 책임 (try-build/test fixture)
- *Big Bang*: 모든 것이 한꺼번에 바뀌어야 함 — 중간 상태 빌드 깨짐

**예상 작업량**: 1-2주 (trait 정의 + StateStore impl + 모든 호출자 수정 + 테스트)

### 3.2 옵션 (b): 단계적 추출 (3-4주) — *권장*

이미 main에 머지된 *facade* 패턴을 단계적으로 확장. 각 단계가 *독립 PR* 가능.

| 단계 | 내용 | 작업량 | 검증 |
|------|------|------|------|
| **b.1** | `chunking`, `normalizer`, `hyperbolic` 모듈 이동 | 2-3일 | 739 tests pass |
| **b.2** | `embedding` 모듈 (TfIdfEmbeddingProvider) 이동 | 2-3일 | 739 tests pass |
| **b.3** | `root_index`, `quota` 모듈 이동 | 1일 | 739 tests pass |
| **b.4** | `decay`, `auto_classify`, `auto_protect` 모듈 이동 | 2-3일 | 739 tests pass |
| **b.5** | `compaction`, `flash_attention`, `graph` 모듈 이동 | 2-3일 | 739 tests pass |
| **b.6** | `MemoryStorage` trait + `StateStore` impl (kernel 잔류) | 3-4일 | 739 tests pass |
| **b.7** | `MemoryManager` 이동 (impl trait) | 3-5일 | 739 tests pass |
| **b.8** | sqlite backend 이동 (`SqliteMemoryStore`) | 2-3일 | 739 tests pass |
| **b.9** | `migrate`, `dream` 이동 (orchestration) | 3-5일 | 739 tests pass |

**장점**:
- 각 단계가 *독립적으로 컴파일 가능*하고 *테스트 통과* 상태를 유지
- 회귀 위험 최소
- 각 PR이 작아 리뷰 가능
- 문제 발생 시 해당 단계만 revert

**단점**:
- 더 많은 PR (10개+)
- 더 긴 전체 일정
- `MemoryManager` 이동(b.7)이 여전히 가장 큰 단계

### 3.3 옵션 (c): Leaf-부터 점진 (4-6주)

옵션 (b)와 유사하지만 *의존성 그래프*의 leaf부터 시작:

```
Phase 1: 의존성 0
  - chunking, cosine_similarity_f32, content_hash (pure math/strings)

Phase 2: 의존성 1
  - MemoryType, MemoryTier, ProtectionLevel (단순 enum, serde)
  - TextVector (uses HashMap)

Phase 3: 의존성 2
  - MemoryEntry (다른 type들을 가짐)

Phase 4: 의존성 3+
  - root_index, quota, decay, auto_classify, auto_protect
  - compaction, flash_attention, graph, embedding_cache
  - hnsw, hyperbolic

Phase 5: trait
  - MemoryStorage trait 도입
  - StateStore impl

Phase 6: 구현
  - MemoryManager 이동
  - SqliteMemoryStore 이동

Phase 7: orchestration
  - dream, migrate, auto_memory_bridge
```

**장점**:
- 의존성 그래프를 *교육적으로* 이해 가능
- 가장 작은 단계 (1-2일)
- 각 단계에서 *도메인 학습* 가능

**단점**:
- 가장 긴 전체 일정
- 작은 단계가 많아 overhead

### 3.4 비교표

| | 옵션 (a) Trait | 옵션 (b) 단계적 | 옵션 (c) Leaf |
|---|-----|-----|-----|
| 총 작업량 | 1-2주 | 3-4주 | 4-6주 |
| PR 수 | 1-2 (big) | 10+ (small) | 15+ (tiny) |
| 회귀 위험 | 중간 | 낮음 | 매우 낮음 |
| Big Bang 의존 | **높음** | 없음 | 없음 |
| 학습 곡선 | 낮음 | 중간 | 높음 |
| **권장** | 빠른 검증용 | **production 권장** | research/learning |

---

## 4. 권장안: 옵션 (b)

### 4.1 선택 근거

1. **production 안전성** 우선 — 큰 PR은 리뷰/롤백이 어려움
2. **각 단계가 2-5일** — 1-2주의 컨텍스트 스위칭 없이 완료 가능
3. **현재 facade 패턴**과 자연스럽게 연결 — `58427d5` 커밋이 기반
4. **b.7 (`MemoryManager` 이동)까지 가면 *자동으로* trait 추상화의 이점을 누림**

### 4.2 단계별 상세

#### b.1: chunking, normalizer, hyperbolic

```bash
git mv crates/oxios-kernel/src/memory/chunking.rs crates/oxios-memory/src/memory/
git mv crates/oxios-kernel/src/memory/normalizer.rs crates/oxios-memory/src/memory/
git mv crates/oxios-kernel/src/memory/hyperbolic.rs crates/oxios-memory/src/memory/
```

- 의존성: stdlib + serde + chrono만
- 다른 모듈과 결합 없음
- kernel의 `memory::chunking::X` → `oxios_memory::memory::chunking::X`로 import 변경

**위험**: 낮음. **작업량**: 2-3일.

#### b.2: embedding (TfIdfEmbeddingProvider)

```bash
git mv crates/oxios-kernel/src/embedding.rs crates/oxios-memory/src/embedding.rs
git mv crates/oxios-kernel/src/embedding/gguf/ crates/oxios-memory/src/embedding/gguf/
```

- 의존성: kernel과 무관 (TfIdf는 pure)
- **단, oxi-sdk에 `EmbeddingProvider` trait이 있음** — re-export로 충분
- kernel의 `embedding::TfIdfEmbeddingProvider` → `oxios_memory::embedding::TfIdfEmbeddingProvider`

**위험**: 낮음. **작업량**: 2-3일.

#### b.3: root_index, quota

- 의존성: chunking, types
- 작은 모듈 (각 ~100 LoC)

**위험**: 낮음. **작업량**: 1일.

#### b.4: decay, auto_classify, auto_protect

- 의존성: types, embedding
- *Auto* 로직이지만 kernel의 다른 부분과 무관

**위험**: 낮음-중간. **작업량**: 2-3일.

#### b.5: compaction, flash_attention, graph

- 의존성: types
- 수학적/그래프 알고리즘 모듈

**위험**: 낮음. **작업량**: 2-3일.

#### b.6: MemoryStorage trait + StateStore impl — *분기점*

```rust
// oxios-memory/src/memory/storage.rs (이미 §3.1에 정의)
#[async_trait]
pub trait MemoryStorage: Send + Sync { ... }
```

```rust
// oxios-kernel/src/state_store.rs
#[async_trait]
impl MemoryStorage for StateStore {
    async fn save_json_value(&self, ...) -> Result<()> { ... }
    // 기존 save_json 호출을 value 변환 후 위임
}
```

- `MemoryManager`는 *여전히* kernel에 있지만 `Arc<StateStore>` → `Arc<dyn MemoryStorage>`로 변경
- 호출자(`store.rs`의 `state_store.X()`)는 *변경 없음* — trait 메서드와 동일 시그니처

**위험**: 중간. **작업량**: 3-4일. **검증 포인트**: 739 tests 모두 pass.

#### b.7: MemoryManager 이동 — *핵심 단계*

```bash
# store.rs의 impl MemoryManager { ... } 부분을 oxios-memory로
git mv crates/oxios-kernel/src/memory/store.rs crates/oxios-memory/src/memory/store.rs
```

- 이제 `MemoryManager`는 `Arc<dyn MemoryStorage>` 보유
- `StateStore`는 kernel에 *구현*으로 남음 (impl MemoryStorage for StateStore)
- kernel의 `MemoryApi`는 `Arc<MemoryManager>`를 *re-export*

**위험**: 높음. **작업량**: 3-5일. **검증 포인트**:
- 739 tests pass
- `KernelHandle::memory()` 정상 동작
- `oxios_kernel::MemoryManager` 경로 호환

#### b.8: sqlite backend 이동

```bash
git mv crates/oxios-kernel/src/memory/{database,sqlite_store,migration,search}.rs crates/oxios-memory/src/memory/
```

- `SqliteMemoryStore`는 `MemoryStorage` trait impl
- `Project` 타입 의존성 해결 필요 — *trait bound* 또는 *oxios_kernel::Project* import

**위험**: 중간. **작업량**: 2-3일.

#### b.9: migrate, dream, auto_memory_bridge

```bash
git mv crates/oxios-kernel/src/memory/{migrate,dream}.rs crates/oxios-memory/src/memory/
git mv crates/oxios-kernel/src/auto_memory_bridge.rs crates/oxios-memory/src/orchestration/
```

- `auto_memory_bridge`는 *orchestration* (RFC-016 §3.1.1 예외) — kernel-특화 위치
- `dream`은 `MemoryManager` 호출 — b.7 이후 자연스러움

**위험**: 중간. **작업량**: 3-5일.

---

## 5. 마이그레이션 순서

### 5.1 작업 흐름

```
b.1 → b.2 → b.3 → b.4 → b.5 → b.6 → b.7 → b.8 → b.9
                                            ↓
                                       완료 (memory 추출)
```

각 단계:
1. 해당 모듈/크레이트를 `oxios-memory`로 이동
2. `oxios-kernel`에서 `pub use oxios_memory::*` 추가 (back-compat)
3. `cargo test -p oxios-kernel --lib` — 739 tests pass 확인
4. `cargo build -p oxios-memory` — clean 확인
5. 커밋 + PR

### 5.2 각 단계의 공통 패턴

```bash
# 1. 파일 이동
git mv crates/oxios-kernel/src/memory/X.rs crates/oxios-memory/src/memory/

# 2. kernel/Cargo.toml 업데이트
# (oxios-memory가 이미 workspace member이므로 추가 작업 불필요)

# 3. kernel/src/lib.rs 업데이트
# 기존: pub use memory::X
# 신규: pub use oxios_memory::X  (or 둘 다)
# 권장: oxios_memory로 re-export하고 memory::X는 deprecation alias로

# 4. 빌드 + 테스트
cargo test -p oxios-kernel --lib
cargo test -p oxios-memory --lib

# 5. CHANGELOG
# - oxios-memory 0.1.0: 새 crate (types/module 이동)
# - oxios-kernel 1.2.0: re-export 경로 추가, 기존 경로는 deprecated
```

---

## 6. 영향 분석

### 6.1 변경되는 import 표면

| 이전 | 이후 |
|------|------|
| `use oxios_kernel::MemoryManager` | `use oxios_kernel::MemoryManager` (re-export로 호환) |
| `use oxios_kernel::MemoryEntry` | 동일 (re-export) |
| `use oxios_kernel::memory::MemoryEntry` | *deprecated* (1 release), `use oxios_kernel::MemoryEntry` 권장 |
| `use oxios_kernel::embedding::TfIdfEmbeddingProvider` | `use oxios_kernel::embedding::TfIdfEmbeddingProvider` (re-export) |

**Breaking change**: 0개 (모든 변경이 *back-compat* re-export 형태).

### 6.2 의존성 그래프 (after b.9)

```
oxios → oxios-kernel → oxios-memory
                  ├──── oxios-ouroboros
                  ├──── oxios-markdown
                  └──── oxios-mcp

oxios-memory (no oxios-kernel dep)
  ├── oxi-sdk (external)
  ├── chrono, serde, etc.
  └── (pure data + algorithms)
```

### 6.3 빌드 매트릭스

| 단계 | oxios-kernel 빌드 | oxios-memory 빌드 | tests |
|------|------|------|------|
| b.1 | ✅ | ✅ | 739 |
| b.2 | ✅ | ✅ | 739 |
| b.3 | ✅ | ✅ | 739 |
| b.4 | ✅ | ✅ | 739 |
| b.5 | ✅ | ✅ | 739 |
| b.6 | ✅ | ✅ | 739 |
| b.7 | ✅ | ✅ | 739 |
| b.8 | ✅ | ✅ | 739 |
| b.9 | ✅ | ✅ | 739 |

(전 단계에서 "kernel-only" build는 *여전히* 가능 — `MemoryManager`가 kernel에 있는 동안)

---

## 7. 리스크

### 7.1 기술적 리스크

| 리스크 | 확률 | 영향 | 완화 |
|------|------|------|------|
| b.7에서 100+ import site 깨짐 | 높음 | 큰 PR | 단계별 commit으로 bisect 가능 |
| `dyn MemoryStorage` 호출자 코드 비대화 | 중간 | 코드 smell | helper method `MemoryManager::load_typed<T>()` 제공 |
| `SqliteMemoryStore`의 `Project` 결합 | 중간 | b.8 지연 | *trait bound* 또는 *kernel::Project* re-export |
| 테스트 fixture의 StateStore 사용 | 낮음 | b.6-b.7 | trait mock으로 대체 |
| chrono API 변경 (num_days 등) | 낮음 | 컴파일 에러 | 명시적 변환으로 해결 |

### 7.2 일정 리스크

| 리스크 | 영향 |
|------|------|
| 각 단계가 1-2일 추가 소요 가능 | 전체 3-4주 → 4-5주 |
| b.6-b.7 구간이 *예상보다 큰* 결합 드러냄 | 1주 추가 가능 |

### 7.3 비기술적 리스크

- **PR 리뷰 부담** — 10+ PR이 짧은 기간에 들어옴. PR template으로 *위험 평가* 명시
- **main branch 안정성** — 각 PR이 *테스트 통과* 상태를 유지하지 않으면 CI 실패

---

## 8. 비고

### 8.1 시도 3 (RFC-016 Phase B attempt 3)의 교훈

2026-06-04에 시도한 3번째 접근:
- `MemoryStorage` trait + `StateStore` impl까지는 성공
- *kernel 통합 단계*에서 `crate::memory::*` import 100+ site가 깨져서 revert

**교훈**: *facade 패턴을 넘어* 실제로 추출하려면 **kernel 측 import 일괄 변경**이 *먼저* 필요. 옵션 (b) b.6-b.7이 정확히 이 단계.

### 8.2 AGENTS.md와의 양립

AGENTS.md §10:
> "Kernel is intentionally monolithic."

**이 RFC는 이 원칙을 *단계적으로* 약화시킨다.** b.9 완료 시 kernel은 ~24,000 LoC (현재 53,000 LoC의 약 45%)가 되어 더 이상 "monolithic"가 아니다. **의도적 결정**.

### 8.3 미래 확장

- b.9 완료 후 *다른* 모듈 (tools, skill, a2a)도 같은 패턴으로 추출 가능
- RFC-016 §7.1의 "memory code 추출 1-2주" 추정치와 본 RFC의 3-4주 추정치 차이 — 본 RFC는 *10개* PR의 overhead 포함

---

## 9. 체크리스트 (PR별)

### b.1
- [ ] `chunking.rs` 이동
- [ ] `normalizer.rs` 이동
- [ ] `hyperbolic.rs` 이동
- [ ] `oxios-memory` pub use 추가
- [ ] `oxios-kernel` re-export 업데이트
- [ ] 739 tests pass

### b.2
- [ ] `embedding.rs` 이동
- [ ] `embedding/gguf/` 이동
- [ ] `oxi-sdk`의 `EmbeddingProvider` 호환 확인
- [ ] 739 tests pass

### b.3
- [ ] `root_index.rs` 이동
- [ ] `quota.rs` 이동
- [ ] 739 tests pass

### b.4
- [ ] `decay.rs` 이동
- [ ] `auto_classify.rs` 이동
- [ ] `auto_protect.rs` 이동
- [ ] 739 tests pass

### b.5
- [ ] `compaction.rs` 이동
- [ ] `flash_attention.rs` 이동
- [ ] `graph.rs` 이동
- [ ] 739 tests pass

### b.6
- [ ] `MemoryStorage` trait 추가 (oxios-memory)
- [ ] `StateStore` impl (oxios-kernel)
- [ ] `MemoryManager.state_store` 필드 타입 변경: `Arc<StateStore>` → `Arc<dyn MemoryStorage>`
- [ ] 호출자 코드 `state_store.X()` → `state_store.X_value()` + 헬퍼로 typed 변환
- [ ] 739 tests pass

### b.7
- [ ] `store.rs` 전체 이동 (oxios-memory)
- [ ] kernel의 `MemoryApi` re-export 업데이트
- [ ] `MemoryManager` impl trait 의존성 해결
- [ ] 739 tests pass

### b.8
- [ ] `database.rs` 이동
- [ ] `sqlite_store.rs` 이동
- [ ] `migration.rs` 이동
- [ ] `search/` 디렉터리 이동
- [ ] `Project` 타입 결합 해결 (trait bound or re-export)
- [ ] 739 tests pass

### b.9
- [ ] `migrate.rs` 이동
- [ ] `dream.rs` 이동
- [ ] `auto_memory_bridge.rs` → `oxios-memory::orchestration` 또는 kernel 잔류 결정
- [ ] 739 tests pass

### 문서
- [ ] `docs/ARCHITECTURE.md` 업데이트 — kernel/memory 경계 명시
- [ ] `DESIGN.md` 업데이트 — "monolithic" → "two-crate" 철학 전환
- [ ] `CHANGELOG.md`에 b.1-b.9 각 단계 항목 추가
- [ ] `AGENTS.md` §10 "monolithic" 항목 *단계적* 양립 명시

---

## 10. 결정 대기 (사용자)

| 항목 | 결정 |
|------|------|
| 옵션 (a) Trait 추상화 | ☐ 1-2주, big-bang |
| **옵션 (b) 단계적 (권장)** | ☐ 3-4주, 10 PR |
| 옵션 (c) Leaf-부터 | ☐ 4-6주, 15+ PR |
| Phase E (browser 일원화) — oxi-sdk 다운그레이드? | ☐ 0.25.x 시도 |
| Phase E — oxi-agent upstream PR 직접? | ☐ PR 작성 |
| **다음 작업** | ☐ b.1 (chunking)부터 시작 |
