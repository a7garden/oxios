# RFC-017: 메모리 시스템 복잡도 축소

> **상태:** 📝 설계
> **날짜:** 2026-05-26
> **우선순위:** P2
> **범위:** `crates/oxios-kernel/src/memory/`
> **선행:** 없음
> **후행:** 없음

---

## 1. 동기

메모리 시스템은 27개 파일, 12,196줄로 커널에서 가장 큰 서브시스템이다. 핵심 기능은 잘 설계되어 있으나, 여러 실험적 모듈이 실제 사용 여부와 무관하게 컴파일/유지보수 비용을 발생시킨다.

### 현재 모듈 분류

**핵심 (실사용 증명됨):**

| 모듈 | 줄 수 | 역할 |
|------|--------|------|
| `mod.rs` | 912 | MemoryManager 코어, TextVector, CRUD |
| `store.rs` | 1,132 | HnswMemoryIndex, 검색/저장 |
| `sqlite_store.rs` | 905 | SQLite 영속 백엔드 |
| `auto_memory_bridge.rs` | 999 | 자동 메모리 생성 브릿지 |
| `dream.rs` | 1,228 | Dream-time 통합 (백그라운드) |
| `database.rs` | 513 | SQLite 스키마/마이그레이션 |
| `migration.rs` | 246 | DB 마이그레이션 |
| `migrate.rs` | 249 | 데이터 마이그레이션 |
| `compaction.rs` | 184 | Raw→Daily→Weekly→Monthly 압축 트리 |
| `decay.rs` | 227 | Ebbinghaus 감쇠 |
| `cache.rs` | 202 | 메모리 캐시 |
| `store` + `database` | — | 영속성 + 스키마 |
| **핵심 소계** | **~6,797** | |

**실험적 (실사용 미증명):**

| 모듈 | 줄 수 | 역할 | 의문점 |
|------|--------|------|--------|
| `hyperbolic.rs` | 688 | 쌍곡선 공간 임베딩 | 계층적 메모리에 실제 이점? |
| `flash_attention.rs` | 591 | Flash attention 메커니즘 | 에이전트 메모리에 attention은 과잉? |
| `sona.rs` | 584 | 학습 엔진 | 무엇을 학습? 실제 에이전트 개선 측정? |
| `rvf_store.rs` | 571 | Retrieval Value Function | RL 기반 검색 가치 추정 — 실사용? |
| `reasoning_bank.rs` | 662 | 추론 은행 | Chain-of-thought 저장 — 검색으로 연결? |
| `graph.rs` | 268 | 메모리 그래프 | HNSW와의 차이/중복? |
| `root_index.rs` | 163 | ROOT 인덱스 | O(1) topic lookup — HNSW 대체? |
| `normalizer.rs` | 122 | 정규화 | TextVector와의 관계? |
| `embedding_cache.rs` | 251 | 임베딩 캐시 | SQLite store 내 캐시와 중복? |
| `auto_classify.rs` | 291 | 자동 분류 | auto_memory_bridge와 중복? |
| `auto_protect.rs` | 368 | 자동 보호 | 보호 기준이 실제로 의미? |
| `proactive.rs` | 219 | 사전 회상 | AgentRuntime에서 호출? |
| `budget.rs` | 42 | 메모리 예산 | — |
| `chunking.rs` | 262 | 텍스트 청킹 | — |
| **실험 소계** | **~5,322** | | |

---

## 2. 설계

### 2.1 원칙: 기능 게이트 (Feature Gate)

모듈을 삭제하지 않고 feature gate 뒤로 이동. 기본 빌드에는 핵심만 포함:

```toml
# Cargo.toml

[features]
default = ["sqlite-memory"]

# 핵심 (default)
sqlite-memory = ["dep:rusqlite"]

# 실험적 — 명시적 활성화 필요
memory-hyperbolic = []      # 쌍곡선 임베딩
memory-flash-attention = [] # Flash attention
memory-sona = []            # 학습 엔진
memory-rvf = []             # Retrieval Value Function
memory-reasoning = []       # 추론 은행
memory-graph = []           # 메모리 그래프
memory-proactive = []       # 사전 회상

# 전체 실험적 기능
memory-experimental = [
    "memory-hyperbolic",
    "memory-flash-attention",
    "memory-sona",
    "memory-rvf",
    "memory-reasoning",
    "memory-graph",
    "memory-proactive",
]
```

### 2.2 모듈 게이팅

```rust
// memory/mod.rs

#[cfg(feature = "memory-hyperbolic")]
pub mod hyperbolic;

#[cfg(feature = "memory-flash-attention")]
pub mod flash_attention;

#[cfg(feature = "memory-sona")]
pub mod sona;

#[cfg(feature = "memory-rvf")]
pub mod rvf_store;

#[cfg(feature = "memory-reasoning")]
pub mod reasoning_bank;

#[cfg(feature = "memory-graph")]
pub mod graph;

#[cfg(feature = "memory-proactive")]
pub mod proactive;

// 핵심은 항상 포함
pub mod auto_classify;
pub mod auto_memory_bridge;
pub mod auto_protect;
pub mod budget;
pub mod cache;
pub mod chunking;
pub mod compaction;
pub mod database;
pub mod decay;
pub mod dream;
pub mod embedding_cache;
pub mod hnsw;
pub mod migrate;
pub mod migration;
pub mod normalizer;
pub mod root_index;
pub mod search;
pub mod sqlite_store;
pub mod store;
```

