# Phase F: AuditTrail 중복 제거 — Subagent 프롬프트

---

## 컨텍스트

oxios-kernel은 oxi-sdk 0.26.2를 사용한다. Phase A(0.24.0→0.26.2 업그레이드)는 완료됐다.
oxi-sdk 0.26.2의 `audit_trail.rs`는 이미 활성화되어 있다 (crates.io에 게시됨).

**작업 디렉토리**: `/Volumes/MERCURY/PROJECTS/oxios`
**대상 crate**: `oxios-kernel` (의존 crate: 없음)
**RFC 문서**: `/Volumes/MERCURY/PROJECTS/oxios/docs/rfc-014/phase-f-audit-trail.md`
**메인 RFC**: `/Volumes/MERCURY/PROJECTS/oxios/docs/rfc-014-oxi-sdk-0.26-migration.md`

---

## 진행 방식: Git Worktree 격리 (필수)

이 Phase와 다른 Phase (B, D) 가 동시에 진행되므로, **반드시 별도 worktree에서 작업해야 한다**.

```bash
# 1. 메인 작업 트리에서 시작
cd /Volumes/MERCURY/PROJECTS/oxios

# 2. 깨끗한 main에서 새 worktree 생성
git worktree add ../oxios-phase-f-audit-trail -b phase/f-audit-trail main

# 3. worktree로 이동
cd ../oxios-phase-f-audit-trail

# 4. 이후 모든 작업은 여기서 진행
```

**절대 main에서 직접 작업하지 말 것.** 작업 완료 후 커밋된 브랜치만 메인으로 머지된다.

---

## 작업 내용

### 1. 사전 확인

먼저 SDK의 AuditTrail API를 확인:

```bash
SDK=/Users/won/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/oxi-sdk-0.26.2
echo "═══ audit_trail.rs 주요 타입 ═══"
grep -E "^pub (struct|enum|trait|fn|type)" $SDK/src/observability/audit_trail.rs
echo ""
echo "═══ AuditPersistence trait ═══"
grep -A6 "pub trait AuditPersistence" $SDK/src/observability/audit_trail.rs
echo ""
echo "═══ AuditAction enum (전체 variant) ═══"
grep -E "^\s+[A-Z][a-zA-Z]+\s*[{|(]" $SDK/src/observability/audit_trail.rs | head -30
```

oxios의 `audit_trail.rs`도 확인:

```bash
# oxios의 AuditEntry vs SDK의 TrailEntry 비교
diff <(grep -A1 "pub struct AuditEntry" crates/oxios-kernel/src/audit_trail.rs | head -20) \
     <(grep -A1 "pub struct TrailEntry" $SDK/src/observability/audit_trail.rs | head -20)
```

### 2. 작업 단계

RFC-014/phase-f-audit-trail.md의 "작업" 섹션 그대로 진행:

#### Step 1. StateStore용 `AuditPersistence` 구현을 새 모듈로 분리

`oxi_sdk::AuditPersistence` trait을 구현하는 모듈을 만든다.

```bash
# 새 파일 생성
touch crates/oxios-kernel/src/audit_persistence.rs
```

내용:
```rust
//! StateStore-backed AuditPersistence for oxi-sdk's AuditTrail.

use anyhow::Result;
use oxi_sdk::audit_trail::{AuditPersistence, TrailEntry};

use crate::state_store::StateStore;

impl AuditPersistence for StateStore {
    fn save(&self, entries: &[TrailEntry]) -> Result<()> {
        // crates/oxios-kernel/src/audit_trail.rs:500의 save_audit_entries 로직을 여기로
        let path = self.audit_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(entries)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    fn load(&self) -> Result<Vec<TrailEntry>> {
        // 기존 load_audit_entries 로직
        let path = self.audit_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let json = std::fs::read_to_string(&path)?;
        let entries: Vec<TrailEntry> = serde_json::from_str(&json)?;
        Ok(entries)
    }
}
```

