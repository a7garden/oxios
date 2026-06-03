# Phase A: Cargo.toml 업그레이드

> **위험**: 낮음 (backward compatible)
> **예상 시간**: 10분
> **선행**: 없음

---

## 작업

### 1. workspace Cargo.toml 버전 변경

```toml
# 변경 전
oxi-sdk = "0.24.0"

# 변경 후
oxi-sdk = "0.26.0"
```

### 2. 컴파일 확인

```bash
cargo build
cargo test --workspace
```

0.26.0은 0.24.0의 모든 공개 API를 유지하므로, 소스 코드 수정 없이 컴파일되어야 한다.

### 3. 실행 확인

```bash
cargo run -- run --json "Hello, respond with one word"
```

---

## 검증 기준

- [ ] `cargo build` 성공
- [ ] `cargo test --workspace` 통과
- [ ] `cargo run -- run --json "test"` 정상 응답