### 2.3 사용처 조건부 컴파일

```rust
// agent_runtime.rs — 메모리 리콜

async fn recall_memory(&self, query: &str) -> Option<String> {
    // 핵심: 항상 사용 가능
    let memories = self.kernel.memory().search(query, 5).await.ok()?;

    // 실험적: 기능이 켜져 있을 때만
    #[cfg(feature = "memory-proactive")]
    {
        if let Some(proactive) = self.kernel.memory().proactive_recall(query).await.ok() {
            // 사전 회상 결과를 우선 사용
        }
    }

    Some(format_memories(&memories))
}
```

```rust
// dream.rs — Dream 프로세스

async fn consolidation_cycle(&self) -> Result<()> {
    // 핵심: 항상 수행
    self.compact_memories().await?;
    self.apply_decay().await?;

    // 실험적: 기능이 켜져 있을 때만
    #[cfg(feature = "memory-sona")]
    {
        self.sona().learn_from_recent().await?;
    }

    #[cfg(feature = "memory-rvf")]
    {
        self.rvf_store().update_values().await?;
    }

    Ok(())
}
```

### 2.4 설정에서 제어

```toml
# config.toml — 실험적 기능 토글 (feature gate와 함께 작동)

[memory]
recall_limit = 10
summarization_threshold = 50

[memory.experimental]
# 이 설정들은 해당 feature가 컴파일 타임에 활성화된 경우에만 의미 있음
hyperbolic_enabled = false
flash_attention_enabled = false
sona_mode = "off"           # "off" | "passive" | "active"
rvf_enabled = false
reasoning_bank = false
proactive_recall = false
```

---

## 3. 검증 계획

Feature gate 적용 전, 각 실험적 모듈의 실제 사용처를 조사:

### 사용처 감사 체크리스트

| 모듈 | `grep -r "use.*hyperbolic"` 결과 | AgentRuntime에서 호출? | Dream에서 호출? | 설정에서 제어? |
|------|----------------------------------|----------------------|----------------|--------------|
| `hyperbolic` | ? | ? | ? | ? |
| `flash_attention` | ? | ? | ? | ? |
| `sona` | ? | ? | ? | ? |
| `rvf_store` | ? | ? | ? | ? |
| `reasoning_bank` | ? | ? | ? | ? |
| `graph` | ? | ? | ? | ? |
| `proactive` | ? | ? | ? | ? |

**감사 결과에 따라:**
- **활성 사용됨** → feature gate 없이 핵심으로 승격
- **코드는 있으나 설정으로 비활성화** → feature gate 적용
- **코드는 있으나 호출부 없음** → `#[cfg(feature = "...")]` 게이트 + 별도 브랜치로 이동 검토

---

## 4. 마이그레이션 계획

### Phase 1: 사용처 감사 (1일)

| 작업 | 비고 |
|------|------|
| 각 모듈의 import/call site 전체 조사 | `grep`, `rg` |
| 호출 경로가 완결적인지 확인 | dead code path 탐지 |
| 감사 결과 문서화 | 각 모듈별 "활성/비활성/미사용" 분류 |

### Phase 2: Feature Gate 적용 (1일)

| 작업 | 비고 |
|------|------|
| `Cargo.toml`에 features 추가 | 기본: 없음, 선택: 각 모듈 |
| `mod.rs`에 `#[cfg(feature = "...")]` 적용 | 감사 결과 기반 |
| 사용처에 `#[cfg(...)]` 추가 | AgentRuntime, Dream, etc. |
| 컴파일 테스트 | `cargo build` (default + 각 feature) |

### Phase 3: CI 조정 (0.5일)

| 작업 | 비고 |
|------|------|
| 기본 CI: `cargo test` (핵심만) | 빌드 시간 단축 |
| 야간 CI: `cargo test --features memory-experimental` | 실험적 기능 회귀 방지 |

---

## 5. 기대 효과

| 지표 | 변경 전 | 변경 후 |
|------|---------|---------|
| 기본 컴파일 메모리 줄 수 | ~12,200 | ~6,800 |
| 기본 빌드 시간 | 기준 | 약 40% 감소 예상 |
| 인지 부하 (개발자) | 27개 모듈 파악 필요 | 15개 핵심 + 12개 명시적 옵트인 |
| 실험적 코드 손실 위험 | 없음 | 없음 (삭제 아닌 게이팅) |

---

## 6. 성공 기준

- [ ] 기본 `cargo build`에 실험적 메모리 모듈 미포함
- [ ] `cargo build --features memory-experimental`로 전체 포함 가능
- [ ] 각 실험적 모듈의 실제 사용처 문서화 완료
- [ ] 기존 테스트 전체 통과 (default + experimental)
- [ ] CI 빌드 시간 측정 및 개선 확인
