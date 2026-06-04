# RFC-018: Memory Extraction Strategy — RFC-016 Phase B 완료

> **날짜**: 2026-06-04
> **상태**: ✅ 결정됨 — **옵션 (B) 단계적 추출, 9 sub-phases (b.1-b.9)**
> **선행 RFC**: [RFC-016: Kernel Boundary Cleanup](rfc-016-kernel-boundary-cleanup.md)
> **다음 작업**: **b.1 (chunking/normalizer/hyperbolic) — 즉시 시작 가능**

---

## 0. 이 문서를 읽는 사람에게

이 문서는 **새 세션이 cold-start로 작업을 이어갈 수 있도록** 작성된 인수인계 문서입니다.

1. **§1-§3을 읽고** "왜 3개 옵션이 나왔고, 왜 B인가"를 이해
2. **§4를 읽고** "지금부터 뭘 해야 하는가"를 파악
3. **§5를 따라** 첫 PR을 작성

현재 main 브랜치 상태는 §6, *교훈*은 §7.

---

## 1. 배경/동기

### 1.1 RFC-016의 잔여 작업

RFC-016은 5개 Phase 중 4개를 완료했으나 **Phase B (memory 추출)는 미완**으로 남아 있다:

| Phase | 상태 | 위치 |
|------|------|------|
| A (cleanup) | ✅ 완료 | `2459cae`, `ce79b43` |
| B (memory) | 🟡 Facade만 | `58427d5` (5개 시도 끝에 facade 패턴으로 *부분* 완료) |
| C (MemoryApi) | ✅ 완료 | `9273648` |
| D (default features) | 🟡 부분 | `fceb22e` |
| E (browser) | 🟡 kernel만 | `ffec07e` (oxi-sdk 통합은 upstream blocker) |

3번의 시도 끝에 다음이 확인됐다:

- **시도 1**: 30+ 컴파일 에러 → trait 추상화로 17까지 → revert
- **시도 2**: *facade* 패턴 채택 — `oxios-memory` 크레이트 신설, kernel의 memory 타입 re-export → **main 머지**
- **시도 3**: 진짜 추출 시도 — 82→0 에러 도달 → kernel 통합 단계에서 `crate::memory::*` import 100+ site 깨짐 → revert

**결론**: 단순 file move로 불가능. *설계된 단계적 작업*이 필요.

### 1.2 본질적 blocker

memory subsystem은 `oxios-kernel`의 다른 핵심 컴포넌트와 *양방향 강결합*:

| Blocker | 영향 범위 | 추출 우선순위 |
|------|------|------|
| `MemoryManager.state_store: Arc<StateStore>` | `kernel_handle/agent_api.rs`, `tools/memory_tools.rs`, `tools/kernel_bridge.rs` 등 15+ 파일 | 높음 |
| `MemoryManager.git_layer: Option<Arc<GitLayer>>` | `auto_memory_bridge.rs` 외 | 중간 |
| `with_config(&MemoryConfig)` | `lib.rs`의 facade | 높음 |
| `database.rs: save_project/list_projects` | `project/manager.rs`와 결합 | 중간 |
| `migrate.rs #[cfg(test)]` | 테스트 코드 | 낮음 |

---

## 2. 결정: 옵션 (B) 단계적 추출

### 2.1 선택 근거

| 옵션 | 작업량 | PR 수 | 회귀 위험 | 권장 시나리오 |
|------|------|------|------|------|
| (a) Trait 추상화 | 1-2주 | 1-2 (big) | 중간 | 시니어 1명, 압박 |
| **(B) 단계적** | **3-4주** | **10 PR** | **낮음** | **팀, CI/CD 엄격, 리뷰 문화** ✅ |
| (C) Leaf-부터 | 4-6주 | 15+ PR | 매우 낮음 | 학습, 연구 |

**옵션 (B) 선택 이유**:

1. **RFC-016 시도 3의 교훈** — facade만으로는 부족, *kernel 통합 단계*에서 100+ import 깨짐. 10 PR로 쪼개면 그 단계가 *명확히 드러남*.
2. **현재 main 상태** — `oxios-memory` facade가 이미 있음 (`58427d5`). 옵션 (B)는 이걸 *확장*.
3. **CI/CD 현실** — oxios는 `cargo fmt && clippy -D warnings && cargo test --workspace`로 CI 강함. Big Bang PR은 *거의 확실히* CI 깨뜨림.
4. **각 PR이 *문서화* 역할** — §9의 *체크리스트*가 PR별 가이드. "지금 뭐 하고 있는지" 명확.

### 2.2 작업 흐름 (개요)

