# Loop 4 — 품질/유지보수성 개선 설계

> 현재 점수: 70/100. 보안(P0)과 아키텍처 기반(P2)은 해결.
> 남은 것: Clippy 정리, 에러 일관성, routes.rs 분할, dead code 정리, CI 보강.
> 각 항목에 문제 원인 → 해결 방법 → 수정 파일 → 검증 방법 명시.

---

## 현재 상태

| 영역 | 점수 | 주요 잔여 문제 |
|------|------|----------------|
| Architecture | 80 | Orchestrator dead fields, routes.rs 1926줄 |
| Quality | 72 | 에러 응답 2종 혼재(StatusCode vs (StatusCode, String)), 에러 삼킴 17곳 |
| Security | 68 | 안정적. TODO 1개 (WS user_id 파생) |
| Performance | 65 | N+1 세션 읽기, spawn_blocking + block_on |
| DevOps | 55 | CI 있으나 cargo audit 없음, 릴리즈 파이프라인 없음 |
| Clippy | — | 7개 경고 (unused import, redundant closure, MutexGuard-await) |

---

## Step 0: Clippy 7개 경고 정리

목표: `cargo clippy --workspace -D warnings`가 oxios 크레이트에서 0 경고.

| # | 경고 | 위치 | 수정 |
|---|------|------|------|
| 1 | unused import `std::sync::Arc` | `src/main.rs:11` | 제거 (이미 `use std::sync::Arc` 있는데 중복 또는 미사용) |
| 2 | redundant closure | `agent_runtime.rs:218` | `rt.block_on(async { ... })` → `rt.block_on(pm.list_enabled())` 직접 호출 |
| 3 | MutexGuard held across await | `mcp.rs:811,818` | `self.servers.read()` → let 바인딩 후 수명 단축 |
| 4 | unused import `tokio::sync::Mutex` | `server.rs:27` | 제거 (Arc<McpBridge>로 변경 후 미사용) |
| 5 | unused import `std::sync::Arc` | `src/main.rs:11` | #1과 동일 |
| 6 | variable does not need to be mutable | `kernel.rs:152` | `let mut mcp_bridge` → `let mcp_bridge` (register_server가 &self) |
| 7 | variable does not need to be mutable | `kernel.rs:254` | `let mut mcp_bridge` → `let mcp_bridge` (init_mcp_bridge 내부) |

**검증:** `cargo clippy --workspace -- -D warnings` (oxi-ai 제외)

**커밋:** `chore: zero clippy warnings in oxios crates`

---

## Step 1: 에러 응답 통일 — AppError 전환

### 문제
routes.rs에 2가지 에러 타입이 혼재:
- `Result<T, StatusCode>` (17곳) — 에러 메시지 없음
- `Result<T, (StatusCode, String)>` (16곳) — 에러 메시지 있음
- `AppError` (0곳) — Loop 3에서 만들었으나 아직 사용 안 함

### 해결
모든 핸들러의 에러 타입을 `Result<T, AppError>`로 통일.

### 수정 방법
routes.rs 상단에 `use crate::error::AppError;` 추가 후,
각 핸들러의 `Err(StatusCode::NOT_FOUND)` → `Err(AppError::NotFound("...".into()))`,
`Err((StatusCode::BAD_REQUEST, msg))` → `Err(AppError::BadRequest(msg))` 형태로 교체.

### 에러 삼킴 정리
`Err(_) => Json(Vec::new())` 패턴 3곳 (agents, skills, sessions)을 적절한 에러로 변경:

```rust
// AS-IS: agents
Err(_) => Json(Vec::new()),

// TO-BE:
Err(e) => {
    tracing::error!(error = %e, "Failed to list agents");
    return Err(AppError::Internal("supervisor unavailable".into()));
}
```

memory/skills listing의 `Err(_) => continue`는 개별 항목 스킵이므로 유지 (정당함).

