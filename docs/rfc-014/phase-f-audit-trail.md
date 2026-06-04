# Phase F: AuditTrail 중복 제거

> **위험**: 중간 (6개 파일 import 변경)
> **예상 시간**: 3시간
> **선행**: oxi-sdk에서 audit_trail.rs 활성화 (dormant 문서 Step 3~4)

---

## 전제 조건

oxi-sdk의 `observability/audit_trail.rs`가 활성화되어야 한다:
- `Cargo.toml`에 `blake3`, `chrono` 의존 추가
- `observability/mod.rs`에 `mod audit_trail;` 등록
- `AuditTrail`, `TrailEntry`, `AuditAction`, `AuditError`, `AuditPersistence`, `HashDigest` re-export

→ oxi 측 `sdk-dormant-modules-activation.md` Step 3~4

## 현재 상태

```
crates/oxios-kernel/src/audit_trail.rs  → 1134줄 (oxios 구현)
oxi-sdk/src/observability/audit_trail.rs → 973줄 (SDK 구현, oxios에서 마이그레이션됨)
```

두 파일은 95% 동일하다. 유일한 차이:
- oxios: `StateStore`에 직접 의존
- SDK: `AuditPersistence` trait으로 추상화

## 작업

### 1. oxios의 audit_trail.rs 삭제

```bash
rm crates/oxios-kernel/src/audit_trail.rs
```

### 2. lib.rs에서 mod audit_trail 제거

```rust
// crates/oxios-kernel/src/lib.rs — 삭제
// mod audit_trail;
```

### 3. SDK에서 re-export

```rust
// crates/oxios-kernel/src/lib.rs 또는 적절한 위치에 추가
pub use oxi_sdk::{
    AuditAction, AuditError, AuditPersistence, AuditTrail, HashDigest, TrailEntry,
};
```

### 4. import 경로 변경

| 파일 | 변경 전 | 변경 후 |
|------|---------|---------|
| `access_manager/audit_sink.rs` | `use crate::audit_trail::*` | `use oxi_sdk::{AuditAction, AuditTrail as SdkAuditTrail, ...}` |
| `kernel_handle/security_api.rs` | `use crate::audit_trail::*` | `use oxi_sdk::{AuditTrail, ...}` |
| `tools/kernel/security_tool.rs` | `use crate::audit_trail::*` | `use oxi_sdk::{AuditTrail, ...}` |
| `kernel_handle/mod.rs` | `use crate::audit_trail::AuditTrail` | `use oxi_sdk::AuditTrail` |
| `event_bus.rs` | `use crate::audit_trail::*` | `use oxi_sdk::{AuditAction, ...}` |
| `agent_runtime.rs` | `use crate::audit_trail::AuditTrail` | `use oxi_sdk::AuditTrail` |

### 5. StateStore 연동 유지 (oxios-kernel 전용)

oxios의 `StateStore`는 `AuditPersistence` trait을 구현한다:

```rust
// crates/oxios-kernel/src/state_store.rs 또는 적절한 위치
impl oxi_sdk::AuditPersistence for StateStore {
    fn save(&self, entries: &[oxi_sdk::TrailEntry]) -> anyhow::Result<()> {
        // 기존 save_audit_entries() 로직 유지
    }

    fn load(&self) -> anyhow::Result<Vec<oxi_sdk::TrailEntry>> {
        // 기존 load_audit_entries() 로직 유지
    }
}
```

### 6. AuditTrail 관련 타입 alias 유지 (호환성)

```rust
// 기존 코드에서 사용하는 타입들이 그대로 동작하도록
pub use oxi_sdk::TrailEntry as AuditTrailEntry;  // 필요시
```

## 검증 기준

- [ ] `cargo build -p oxios-kernel` 성공
- [ ] `cargo test -p oxios-kernel` 통과
- [ ] AuditTrail 해시 체인 무결성 유지 (`verify()` 동작)
- [ ] StateStore 연동 정상 (save/load)
- [ ] audit JSONL 파일 출력 정상