```
b.1 → b.2 → b.3 → b.4 → b.5 → b.6 → b.7 → b.8 → b.9
  │      │      │      │      │      │      │      │      │
  ▼      ▼      ▼      ▼      ▼      ▼      ▼      ▼      ▼
2-3d  2-3d   1d   2-3d   2-3d  3-4d  3-5d  2-3d  3-5d
                                                    ↓
                                              완료 (memory 추출)
```

---

## 3. 다음 작업: b.1 (chunking/normalizer/hyperbolic 모듈 이동)

**즉시 시작 가능. 다른 작업과 *독립적*.** 이 단계가 끝나면 다음 PR을 위해 1-2일 휴식 가능.

### 3.1 목표

`oxios-kernel/src/memory/`에서 *3개 모듈*을 `oxios-memory`로 이동:

- `chunking.rs` — `TextChunk`, `TextVector`, `ChunkConfig`, `chunk_fixed()`, `chunk_paragraphs()`
- `normalizer.rs` — `cosine_similarity_f32()`, `l2_normalize_f32()`, `l2_normalize_f64()`, `l2_norm_*`, `dot_product_*`
- `hyperbolic.rs` — `HyperbolicConfig`, `HyperbolicEmbedding`, `euclidean_to_poincare()`, `mobius_add()` 등

### 3.2 의존성 분석

| 모듈 | 외부 의존성 | kernel 결합 |
|------|------|------|
| `chunking` | `std`, `serde` | 없음 |
| `normalizer` | `std` | 없음 |
| `hyperbolic` | `std`, `anyhow` | 없음 |

*세 모듈 모두 kernel의 다른 부분과 결합 없음*. 안전.

### 3.3 작업 단계 (PR당 30분-2시간)

```bash
# 0. worktree 생성
cd /Volumes/MERCURY/PROJECTS/oxios
git fetch
git worktree add -b rfc-017-b1-chunking /Volumes/MERCURY/PROJECTS/oxios-b1
cd /Volumes/MERCURY/PROJECTS/oxios-b1

# 1. 파일 이동 (3개)
git mv crates/oxios-kernel/src/memory/chunking.rs crates/oxios-memory/src/memory/
git mv crates/oxios-kernel/src/memory/normalizer.rs crates/oxios-memory/src/memory/
git mv crates/oxios-kernel/src/memory/hyperbolic.rs crates/oxios-memory/src/memory/

# 2. oxios-memory/Cargo.toml에 필요 의존성 추가
#    - chunking: serde 필요
#    - hyperbolic: anyhow 필요
#    - normalizer: 아무것도 안 필요 (std)
# (현재 Cargo.toml은 serde, anyhow를 이미 가지고 있는지 확인)

# 3. oxios-memory/src/memory/mod.rs 업데이트
#    - chunking, normalizer, hyperbolic을 mod로 추가
#    - pub use 로 re-export

# 4. oxios-kernel/src/lib.rs의 re-export 업데이트
#    기존: pub use memory::chunking::TextChunk
#    신규: pub use oxios_memory::TextChunk  (back-compat 위해 둘 다 가능)

# 5. 빌드 + 테스트
cargo build -p oxios-memory
cargo test -p oxios-memory
cargo build -p oxios-kernel
cargo test -p oxios-kernel  # 739 tests pass 필수

# 6. CHANGELOG 업데이트

# 7. 커밋
git add -A
git commit -m "refactor(memory): move chunking/normalizer/hyperbolic to oxios-memory (b.1)"

# 8. push + PR
git push origin rfc-017-b1-chunking
gh pr create --title "RFC-018 b.1: Move chunking/normalizer/hyperbolic to oxios-memory" \
              --body "Implements RFC-018 §3 (b.1). See docs/rfc-018-memory-extraction-strategy.md"
```

### 3.4 검증 체크리스트 (PR 머지 전)

- [ ] `cargo build -p oxios-memory` — clean
- [ ] `cargo test -p oxios-memory` — 0 failed
- [ ] `cargo build -p oxios-kernel` — clean (2 warnings는 pre-existing, 무관)
- [ ] `cargo test -p oxios-kernel --lib` — **739 passed, 0 failed**
- [ ] `cargo fmt` — no changes
- [ ] `cargo clippy -D warnings` — no new warnings
- [ ] `oxios-kernel::TextChunk` re-export 동작 (back-compat)
- [ ] `oxios-kernel::HyperbolicEmbedding` re-export 동작
- [ ] `oxios-kernel::cosine_similarity_f32` re-export 동작

### 3.5 b.1 완료 후 다음 단계