### 파일
- `channels/oxios-web/src/routes.rs` — 전체 핸들러 에러 타입 교체

### 검증
- `cargo build` 성공
- `cargo test --workspace` 통과
- API 에러 응답이 항상 `{"error": "..."}` JSON 형태

**커밋:** `refactor(web): unify API error responses to AppError`

---

## Step 2: routes.rs 분할 — 1926줄 → 5개 모듈

### 문제
routes.rs가 1926줄, 핸들러 51개. 유지보수 불가.

### 해결
기능별 5개 모듈로 분할:

```
channels/oxios-web/src/routes/
├── mod.rs          (~180줄) build_routes, 공통 타입, WsParams, TreeQuery, Body limit 상수
├── chat.rs         (~120줄) handle_chat, handle_chat_stream, handle_chat_websocket
├── system.rs       (~250줄) health, status, agents, config, sessions, approvals, audit, permissions
├── workspace.rs    (~200줄) workspace tree, file get/put, seeds, skills, memory
├── resources.rs    (~350줄) gardens, programs, host-tools, scheduler, MCP
└── events.rs       (~120줄) SSE handle_events, sanitize_event
```

persona_routes.rs는 그대로 유지 (이미 별도 파일).

### 분할 원칙
1. **mod.rs**: 라우트 등록 + 공통 타입 + `build_routes` 함수
2. **각 모듈**: `pub(crate) async fn handle_*` 형태, 필요한 타입 로컬 정의
3. `AppState`는 `crate::server::AppState`로 참조
4. `AppError`는 `crate::error::AppError`로 참조

### 의존성
- Step 1(AppError 전환) 선행 필요 — 분할 전에 에러 타입이 통일되어야 각 모듈이 깔끔함

### 파일
- `channels/oxios-web/src/routes.rs` → 삭제
- `channels/oxios-web/src/routes/mod.rs` — 신규
- `channels/oxios-web/src/routes/chat.rs` — 신규
- `channels/oxios-web/src/routes/system.rs` — 신규
- `channels/oxios-web/src/routes/workspace.rs` — 신규
- `channels/oxios-web/src/routes/resources.rs` — 신규
- `channels/oxios-web/src/routes/events.rs` — 신규
- `channels/oxios-web/src/lib.rs` — `pub mod routes;` 유지 (dir-based module)

### 검증
- `cargo build` 성공
- `cargo test --workspace` 통과
- 각 모듈 200~350줄 이내

**커밋:** `refactor(web): split routes.rs into 6 modules`

---

## Step 3: Dead code 정리

### 3-A: Orchestrator dead fields

Orchestrator의 5개 필드가 `#[allow(dead_code)]`로 표시됨.
lifecycle 이관 후 실제 사용처 조사 결과:

| 필드 | 사용처 | 조치 |
|------|--------|------|
| `supervisor` | 0 (lifecycle이 보유) | 제거 |
| `scheduler` | 1곳 (reap_zombies) | lifecycle으로 이관 후 제거 |
| `access_manager` | 1곳 (reap_zombies 로깅) | lifecycle으로 이관 후 제거 |
| `persona_manager` | 0 | 제거 |
| `a2a_protocol` | 0 (lifecycle이 보유) | 제거 |

`reap_zombies`를 `AgentLifecycleManager`로 이관:

```rust
// agent_lifecycle.rs에 추가
pub fn reap_zombies(&self) -> Vec<uuid::Uuid> {
    let reaped = self.scheduler.reap_zombies();
    if !reaped.is_empty() {
        tracing::warn!(count = reaped.len(), "Zombie tasks reaped");
        let mut access = self.access_manager.lock();
        for task_id in &reaped {
            access.log_access("scheduler", "zombie_reap", &task_id.to_string(), true, None);
        }
    }
    reaped
}
```

