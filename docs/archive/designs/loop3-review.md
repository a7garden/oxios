# Loop 3 설계 리뷰

## 요약

12개 항목 중 **6개는 수정 필요**, **6개는 양호**. 설계 누락 3건, 접근법 수정 2건, 위험도 재평가 1건.

---

## 🔴 누락: Auth 미들웨어가 라우터에 연결되어 있지 않음

이것이 가장 치명적인 문제다.

**현황:** `middleware.rs`에 `require_auth` 함수가 구현되어 있으나, `build_routes`에서 **어떤 `.layer()` 호출도 없다.** 즉, `auth_enabled = true`로 설정해도 인증이 작동하지 않는다.

**설계의 문제:** Step 0-3이 "미들웨어 코드 수정"에만 집중하고, **실제 연결**을 다루지 않는다. 코드만 고치고 라우터에 안 넣으면 아무 효과가 없다.

**필요 조치:**
```
Step 0-3에 추가:
- build_routes()에서 /api/* 라우트 그룹에 .layer(from_fn_with_state(require_auth)) 적용
- 또는 Axum 0.8의 .layer(axum::middleware::from_fn(require_auth)) 사용
- 주의: 이전에 from_fn_with_state 타입 에러로 실패한 이력이 있음
```

**우선순위:** 이것이 안 되면 Step 0-3, 1-1 (WS 인증) 모두 의미 없음. Step 0의 **전제 조건**으로 격상 필요.

---

## 🔴 누락: SSE 이벤트 필터링

평가에서 H-04로 지적됨 ("모든 KernelEvent가 모든 SSE 구독자에게 브로드캐스트"). 그런데 설계의 1-1은 WS만 다루고 SSE는 누락.

**필요 추가:**
```
Step 1-1에 SSE 필터링 추가:
- handle_events()에서 이벤트 스트림에 사용자 컨텍스트 전달
- 민감한 이벤트(SeedCreated, EvaluationComplete 등)는 소유자에게만 전송
- 또는 익명화된 버전만 브로드캐스트
```

---

## 🔴 누락: 프로그램 설치 경로 검증

평가에서 H-02로 지적됨 ("`body.path`가 임의 파일시스템 경로를 허용"). 설계에 포함되지 않음.

**필요 추가:**
```
Step 0에 추가:
- handle_program_install에서 path를 program 디렉토리 내로 제한
- 또는 Local InstallSource를 금지하고 Git/Tarball만 허용
```

---

## 🟡 0-4 접근법 수정: MCP 등록

**설계의 제안:** McpBridge 내부에 `RwLock<Vec<McpServer>>` → `&self`로 register.

**문제 1:** `register_server`가 `spawn_blocking` 내부에서 호출됨. `tokio::sync::RwLock`은 `.await`이 필요한데 blocking context에서는 쓸 수 없음. 설계에 `blocking_push`라고 적었지만 `tokio::sync::RwLock`에는 `blocking_write()`가 없음.

**문제 2:** 더 간단한 대안이 있음. MCP 서버 등록은 `kernel.rs`의 `build()` 시점에 가능:

```
현재 흐름:
  kernel.rs: build() → Arc<McpBridge> 생성 → agent_runtime에 전달
  agent_runtime: spawn_blocking 내에서 program 목록 읽고 → register_server 호출

더 나은 흐름:
  kernel.rs: build() → program_manager.init() → program MCP 서버 수집
           → mcp_bridge에 register_server (아직 Arc로 안 감쌈)
           → Arc::new(mcp_bridge) → agent_runtime에 전달
```

kernel.rs의 `build()`에서 `mcp_bridge`를 `Arc::new()`으로 감싸기 **전에** 모든 서버를 등록하면 interior mutability가 필요 없음. `agent_runtime.rs`에서는 등록할 필요가 없어짐.

**권장:** RwLock 접근 대신 kernel.rs에서 사전 등록으로 변경.

---

## 🟡 1-1 접근법 수정: WebSocket user_id

**설계의 제안:**
```rust
struct ChatStreamParams {
    token: Option<String>,
    user_id: Option<String>,  // ← 클라이언트가 임의 지정
}
```

**문제:** `user_id`를 클라이언트가 제출하면 **신원 위조**가 가능. 사용자 A가 `user_id=B`로 접속하면 B의 메시지를 수신.

**수정:**
```
user_id를 토큰에서 파생. 클라이언트가 제출하지 않음.
AuthManager에 token→user_id 매핑 추가, 또는 JWT 도입.
```

최소한 alpha에서는:
```rust
struct ChatStreamParams {
    token: Option<String>,
    // user_id 제거 — 토큰에서 파생 또는 세션 ID 사용
}
// user_id = token의 해시 또는 자동 생성 UUID
```