b.1이 머지되면:
- **b.2 (embedding 모듈)** — `TfIdfEmbeddingProvider` + `embedding/gguf/`. 작업량 2-3일.
- 또는 **충분한 휴식** — 다음 세션에서.

---

## 4. 전체 단계 (9 sub-phases)

### 4.1 단계별 상세

#### b.1: chunking, normalizer, hyperbolic — *현재 단계*

- 의존성: stdlib + serde + chrono + anyhow만
- 다른 모듈과 결합 없음
- kernel의 `memory::chunking::X` → `oxios_memory::chunking::X`로 import 변경

**위험**: 낮음. **작업량**: 2-3일. **검증**: 739 tests pass.

#### b.2: embedding (TfIdfEmbeddingProvider)

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

- `MemoryManager`는 *여전히* kernel에 있지만 `Arc<StateStore>` → `Arc<dyn MemoryStorage>`로 변경
- 호출자(`store.rs`의 `state_store.X()`)는 *변경 없음* — trait 메서드와 동일 시그니처

**위험**: 중간. **작업량**: 3-4일. **검증 포인트**: 739 tests 모두 pass.

#### b.7: MemoryManager 이동 — *핵심 단계*

```bash
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

### 4.2 각 단계의 공통 패턴

```bash
# 1. 파일 이동
git mv crates/oxios-kernel/src/memory/X.rs crates/oxios-memory/src/memory/

# 2. oxios-memory/Cargo.toml 업데이트 (필요시)

# 3. oxios-memory/src/memory/mod.rs 업데이트
#    mod X; 또는 pub mod X;
#    pub use X::*;

# 4. oxios-kernel/src/lib.rs 업데이트
#    기존: pub use memory::X
#    신규: pub use oxios_memory::X  (back-compat 위해 둘 다 가능)

# 5. 빌드 + 테스트
cargo test -p oxios-kernel --lib  # 739 tests pass 필수

# 6. CHANGELOG 업데이트

# 7. 커밋 + PR
```

---

## 5. 영향 분석

### 5.1 변경되는 import 표면

| 이전 | 이후 |
|------|------|
| `use oxios_kernel::MemoryManager` | `use oxios_kernel::MemoryManager` (re-export로 호환) |
| `use oxios_kernel::MemoryEntry` | 동일 (re-export) |
| `use oxios_kernel::memory::MemoryEntry` | *deprecated* (1 release), `use oxios_kernel::MemoryEntry` 권장 |
| `use oxios_kernel::embedding::TfIdfEmbeddingProvider` | `use oxios_kernel::embedding::TfIdfEmbeddingProvider` (re-export) |

**Breaking change**: 0개 (모든 변경이 *back-compat* re-export 형태).

### 5.2 의존성 그래프 (after b.9)

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

### 5.3 빌드 매트릭스

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

---

## 6. 리스크

### 6.1 기술적 리스크

| 리스크 | 확률 | 영향 | 완화 |
|------|------|------|------|
| b.7에서 100+ import site 깨짐 | 높음 | 큰 PR | 단계별 commit으로 bisect 가능 |
| `dyn MemoryStorage` 호출자 코드 비대화 | 중간 | 코드 smell | helper method `MemoryManager::load_typed<T>()` 제공 |
| `SqliteMemoryStore`의 `Project` 결합 | 중간 | b.8 지연 | *trait bound* 또는 *kernel::Project* re-export |
| 테스트 fixture의 StateStore 사용 | 낮음 | b.6-b.7 | trait mock으로 대체 |
| chrono API 변경 (num_days 등) | 낮음 | 컴파일 에러 | 명시적 변환으로 해결 |

### 6.2 일정 리스크

| 리스크 | 영향 |
|------|------|
| 각 단계가 1-2일 추가 소요 가능 | 전체 3-4주 → 4-5주 |
| b.6-b.7 구간이 *예상보다 큰* 결합 드러냄 | 1주 추가 가능 |

### 6.3 비기술적 리스크

- **PR 리뷰 부담** — 10+ PR이 짧은 기간에 들어옴. PR template으로 *위험 평가* 명시
- **main branch 안정성** — 각 PR이 *테스트 통과* 상태를 유지하지 않으면 CI 실패

---

## 7. 비고

### 7.1 시도 3 (RFC-016 Phase B attempt 3)의 교훈

2026-06-04에 시도한 3번째 접근:
- `MemoryStorage` trait + `StateStore` impl까지는 성공
- *kernel 통합 단계*에서 `crate::memory::*` import 100+ site가 깨져서 revert

**교훈**: *facade 패턴을 넘어* 실제로 추출하려면 **kernel 측 import 일괄 변경**이 *먼저* 필요. 옵션 (B) b.6-b.7이 정확히 이 단계.

### 7.2 AGENTS.md와의 양립

AGENTS.md §10:
> "Kernel is intentionally monolithic."

**이 RFC는 이 원칙을 *단계적으로* 약화시킨다.** b.9 완료 시 kernel은 ~24,000 LoC (현재 53,000 LoC의 약 45%)가 되어 더 이상 "monolithic"가 아니다. **의도적 결정**.

### 7.3 미래 확장

- b.9 완료 후 *다른* 모듈 (tools, skill, a2a)도 같은 패턴으로 추출 가능
- RFC-016 §7.1의 "memory code 추출 1-2주" 추정치와 본 RFC의 3-4주 추정치 차이 — 본 RFC는 *10개* PR의 overhead 포함

---

## 8. 체크리스트 (PR별)

### b.1 — *현재 작업*
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

## 9. 새 세션을 위한 Quick Start

```bash
# 1. main 브랜치에서 최신 코드 받기
cd /Volumes/MERCURY/PROJECTS/oxios
git pull origin main
git log --oneline -5   # 현재 상태 확인

