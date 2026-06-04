# Phase G: 최종 정리

> **위험**: 낮음
> **예상 시간**: 1시간
> **선행**: Phase B~F 모두 완료

---

## 작업

### 1. Dead code 제거

Phase B~F 완료 후 더 이상 사용되지 않는 코드 정리:

```bash
# 미사용 import, dead code warning 확인
cargo clippy --workspace -- -W dead_code
```

### 2. lib.rs 정리

```rust
// crates/oxios-kernel/src/lib.rs
// audit_trail mod 제거됨 (Phase F)
// event_bus 간소화됨 (Phase C)
// AgentPool re-export (Phase E)
```

### 3. Cargo.toml 의존성 정리

oxios-kernel의 `Cargo.toml`에서 더 이상 필요 없는 직접 의존 제거
(모두 oxi-sdk를 통해서만 접근).

### 4. 주석 업데이트

`agent_runtime.rs`의 "oxi-sdk 0.23.0 Integration" 주석을
"oxi-sdk 0.26.0 Integration"으로 업데이트.

### 5. AGENTS.md 업데이트

`AGENTS.md`의 버전 정보 업데이트:

```markdown
| **SDK** | oxi-sdk 0.26.0 |
```

### 6. 최종 테스트

```bash
cargo test --workspace
cargo clippy --workspace
cargo run -- run --json "test prompt"
```

---

## 검증 기준

- [ ] `cargo test --workspace` 통과
- [ ] `cargo clippy --workspace` 경고 없음
- [ ] `cargo run -- run --json "test"` 정상 동작
- [ ] `git diff --stat`에서 절감 라인 수 확인