---

## 🟡 1-4 위험도 재평가: AppError 표준화

**설계:** "점진적" 마이그레이션, 위험도 "중간".

**실제 위험도:** **높음**. 이유:

1. `routes.rs`는 1813줄, 핸들러 40+개. 점진적이면 혼재 기간이 길어짐
2. `AppError`가 `StatusCode`를 대체하면 모든 `match` 브랜치의 타입이 바뀜
3. `Result<Json<T>, StatusCode>` → `Result<Json<T>, AppError>`는 `IntoResponse` 구현이 달라서 일괄 변경 필요

**권장:** 
- Step 1-4를 Step 2로 이동 (보안/안정성 수정 후)
- 전용 Step으로 분리하여 routes.rs를 한 번에 변환
- 또는 AppError 도입을 Loop 4로 미루고, 지금은 `(StatusCode, String)`으로 통일하는 간이 접근 사용

---

## ✅ 양호한 항목

| 항목 | 평가 |
|------|------|
| **0-1 Path Traversal (tree)** | canonicalize + starts_with 패턴 정확. file_get의 기존 구현과 일관됨 |
| **0-2 Path Traversal (PUT)** | create_dir_all 후 반드시 canonicalize 재시도. 폴스루 제거. 정확함 |
| **0-3 Auth 우회 (prefix 매칭)** | suffix → prefix 전환이 올바른 방향. 실제 정적 자산 경로 반영 |
| **1-2 Body limit** | DefaultBodyLimit 레이어. 간단하고 효과적 |
| **1-3 expect 제거** | Result 전파. 파급 효과가 명확히 식별됨 |
| **2-3 Graceful shutdown** | 4단계 시퀀스가 합리적. 컨테이너 정지 보류 판단도 타당 |

---

## 2-2 Orchestrator 분리 — 보완 필요

Phase A (AgentLifecycleManager)는 좋은 시작이지만, 설계가 불완전:

**누락 1:** `EventBus::publish` 호출이 어디로 가는지 명시 안 됨. 현재 Orchestrator가 AgentForked/AgentCompleted 이벤트를 발행하는데, 이것도 lifecycle manager로 이동해야 함.

**누락 2:** Phase B (InterviewSessionStore), Phase C (ToolAccessPolicy)가 스케치만 있고 구체적인 타입 시그니처가 없음. "별도 타입으로"라고만 적혀 있음.

**권장:** Phase A만 이번 Loop에 구현. B/C는 Loop 4로. 한 번에 3-phase 변경은 위험도가 너무 높음.

---

## 2-1 Dead code — 접근법 수정

**설계:** `#[cfg(feature = "context-mgr")]` 또는 제거.

**문제:** alpha 단계에서 feature gate는 과잉 설계. 빌드 복잡도만 증가.

**권장:** 과감히 삭제. Git 히스토리에 남아 있으니 필요시 복원 가능.

---

## 점수 추정 재평가

| Step | 설계 추정 | 실제 예상 | 이유 |
|------|----------|----------|------|
| 0 (보안) | 45→58 | 45→**55** | 미들웨어 연결 누락이면 auth 효과 0. Path traversal + MCP만 해결 |
| 1 (안정성) | 58→66 | 55→**63** | AppError 전환 위험도 과소평가. WS user_id 위조 미해결 |
| 2 (아키텍처) | 66→72 | 63→**70** | Orchestrator Phase A만 실현 가능 |

**현실적 종착점:** 61 → **70** (설계의 72보다 2점 낮음)

---

## 수정된 권장 순서

```
Step 0-A: Auth 미들웨어 라우터 연결 (전제 조건)
Step 0-B: Path Traversal 2건 + Auth 우회 수정
Step 0-C: MCP 사전 등록 (kernel.rs에서 Arc 전에)
Step 0-D: 프로그램 설치 경로 검증 (누락 보충)

Step 1-A: Body limit (간단)
Step 1-B: expect 제거 (간단)
Step 1-C: WS 토큰 인증 (user_id는 토큰에서 파생)
Step 1-D: SSE 이벤트 필터링 (누락 보충)

Step 2-A: Dead code 삭제 (feature gate 말고 삭제)
Step 2-B: Graceful shutdown
Step 2-C: AgentLifecycleManager 추출 (Phase A만)
Step 2-D: AppError 표준화 (routes.rs 일괄 변환)
```

**Step 2-4 (spawn_blocking)은 Loop 4로 이동 권장.** oxi-agent의 AgentLoop Send 여부 조사가 선행되어야 하는데, 이것은 외부 의존성이라 설계 단계에서 확정 불가.
