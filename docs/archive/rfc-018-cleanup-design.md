# RFC-018 메모리 추출 — 정리 설계

> **날짜**: 2026-06-05
> **상태**: 설계 완료, 구현 대기
> **목표**: 추출 완료 후 남은 5가지 smell을 정리

---

## 현재 상태

```
kernel/memory/mod.rs          94줄   (trait impl + re-export)
kernel/memory/auto_memory_bridge.rs  1002줄 (oxios_markdown 의존으로 잔존)
oxios-memory/memory/manager*.rs     1134줄 (3파일 분할)
```

## 문제 5가지

### P1. `manager*.rs` 3파일 분할이 부자연스럽다

**현재**:
```
manager.rs          221줄  struct + constructor + curate
manager_store.rs    535줄  CRUD, search, recall
manager_ops.rs      378줄  tier, HNSW, semantic, decay
```

**문제**: `impl MemoryManager`가 3개 파일에 흩어져 있음. Rust에서는 일반적이지만,
파일명이 `manager_store`, `manager_ops`로 명확하지 않음.

**해결**: 디렉토리 모듈로 전환.

```rust
// oxios-memory/src/memory/manager/mod.rs
mod store;   // CRUD, search, recall, blend
mod ops;     // tier, HNSW, semantic, decay, pin

pub use self::_struct::MemoryManager;

// _struct.rs 에 struct + constructor만
// store.rs 에 impl MemoryManager { remember, forget, list, search, recall, ... }
// ops.rs 에 impl MemoryManager { semantic_search, shift_tier, pin, ... }
```

**이유**: `manager_store.rs` → `manager/store.rs`가 더 자연스러움.
Rust 관용구 (`mod/` 디렉토리)와 일치.

---

### P2. `auto_memory_bridge` 1002줄이 kernel에 잔존

**현재**: `oxios_markdown::KnowledgeBase`에 4곳서 의존:
```rust
kb.index_all()?
kb.note_tree("/")?
kb.note_read(&path)?
kb.extract_headings(&content)?
```

**해결**: `MarkdownSource` trait을 `oxios-memory`에 정의하고,
kernel이 `KnowledgeBase`에 대해 구현.

```rust
// oxios-memory/src/memory/storage.rs 에 추가
#[async_trait]
pub trait MarkdownSource: Send + Sync {
    fn index_all(&self) -> anyhow::Result<usize>;
    fn note_tree(&self, prefix: &str) -> anyhow::Result<Vec<NoteEntry>>;
    fn note_read(&self, path: &str) -> anyhow::Result<Option<String>>;
    fn extract_headings(&self, content: &str) -> Vec<String>;
}

pub struct NoteEntry {
    pub name: String,
    pub parent_dir: String,
    pub is_dir: bool,
}
```

kernel/memory/mod.rs 에:
```rust
impl MarkdownSource for oxios_markdown::KnowledgeBase { ... }
```

이후 `auto_bridge.rs` 전체를 `oxios-memory`로 이동.

---

### P3. `DreamConfig::from_consolidation` 누락

**현재**: `from_consolidation`을 제거하고 `Default`만 남김.
`src/kernel.rs:742`에서 여전히 `from_consolidation` 호출 → **컴파일 안 됨**.

**해결**: kernel 측에 어댑터 함수 추가.

```rust
// kernel/memory/mod.rs 또는 kernel/config adapter
pub fn dream_config_from_consolidation(c: &ConsolidationConfig) -> DreamConfig {
    DreamConfig {
        dream_enabled: c.dream_enabled,
        dream_interval_hours: c.dream_interval_hours,
        // ... 필드 매핑
    }
}
```

또는 `DreamConfig`에 `From<&ConsolidationConfig>`를 kernel 측에서 구현.

---

### P4. re-export 폭발

**현재**: `kernel/memory/mod.rs` 94줄 중 ~50줄이 `pub use` 나열.
`kernel/lib.rs`에서도 중복 re-export.

**문제**: kernel 외부에서 실제로 쓰는 타입은 극소수:
```
crate::memory::MemoryManager
crate::memory::MemoryTier
crate::memory::ProtectionLevel
crate::memory::RecallTiming
crate::memory::content_hash
```

**해결**: 2층 re-export 정리.

```rust
// kernel/memory/mod.rs — 최소 re-export만
pub use oxios_memory::memory::manager::MemoryManager;
pub use oxios_memory::memory::types::{
    MemoryEntry, MemoryTier, MemoryType, ProtectionLevel, TextVector,
};
pub use oxios_memory::memory::proactive::RecallTiming;

// 나머지는 필요한 모듈에서 직접 oxios_memory 사용
```

`lib.rs`에서도 `pub use memory::` 대신 `pub use oxios_memory::`로 바로 가는 타입은 정리.

---

### P5. `oxios-memory/src/lib.rs`에 `MemoryManager`, `Dream`, `Proactive` re-export 누락

**현재**: `memory/mod.rs`에는 re-export가 있지만 `lib.rs`에는 없음.
kernel이 `oxios_memory::memory::manager::MemoryManager`로 직접 접근해야 함.

**해결**: `lib.rs`에 누락된 re-export 추가.

```rust
// oxios-memory/src/lib.rs
pub use crate::memory::manager::MemoryManager;
pub use crate::memory::dream::{DreamCheckpoint, DreamProcess, DreamReport, DreamConfig};
pub use crate::memory::proactive::{ProactiveRecall, RecallTiming};
```

---

## 구현 순서

```
P5 (lib.rs re-export)     ← 2분, 바로 수정
P3 (DreamConfig adapter)  ← 5분, 안 하면 binary 안 돌아감
P4 (re-export 정리)       ← 10분, 중복 제거
P1 (manager 디렉토리)     ← 15분, 파일 이동 + mod 선언
P2 (MarkdownSource trait) ← 30분, trait 설계 + impl + auto_bridge 이동
```

## 최종 구조

```
oxios-memory/src/
├── lib.rs                    (최상위 re-export, 깔끔)
└── memory/
    ├── manager/
    │   ├── mod.rs            (struct + constructor)
    │   ├── store.rs          (CRUD, search, recall)
    │   └── ops.rs            (tier, HNSW, pin, decay)
    ├── dream.rs
    ├── proactive.rs
    ├── auto_bridge.rs        ← P2 완료 후 이동
    ├── storage.rs            (MemoryStorage, MemoryGit, MarkdownSource)
    └── ...

kernel/memory/
├── mod.rs                    (~70줄, trait impl + 최소 re-export)
└── (auto_bridge.rs 삭제)     ← P2 완료 후
```