**주의**: `audit_path()` 메서드는 `impl StateStore` 안에 정의되어 있다.
그대로 호출하면 된다. 단, `audit_trail.rs`에서 `audit_path()`를
호출하는 곳이 있는지 확인하고, 더 이상 audit_trail.rs에서 StateStore의
다른 메서드(`save_audit_entries`, `load_audit_entries`)를 사용하지 않으면
해당 메서드들을 audit_trail.rs에서 제거.

#### Step 2. oxios의 `audit_trail.rs` 삭제

```bash
# 기존 audit_trail.rs에서 어떤 것을 export 하고 있었는지 확인
grep -rl "use crate::audit_trail" crates/oxios-kernel/src/ | tee /tmp/audit_trail_users.txt

# lib.rs에서 mod audit_trail이 있는지 확인
grep "mod audit_trail" crates/oxios-kernel/src/lib.rs
```

이제 audit_trail.rs를 삭제:

```bash
# 새 audit_persistence.rs는 위에서 이미 생성했음
# 이제 옛날 audit_trail.rs는 SDK에서 가져올 것이므로 삭제
git rm crates/oxios-kernel/src/audit_trail.rs
```

#### Step 3. `lib.rs`에 새 모듈 등록 + SDK re-export

```bash
edit crates/oxios-kernel/src/lib.rs
```

다음과 같이 변경:
- `mod audit_trail;` → `mod audit_persistence;` (또는 `mod audit_persistence;` 추가)
- 모듈 끝 부분에 추가:
  ```rust
  pub use oxi_sdk::audit_trail::{
      AuditAction, AuditError, AuditPersistence, AuditTrail, HashDigest, TrailEntry,
  };
  ```

#### Step 4. 6개 파일 import 경로 변경

`/tmp/audit_trail_users.txt`에 나열된 모든 파일에 대해:

```rust
// 변경 전
use crate::audit_trail::{AuditAction, AuditTrail, AuditError, ...};
use crate::audit_trail::AuditTrail;
use crate::audit_trail::*;

// 변경 후
use oxi_sdk::audit_trail::{AuditAction, AuditTrail as SdkAuditTrail, AuditError, ...};
// 또는
use oxi_sdk::audit_trail::AuditTrail;
// 또는 (전부 와일드카드 사용 시)
use oxi_sdk::audit_trail::*;
```

**타입 이름 호환성**:
- oxios의 `AuditEntry` → SDK의 `TrailEntry` (이름만 다름, 구조는 동일)
- 다른 타입 (`AuditAction`, `AuditTrail`, `AuditError`, `HashDigest`, `AgentId`)은 이름이 동일함

**AuditAction variant 호환성 확인**:
- oxios의 `AuditAction::AgentSpawn { task_type }` → SDK에도 있는지 확인
- oxios의 `AuditAction::AgentExit { reason }` → SDK에도 있는지 확인
- 누락된 variant는 oxios에 별도 enum 확장으로 추가

#### Step 5. 옛 `flush()` 메서드 호출처 확인

```bash
grep -rn "audit_trail.*flush\|AuditTrail.*flush\|\.flush(" crates/oxios-kernel/src/ | head -20
```

SDK의 `AuditTrail`이 `flush()` 메서드를 제공하는지 확인:
```bash
grep "pub fn flush" $SDK/src/observability/audit_trail.rs
```

`AuditPersistence` trait을 통해 동일 동작이 가능하므로 옛 `flush(&StateStore)` 호출처가 있다면
다음과 같이 변경:
```rust
// 변경 전
trail.flush(&state_store).map_err(...)?;

// 변경 후
state_store.save(&trail.entries()).map_err(...)?;
```

#### Step 6. 옛 `save_audit_entries` / `load_audit_entries` 메서드 정리

이 메서드들이 다른 곳에서 호출되지 않는다면 `impl StateStore`에서 제거.
audit_persistence.rs의 `AuditPersistence::save/load`로 단일화.

```bash
grep -rn "save_audit_entries\|load_audit_entries" crates/oxios-kernel/src/
```

호출처가 있으면 그대로 유지, 없으면 제거.

