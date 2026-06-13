# 우로보로스 모드 전환 — 기본 에이전트 + 선택적 Ouroboros

> **상태:** Approved  
> **날짜:** 2026-06-09  
> **영역:** oxios-kernel (orchestrator), oxios-gateway (gateway), oxios-web (chat UI)  
> **리뷰:** 2026-06-09 (이슈 7건 반영)

---

## 1. 문제

우로보로스가 파이프라인이 되어 모든 메시지가 무거움. 기본은 일반 에이전트, 우로보로스는 선택적 모드여야 함.

## 2. 설계

### 2.1 라우팅

```
IncomingMessage → Gateway::dispatch()
       │
       ├── mode == "spec"  ──→  Orchestrator.handle_message()  (우로보로스)
       └── default         ──→  Orchestrator.chat()             (일반 에이전트)
```

**키워드**: `#spec` (프리픽스만 매칭. `"#spec 리팩토링해줘"` → spec 모드)

**모드는 메시지별**: 같은 세션 안에서도 메시지마다 chat/spec을 자유롭게 선택.

### 2.2 Orchestrator::chat()

`spawn_and_run()` 사용. fork/A2A/scheduler 오버헤드 무시 가능 (~1ms vs LLM 수 초).

### 2.3 Config

```toml
[orchestrator]
spec_keywords = ["#spec", "#plan"]
default_mode = "chat"   # v1 배포에서는 "spec" (기존 동작 유지)
```

### 2.4 CLI 경로

`src/kernel.rs:execute_prompt_with_session()`도 Gateway처럼 모드 분기 필요.

## 3. 변경 파일

### Backend

| 파일 | 변경 |
|------|------|
| `crates/oxios-kernel/src/config.rs` | `OrchestratorConfig`에 `spec_keywords`, `default_mode` |
| `crates/oxios-kernel/src/orchestrator.rs` | `chat()`, `OrchestrationResult.mode` |
| `crates/oxios-gateway/src/meta.rs` | `MODE` 상수 |
| `crates/oxios-gateway/src/gateway.rs` | `dispatch()` 모드 분기, `detect_spec_mode()` |
| `crates/oxios-gateway/src/message.rs` | `ResponseMeta.mode` |
| `surface/oxios-web/src/routes/chat.rs` | WS 메시지에 `mode` 필드 파싱/전달 |
| `src/kernel.rs` | CLI 경로 모드 분기 |

### Frontend

| 파일 | 변경 |
|------|------|
| `web/src/stores/chat.ts` | `specMode` 상태 |
| `web/src/components/chat/chat-input.tsx` | 모드 토글 |
| `web/src/types/index.ts` | `mode` 필드 |

### 수정하지 않는 것

- `handle_message()` — 기존 우로보로스 그대로
- `agent_lifecycle.rs`, `agent_runtime.rs` — 독립적
- `oxios-ouroboros/` — 프로토콜 자체 불변

## 4. 리뷰 이력

| # | 이슈 | 결과 |
|---|------|------|
| 1 | `chat()`이 `spawn_and_run()` 사용 | OK — 오버헤드 무시 가능 |
| 2 | CLI 경로 (`kernel.rs`) 누락 | 반영 — 변경 파일에 추가 |
| 3 | 기존 `is_task == false` 중복 | 이후 정리 |
| 4 | Gateway 감지 위치 | OK |
| 5 | `mode` 필드 타입 | String |
| 6 | 모드는 메시지별, 세션 공유 | 명확화 |
| 7 | `default_mode` 기본값 | v1: `"spec"` (기존 동작 유지), 이후 `"chat"` 전환 |
| 8 | 인라인 키워드 | 프리픽스만 지원 |