# 2. worktree 생성
git worktree add -b rfc-017-b1-chunking /Volumes/MERCURY/PROJECTS/oxios-b1

# 3. 작업 디렉토리로 이동
cd /Volumes/MERCURY/PROJECTS/oxios-b1

# 4. 빌드 baseline 확인
cargo build -p oxios-kernel --lib
cargo test -p oxios-kernel --lib
# → "test result: ok. 739 passed" 확인

# 5. b.1 단계 시작 (§3 참조)
# - chunking.rs, normalizer.rs, hyperbolic.rs 이동
# - oxios-memory/Cargo.toml 업데이트
# - mod.rs 업데이트
# - lib.rs re-export 업데이트
# - 739 tests pass 확인

# 6. PR 생성
git add -A
git commit -m "refactor(memory): move chunking/normalizer/hyperbolic to oxios-memory (b.1)"
git push origin rfc-017-b1-chunking
gh pr create --title "RFC-018 b.1: Move chunking/normalizer/hyperbolic to oxios-memory" \
              --body "Implements RFC-018 §3 (b.1)"

# 7. PR 머지 후 다음 단계로
# b.1 → b.2 (embedding) → b.3 (root_index/quota) → ...
```

### 9.1 새 세션이 *반드시* 알아야 할 것

1. **현재 main 브랜치 상태** — `f7bc72d` 이후 `oxios-memory` facade가 이미 있음
2. **테스트 baseline** — 739 passed (변경 시 *반드시* 이 숫자 유지)
3. **CI 명령** — `cargo fmt && cargo clippy -D warnings && cargo test --workspace`
4. **AGENTS.md §10** — "kernel is intentionally monolithic" (단계적으로 약화 중)
5. **RFC-016** — 이전 작업의 *설계 의도*와 *교훈*

### 9.2 새 세션이 *하지 말아야 할* 것

1. **Big Bang 변경** — 한 PR에 100+ 파일 변경. b.6-b.7이 *특히* 위험
2. **`crate::memory::*` import 일괄 sed** — 시도 3에서 실패한 패턴
3. **`MemoryManager`의 결합 강제 분리** — trait 없이 추출 시도. b.6까지 기다릴 것
4. **AGENTS.md §10 무시** — 모놀리식 → *단계적* 양립을 *명시*할 것
5. **CHANGELOG 생략** — 10개 PR이므로 *각 단계* 명시 필요

### 9.3 문제 발생 시

- **빌드 실패** — `cargo build -p oxios-kernel --lib`에서 *어디서* 깨졌는지 확인. b.6 이전이면 *단순 import* 문제일 가능성 높음
- **테스트 실패** — `cargo test -p oxios-kernel --lib`. 739가 *줄었다*면 *진짜 회귀*. PR 보류
- **trait dyn 호환성 문제** — `async_trait`의 한계. *helper method* (`load_typed<T>`)로 우회
- **`StateStore::load_json<T>` → `load_json_value` 변환** — `serde_json::from_value` 호출자 책임

---

## 10. 결정 (확정)

| 항목 | 결정 |
|------|------|
| **옵션 (B) 채택** | ✅ 단계적 추출, 9 sub-phases, 3-4주 |
| **다음 작업** | ✅ b.1 (chunking/normalizer/hyperbolic) — 즉시 |
| **Phase E (browser)** | 🟡 oxi-sdk 다운그레이드 or oxi-agent PR (별도 결정) |
| **AGENTS.md §10** | 단계적 양립 — "monolithic" → "kernel + memory" |