Orchestrator에서:
```rust
// 제거: supervisor, scheduler, access_manager, persona_manager, a2a_protocol 필드
// 유지: ouroboros, event_bus, state_store, sessions, lifecycle
```

### 3-B: 기타 dead code

| 항목 | 위치 | 조치 |
|------|------|------|
| `AgentRuntime::with_config` | `agent_runtime.rs:125` | 제거 (사용처 0) |
| `OuroborosEngine` unused fields | `ouroboros_engine.rs:102` | 필드 제거 또는 실제 사용 확인 |

### 파일
- `crates/oxios-kernel/src/orchestrator.rs` — 필드 제거, 생성자 단순화
- `crates/oxios-kernel/src/agent_lifecycle.rs` — reap_zombies 이관
- `crates/oxios-kernel/src/agent_runtime.rs` — with_config 제거
- `src/kernel.rs` — Orchestrator::new 인자 축소

### 검증
- `cargo build` 성공
- `#[allow(dead_code)]` 0개 (oxios-web/frontend 제외)

**커밋:** `refactor(kernel): remove dead code, move reap_zombies to lifecycle`

---

## Step 4: CI 보강

### 4-A: cargo audit 추가

```yaml
# .github/workflows/ci.yml에 추가
- name: Security audit
  run: |
    cargo install cargo-audit
    cargo audit
```

### 4-B: Clippy를 -D warnings로

```yaml
# 기존
run: cargo clippy --workspace -- -W warnings

# 변경
run: cargo clippy --workspace -- -D warnings
```
단 oxi-ai 외부 경고는 실패 원인이 되므로, 범위를 oxios 크레이트로 제한:
```yaml
run: cargo clippy -p oxios -p oxios-kernel -p oxios-ouroboros -p oxios-gateway -p oxios-web -- -D warnings
```

### 4-C: Justfile CI와 GitHub Actions 동기화

```make
# Justfile
ci: fmt-check lint test

lint:
    cargo clippy -p oxios -p oxios-kernel -p oxios-ouroboros -p oxios-gateway -p oxios-web -- -D warnings
```

### 파일
- `.github/workflows/ci.yml` — audit 단계 추가, clippy -D warnings
- `Justfile` — lint 레시피 업데이트

### 검증
- `just ci` 로컬 통과

**커밋:** `ci: add cargo audit, enforce -D warnings`

---

## 실행 순서

```
Step 0: Clippy 정리       (1 파일, 10분, 위험도 낮음)
Step 1: AppError 전환     (1 파일, 30분, 위험도 중간)
Step 2: routes.rs 분할    (7 파일 생성/삭제, 45분, 위험도 중간)
Step 3: Dead code 정리    (4 파일, 20분, 위험도 낮음)
Step 4: CI 보강           (2 파일, 10분, 위험도 낮음)
```

| Step | 파일 | 줄 변화 | 위험도 |
|------|------|---------|--------|
| 0 | 3 | ~20 | 낮음 |
| 1 | 1 | ~100 변경 | 중간 |
| 2 | 7 | ~1926 → ~1220 (5개 모듈) | 중간 |
| 3 | 4 | ~-80 | 낮음 |
| 4 | 2 | ~15 | 낮음 |

### 예상 점수

```
Step 0: 70 → 72  (Clippy 0)
Step 1: 72 → 75  (에러 일관성)
Step 2: 75 → 78  (유지보수성)
Step 3: 78 → 80  (dead code 제거)
Step 4: 80 → 82  (CI 신뢰성)
```

### Loop 4 이후 남는 것 (Loop 5+)

| 항목 | 이유 |
|------|------|
| spawn_blocking + block_on | oxi-agent Send 조사 필요 (외부 의존성) |
| N+1 세션 읽기 | 성능 개선, 세션 인덱스 파일 |
| 릴리즈 파이프라인 | 배포 자동화 |
| WS user_id 파생 | 사용자 시스템 설계 필요 |
| 프론트엔드 dead code | 별도 작업 |