### 3. 변경 범위 요약

| 파일 | 변경 |
|------|------|
| `crates/oxios-kernel/src/audit_trail.rs` | **삭제** |
| `crates/oxios-kernel/src/audit_persistence.rs` | **신규** (~30줄) |
| `crates/oxios-kernel/src/lib.rs` | mod 재구성 + re-export |
| `crates/oxios-kernel/src/access_manager/audit_sink.rs` | import 경로 |
| `crates/oxios-kernel/src/kernel_handle/security_api.rs` | import 경로 |
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | import 경로 |
| `crates/oxios-kernel/src/tools/kernel/security_tool.rs` | import 경로 |
| `crates/oxios-kernel/src/event_bus.rs` | import 경로 |
| `crates/oxios-kernel/src/agent_runtime.rs` | import 경로 |

다른 Phase와 파일 겹침:
- `event_bus.rs` — Phase C와 겹침 (이 Phase에서 `use crate::audit_trail`만 변경,
  Phase C가 import만 다시 정리하면 OK)
- `agent_runtime.rs` — Phase D와 겹침 (이 Phase에서 `use crate::audit_trail`만 변경,
  Phase D가 agent 생성 로직을 건드리면 충돌 → 머지 시 import 합치기)
- `kernel_handle/mod.rs` — Phase C와 겹침 (Phase C가 agent_api.rs의 import 변경,
  이 Phase가 security_api.rs의 import 변경)

→ 충돌 시: 이 Phase의 import 변경 라인은 그대로 두고, 머지 단계에서
`use oxi_sdk::audit_trail::{...}`와 `use oxi_sdk::event_bus::EventBus`를
한 줄로 합치면 됨.

---

## 검증

```bash
# 빌드 확인
cargo build -p oxios-kernel

# 테스트 확인
cargo test -p oxios-kernel

# AuditTrail 테스트 (해시 체인)
cargo test -p oxios-kernel audit_trail

# StateStore 연동 테스트
cargo test -p oxios-kernel state_store

# 회귀 확인
cargo test --workspace
```

기대 결과:
- `cargo build -p oxios-kernel`: 0 errors
- `cargo test --workspace`: 0 failed
- `git log --diff-filter=D --oneline`: `audit_trail.rs` 삭제 확인

---

## 커밋 형식

```bash
# 변경 사항 스테이지
git add -A

git commit -m "refactor(kernel): remove audit_trail duplicate, use oxi-sdk (RFC-014 Phase F)

- Delete crates/oxios-kernel/src/audit_trail.rs (1134줄)
- Add audit_persistence.rs with AuditPersistence impl for StateStore (~30줄)
- Re-export AuditAction, AuditTrail, TrailEntry, etc. from oxi_sdk::audit_trail
- Update 6 files to use oxi_sdk::audit_trail instead of crate::audit_trail
- Net: -1104 lines (95% duplication removed)

Tests: cargo test --workspace passes (0 failed)
Verify: AuditTrail hash chain integrity preserved via AuditPersistence trait"
```

---

## 완료 보고

다음 정보를 출력:
1. `git log --oneline -3` 결과
2. `git diff main --stat` 결과 (감소 라인 수가 음수로 표시되어야 함)
3. `cargo test --workspace` 마지막 10줄
4. AuditTrail/audit 통합 테스트 결과
5. 추가 / 발견한 사항 (있으면)

---

## 주의사항

- **이 Phase는 동작 변경이 아니다.** 해시 체인 알고리즘, 직렬화 형식, AuditAction 의미
  모두 그대로 유지. 단지 코드 위치만 SDK로 이동.
- **`HashDigest`와 `AgentId` 타입 alias는 SDK에서 가져온다** (oxios 자체 정의 삭제).
- **호환성을 위해 `AuditEntry as TrailEntry` re-export는 만들지 않는다.**
  깔끔하게 `TrailEntry`로 통일.
- **worktree 사용 절대 필수**. 다른 Phase와 파일이 일부 겹치므로
  worktree 격리는 필수다.
