# Phase A: Cargo.toml 업그레이드

> **위험**: 낮음 (backward compatible)
> **예상 시간**: 10분
> **선행**: 없음
> **상태**: ✅ 완료 (2026-06-03, 커밋 12c9990)

---

## 작업

### 1. workspace Cargo.toml 버전 변경

```toml
# 변경 전
oxi-sdk = "0.24.0"

# 변경 후
oxi-sdk = "0.26.2"
```

(0.26.2는 0.26.1의 dorman 모듈 활성화 작업을 포함함)

### 2. 컴파일 확인

```bash
cargo build
cargo test --workspace
```

### 3. 실행 확인

```bash
cargo run -- run --json "Hello, respond with one word"
```

---

## 부수적 변경 (0.26.2 API 적응)

`ExecutionResult`에 `tool_calls` 필드 추가로 4개 테스트 업데이트:

```rust
// crates/oxios-kernel/tests/e2e_test.rs
ExecutionResult {
    output: "...".into(),
    steps_completed: 3,
    success: true,
    tool_calls: vec![],  // 신규 필수 필드
}
```

Doctest 경로 수정 (4개):

- `oxios_kernel::a2a_circuit_breaker::*` → `oxios_kernel::a2a::circuit_breaker::*`
- `oxios_kernel::clawhub::*` → `oxios_kernel::skill::clawhub::*`
- `oxios_kernel::skills_sh::*` → `oxios_kernel::skill::skills_sh::*`
- `oxios_kernel::coordination::*` → `oxi_sdk::coordination::*`

`workers.rs`의 `dispatch` 반환 타입 변경: `Result<String, String>` → `Result<ExecutionResult, WorkerError>`. 테스트가 `result.is_err()` 대신 `!result.success`를 확인하도록 변경.

`compaction.rs` 테스트 임계값: 20줄 → 50줄 (이전 임계값 하에서는 compaction이 발생하지 않음).

---

## 검증 기준

- [x] `cargo build` 성공
- [x] `cargo test --workspace` 통과 (0 failed)
- [x] `cargo run -- run --json "test"` 정상 응답
