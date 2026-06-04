# Phase B: Observability 정리 — Subagent 프롬프트

---

## 컨텍스트

oxios-kernel은 oxi-sdk 0.26.2를 사용한다. Phase A(0.24.0→0.26.2 업그레이드)는 완료됐다.
이제 RFC-014에 따라 Phase B를 독립적으로 진행한다.

**작업 디렉토리**: `/Volumes/MERCURY/PROJECTS/oxios`
**대상 crates**: `oxios-kernel` (의존: `oxios-ouroboros`는 read-only)
**RFC 문서**: `/Volumes/MERCURY/PROJECTS/oxios/docs/rfc-014/phase-b-observability.md`
**메인 RFC**: `/Volumes/MERCURY/PROJECTS/oxios/docs/rfc-014-oxi-sdk-0.26-migration.md`

---

## 진행 방식: Git Worktree 격리 (필수)

이 Phase와 다른 Phase (D, F) 가 동시에 진행되므로, **반드시 별도 worktree에서 작업해야 한다**.

```bash
# 1. 메인 작업 트리에서 시작
cd /Volumes/MERCURY/PROJECTS/oxios

# 2. 깨끗한 main에서 새 worktree 생성
git worktree add ../oxios-phase-b-observability -b phase/b-observability main

# 3. worktree로 이동
cd ../oxios-phase-b-observability

# 4. 이후 모든 작업은 여기서 진행
```

**절대 main에서 직접 작업하지 말 것.** 작업 완료 후 커밋된 브랜치만 메인으로 머지된다.

---

## 작업 내용

### 1. 현재 상태 파악

먼저 `crates/oxios-kernel/src/observability.rs`를 읽고 다음을 확인:
- 현재 re-export되는 타입 목록
- `init()` 함수 존재 여부
- 글로벌 OnceLock 인스턴스 (TRACER, COST_TRACKER, AUDIT_LOG)
- `kernel.rs`에서 `observability::init()`이 호출되는지

```bash
# 1) observability.rs 읽기
cat crates/oxios-kernel/src/observability.rs

# 2) 사용처 확인
grep -rl "use crate::observability\|observability::init\|observability::TRACER" \
  crates/oxios-kernel/src/
```

### 2. 작업

RFC-014/phase-b-observability.md의 "작업" 섹션 그대로 진행:

**a) 불필요한 re-export 정리**

`observability.rs` 상단의 `pub use oxi_sdk::{...}`를 검토하여:
- 사용되지 않는 타입 제거
- 누락된 타입 추가
- 카테고리별 주석 추가 (Tracing / Cost / Audit)

oxi-sdk 0.26.2에서 사용 가능한 타입은 다음으로 확인:
```bash
# oxi-sdk 0.26.2의 observability 모듈
SDK=/Users/won/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/oxi-sdk-0.26.2
cat $SDK/src/observability/mod.rs
```

**b) `init()` 함수가 부팅 시 호출되는지 검증**

`crates/oxios-kernel/src/kernel.rs`에서 `observability::init()` 호출을 찾는다.
- 있으면 그대로 유지
- 없으면 추가 (단, `Oxi` 객체가 필요한 init은 호출 순서 확인)

### 3. 변경 범위

| 파일 | 변경 종류 |
|------|----------|
| `crates/oxios-kernel/src/observability.rs` | re-export 정리, 주석 추가 |
| `crates/oxios-kernel/src/kernel.rs` | init() 호출 누락 시만 |

다른 Phase와 파일 충돌 없음. 이 두 파일만 건드려야 한다.

---

## 검증

```bash
# 빌드 확인
cargo build -p oxios-kernel

# 테스트 확인
cargo test -p oxios-kernel

# 전체 워크스페이스 (regression)
cargo test --workspace
```

기대 결과:
- `cargo build -p oxios-kernel`: 0 errors
- `cargo test --workspace`: 0 failed (기존 5개 경고는 동일)
- `cargo clippy -p oxios-kernel -- -D warnings`: 통과

---

## 커밋 형식

```bash
git add crates/oxios-kernel/src/observability.rs \
        crates/oxios-kernel/src/kernel.rs  # 변경 시에만

git commit -m "refactor(kernel): clean up observability re-exports (RFC-014 Phase B)

- Review pub use oxi_sdk::{...} in observability.rs
- Add category comments (Tracing/Cost/Audit)
- Verify observability::init() is called from kernel.rs
- No functional changes, just code hygiene"
```

---

## 완료 보고

다음 정보를 출력:
1. `git log --oneline -3` 결과
2. `git diff main --stat` 결과 (변경 라인 수)
3. `cargo test --workspace` 마지막 5줄
4. 추가 / 발견한 사항 (있으면)

---

## 주의사항

- **외부 re-export 절대 변경 금지**: `pub use oxi_sdk::Span` 같은 것을
  `pub use crate::observability::Span` 으로 바꾸면 다른 모듈이 깨진다.
  모든 re-export는 그대로 유지하되, 그룹화 주석만 추가하는 것이 핵심.
- **이 Phase는 동작 변경이 없다**. 깨끗한 main에서 시작하므로 다른 Phase와
  자연스럽게 머지 가능해야 한다.
- **worktree 사용 절대 필수**. 동시에 진행되는 Phase D (engine.rs) 와
  Phase F (audit_trail.rs) 와 파일이 겹치지 않지만, worktree 격리는
  cherry-pick / 머지 시 충돌 방지 표준 절차다.
