# RFC-033: Unified Streaming Orchestration — assess 게이트 제거, 모든 메시지 스트리밍

> **상태:** Implemented (2026-06-28)
> **날짜:** 2026-06-28
> **영역:** oxios-ouroboros (IntentEngine), oxios-kernel (orchestrator, agent_runtime), oxios-gateway, src/api/routes/chat.rs, web (chat store)
> **관계:** [RFC-015 Chat Transparency](rfc-015-chat-transparency.md)의 완성. [RFC-027 Unified Intent Handling](rfc-027-unified-intent-handling.md)의 후속.
> **목표:** Claude/Gemini 수준의 응답 스트리밍을 **모든 메시지**에 적용.
>
> **구현 노트:** assess/crystallize 게이트 제거 + `handle()` 단일 스트리밍 경로 통합 완료.
> 구현 중 §9의 WS 디스패치 버그를 **전제 조건**으로 먼저 수정(`incoming_tx.send()` 누락).
> 추가로 **스트리밍 전달의 근본 원인**을 발견·수정: 런타임의 `transparency_session`이
> 랜덤 `agent_id`를 써서 게이트웨이가 등록한 `session_id`와 불일치 → 토큰/도구/thinking
> 델타가 클라이언트에 도달하지 못함. `ExecEnv.session_id`를 경유해 채팅 세션 키를
> 런타임까지 전달하고, 게이트웨이 sink 등록/해제/partial 메타데이터를 모두
> `session_id.unwrap_or(request_id)` resolved 키로 통일. 첫 메시지(세션 미확정) 포함.
> `review`는 criteria-bearing Directive에서만 발화하므로 인터랙티브 채팅에서는 dormant
> (에이전트 내재적 VERIFY가 대체). cron executor가 wiring되지 않아 현재 criteria 생산자 없음.

---

## 1. 동기

RFC-015가 구축한 투명성 인프라(phase 이벤트, 타자기 텍스트, thinking 스트림, 도구 가시성)는 **Task로 분류된 메시지에만** 작동한다. 일상 대화("안녕", 단순 Q&A)는 `assess()` 단계에서 답변 전체가 비스트리밍 JSON으로 생성되어 통째로 반환된다.

사용자의 핵심 불만:
> "굳이 이거 판단하느라 LLM 호출을 하나 소모하는 거 아니야?"

맞다. `assess()`는:
1. **LLM 호출 1회를 소모**하여 메시지를 분류(conversation/clarify/task)
2. Conversation 분류 시 **답변 전체를 JSON `reply` 필드로 생성** → 비스트리밍
3. Task 분류 시에도 **추가 LLM 호출** 필요 (crystallize + execute + review)

Claude.ai / Gemini Web은 사전 분류 단계 없이 모든 응답을 스트리밍한다. 모델의 지능이 곧 분류기다.

---

## 2. 현재 아키텍처

```
User Message
  → handle_unified()
    → handle()
      → assess() [LLM 호출 #1: 분류 + conversation 답변 생성]
      → match assessment:
          Conversation(reply) → HandleResponse::Reply(reply)     ← SHORT-CIRCUIT
          Clarify(questions)  → HandleResponse::Clarify(...)     ← SHORT-CIRCUIT
          Task(scope) →
            Trivial    → Directive::from_message(msg)
            Substantial → crystallize() [LLM 호출 #2]
            → execute_directive() → run_streaming() [LLM 호출 #3: 스트리밍 + 도구]
            → review() [LLM 호출 #4, Substantial만] → retry 가능
```

**LLM 호출 수:**
| 메시지 유형 | 현재 | 비고 |
|---|---|---|
| Conversation | 1회 (assess) | 비스트리밍, 답변 통째 |
| Task::Trivial | 2회 (assess + execute) | 스트리밍 |
| Task::Substantial | 3–4회 (assess + crystallize + execute + review) | 스트리밍 |

**가시성:**
| 메시지 유형 | phase | 타자기 | 도구 | thinking |
|---|---|---|---|---|
| Conversation | `assess`만 | ❌ | ❌ | ❌ |
| Task::Trivial | `assess` → `execute` | ✅ | ✅ | (옵션) |
| Task::Substantial | `assess` → `plan` → `execute` → `review` | ✅ | ✅ | (옵션) |

---

## 3. 설계: Unified Streaming

### 3.1 핵심 원칙

**분류는 에이전트의 지능으로, 외부 게이트가 아니다.**

모든 메시지를 `run_streaming` 에이전트 루프로 보낸다. 에이전트가 스트리밍 응답 안에서 자연스럽게:
- 단순 인사 → 그냥 답변 (도구 없음, 타자기 효과)
- 코딩 작업 → 도구 사용 (도구 카드 표시)
- 모호한 요청 → 명확화 질문 (`ask_user` / `pi-questionnaire` 도구)

이것이 Claude.ai / Gemini Web의 구조다.

### 3.2 새 아키텍처

```
User Message
  → handle_unified()
    → Directive::from_message(msg)         // 경량: 메시지를 goal로
    → execute_directive(directive, env)
      → run_streaming(prompt, callback)    // 모든 메시지가 여기를 통과
        → 에이전트 스트리밍 응답
          → 단순 대화: 그냥 답변 (스트리밍, 타자기)
          → 복잡 작업: 도구 사용 (타임라인에 표시)
          → 모호함: 질문 (ask_user / pi-questionnaire)
        → 모든 이벤트가 StreamingSink로 실시간 전달
    → (선택) 외부 review — acceptance_criteria가 있을 때만
    → Done
```

**LLM 호출 수 (개선 후):**
| 메시지 유형 | 개선 후 | 절약 |
|---|---|---|
| Conversation | 1회 (에이전트 스트리밍) | 0회 (동일하나 이제 스트리밍) |
| Task::Trivial | 1회 (에이전트 스트리밍) | **1회 절약** (assess 제거) |
| Task::Substantial | 1–2회 (에이전트 + 선택적 외부 review) | **2회 절약** (assess + crystallize 제거) |

**가시성 (개선 후):**
| 메시지 유형 | phase | 타자기 | 도구 | thinking |
|---|---|---|---|---|
| 모든 메시지 | `execute` (+에이전트 내재적 plan/verify) | ✅ | ✅ | (옵션) |

### 3.3 Ouroboros 프로토콜의 내재화

현재 assess/crystallize/review는 **외부 LLM 게이트**다. 각각 별도 system prompt(`ASSESS_SYSTEM_PROMPT`, `CRYSTALLIZE_SYSTEM_PROMPT`, `REVIEW_SYSTEM_PROMPT`)으로 독립적인 LLM 역할을 수행한다.

개선 후, 프로토콜 단계는 에이전트 시스템 프롬프트의 **행동 지침**으로 내재화된다:

| 단계 | 현재 (외부 게이트) | 개선 후 (내재화) |
|---|---|---|
| **assess** | 별도 LLM 호출로 분류 | **제거** — 에이전트가 응답 안에서 자연스럽게 판단 |
| **crystallize** | 별도 LLM 호출로 Directive 생성 | **시스템 프롬프트 지침** — UNDERSTAND → PLAN 단계에서 자연스럽게 수행 (thinking 패널에 표시) |
| **review** | 별도 LLM 호출로 검증 | **선택적 외부 review** — `acceptance_criteria`가 있을 때만 post-execution 게이트 유지 (자기 검토 편향 방지) |

에이전트 시스템 프롬프트(`agent_runtime.rs:1577`)에는 이미 다음 실행 프로토콜이 내장되어 있다:

```
UNDERSTAND → PLAN → EXECUTE → VERIFY → REPORT
```

이것이 곧 Ouroboros 프로토콜이다. 외부 게이트가 이를 중복한다.

### 3.4 Clarify (인터뷰 위저드) 보존

**현재:** `assess()`가 `Assessment::Clarify`를 반환 → 구조화된 질문 JSON → 프론트엔드 인터뷰 위저드 렌더링.

**개선 후:** 에이전트가 모호성을 감지하면 **에이전트 도구**로 명확화를 수행:

- `ask_user` — 이미 존재하는 도구 (`tools/builtin/mod.rs`). 자유 텍스트 질문.
- `pi-questionnaire` — 이미 별도 존재하는 구조화 질문 도구 (`questionnaire-card.tsx`). 단일/다중선택, yes/no 지원.

에이전트가 시스템 프롬프트 지침("요청이 모호하면 실행 전에 명확화하라")에 따라 자연스럽게 이 도구들을 호출. 인터뷰 위저드 UI는 그대로 작동 — 단, assess 게이트가 아닌 에이전트 도구 호출로 트리거.

**이중 이점:**
1. 모든 메시지가 스트리밍 (clarify 필요 없는 경우)
2. 구조화된 명확화는 에이전트 판단으로 발생 (불필요한 위저드 차단)

### 3.5 외부 Review 보존 (선택적)

자기 검토 편향(self-review bias)을 방지하기 위해, `acceptance_criteria`가 있는 작업(Scope::Substantial에 해당)은 post-execution 외부 review를 유지한다:

```
execute_directive → 결과 생성
  → Directive에 acceptance_criteria가 있으면
    → 외부 review() LLM 호출 (impartial evaluator)
    → 실패 시 gaps를 constraints에 주입하고 retry
```

이것은 현재 `verify_or_retry()` 로직과 동일. 단, assess/crystallize 없이 execute 직후에만 실행.

---

## 4. 구현 범위

### 4.1 백엔드 — `orchestrator.rs`

**`handle()` 재작성:**

```rust
pub async fn handle(&self, engine: &dyn IntentEngineOps, msg: &str, ctx: &MsgCtx)
    -> Result<HandleResponse>
{
    // assess 제거. 모든 메시지를 execute로.
    let _ = self.event_bus.publish(KernelEvent::PhaseStarted {
        session_id: ctx.session_id.clone(),
        phase: "execute".to_string(),
        summary: None,
    });

    // Directive: 메시지를 goal로 사용 (경량)
    let directive = Directive::from_message(msg);
    let env = self.resolve_exec_env(ctx, msg);

    let mut result = self.execute_directive(&directive, &env).await?;

    let _ = self.event_bus.publish(KernelEvent::PhaseCompleted {
        session_id: ctx.session_id.clone(),
        phase: "execute".to_string(),
    });

    // 선택적 외부 review (acceptance_criteria가 있을 때만)
    if directive.needs_review() {
        let _ = self.event_bus.publish(KernelEvent::PhaseStarted {
            session_id: ctx.session_id.clone(),
            phase: "review".to_string(),
            summary: None,
        });
        let (r, v) = self.verify_or_retry(engine, &mut directive, &env, result, msg, ctx).await?;
        result = r;
        let _ = self.event_bus.publish(KernelEvent::PhaseCompleted {
            session_id: ctx.session_id.clone(),
            phase: "review".to_string(),
        });
        Ok(HandleResponse::Task { scope: Scope::Substantial, directive: Box::new(directive), result, verdict: Some(v), evaluation_passed: Some(v.all_passed()) })
    } else {
        Ok(HandleResponse::Task { scope: Scope::Trivial, directive: Box::new(directive), result, verdict: None, evaluation_passed: None })
    }
}
```

**제거 대상:**
- `assess()` 호출 및 `match assessment` 분기
- `crystallize()` 호출 (Substantial 경로)
- `Assessment::Conversation` / `Assessment::Clarify` 처리
- `publish_phase("assess")` / `publish_phase("plan")` (이제 에이전트 루프 안에서 자연스럽게)

**유지 대상:**
- `verify_or_retry()` — 외부 review (acceptance_criteria 있을 때만)
- `Directive` 구조체 — 그대로 사용
- `execute_directive()` — 그대로 사용

### 4.2 백엔드 — `engine.rs` / `prompts.rs`

- `assess()` / `crystallize()` 메서드는 **폐기 또는 deprecated** 표시
- `ASSESS_SYSTEM_PROMPT`, `CRYSTALLIZE_SYSTEM_PROMPT` — 폐기
- `IntentEngineOps` trait에서 `assess`/`crystallize` 제거 (또는 `#[deprecated]`)
- `REVIEW_SYSTEM_PROMPT` / `review()` — 유지 (외부 review용)

### 4.3 백엔드 — `handle_response_to_orchestration_result()`

`HandleResponse::Reply` / `HandleResponse::Clarify` 변종 제거. 모든 응답이 `HandleResponse::Task`로 통일:

```rust
// Before: 3개 변종 (Reply, Clarify, Task)
// After:  1개 변종 (Task) — phase_reached 항상 "execute"
```

### 4.4 에이전트 시스템 프롬프트 — `agent_runtime.rs`

실행 프로토콜 섹션 강화 (`build_system_prompt_inner()`):

```
## Execution Protocol
1. UNDERSTAND — Read the user's request carefully. If it's a simple
   greeting or question, respond naturally and conversationally.
2. PLAN — For complex tasks, outline your approach before acting.
3. EXECUTE — Use tools as needed. For simple requests, no tools needed.
4. VERIFY — Check your work against any criteria.
5. REPORT — Summarize what you did.

If the request is ambiguous, use the ask_user or pi-questionnaire tool
to clarify before executing.
```

이 지침은 이미 부분적으로 존재한다 (`UNDERSTAND → PLAN → EXECUTE → VERIFY → REPORT`). 강화만 하면 된다.

### 4.5 프론트엔드 — 변경 최소

스트리밍 파이프라인은 이미 완성되어 있으므로(RFC-015), 프론트엔드 변경은 최소:

- `phase_reached`가 항상 "execute" → 프론트엔드 phase 표시 단순화
- 인터뷰 위저드는 에이전트 도구 호출(`pi-questionnaire`)로 트리거 → 기존 컴포넌트 재사용
- `LiveActivityBar`, `ActivityTimeline`, thinking 패널 — 변경 없음

### 4.6 StreamingSink — 이미 준비됨

`StreamingSinkRegistry`는 `handle_unified()` 호출 전에 게이트웨이가 등록 (`gateway.rs:456`). `execute_directive` → `run_streaming` 콜백이 `TextChunk`/`ThinkingDelta`를 sink로 전송. **이 경로가 모든 메시지에 활성화됨.**

---

## 5. 대안 검토

### 대안 A: Light streaming for Conversation only (최소 변경)

assess를 classify-only로 경량화. Conversation은 직접 스트리밍 LLM 호출(에이전트 루프 없음). Task는 기존 경로 유지.

| 장점 | 단점 |
|---|---|
| 최소 변경, 낮은 위험 | assess LLM 호출 존속 (사용자 불만 해소 안 됨) |
| Conversation 스트리밍 | 3개 경로 유지 (conversation/task/clarify) → 복잡성 |
| 인터뷰 위저드 보존 | crystallize 외부 호출 존속 |

**기각:** "굳이?" 질문을 해결하지 못함. 복잡성을 줄이지 않음.

### 대안 B: assess 유지 + crystallize/review 내재화 (중간)

assess는 외부 gate로 유지(빠른 routing). crystallize/review를 에이전트 도구(`self_plan`, `self_review`)로 내재화.

| 장점 | 단점 |
|---|---|
| 인터뷰 위저드 보존 | assess LLM 호출 존속 |
| plan/review가 에이전트 루프 안에서 가시적 | 2단계 구조(assess + agent) 유지 |
| 자기 검토 편향 부분 완화 | 복잡한 도구 추가 필요 |

**기각:** 가장 복잡하고, "가장 우아한" 구조가 아님.

### 대안 C (채택): assess 제거, 통합 스트리밍

모든 메시지를 에이전트 루프로. Ouroboros 프로토콜을 시스템 프롬프트 지침으로 내재화.

| 장점 | 단점 |
|---|---|
| 하나의 경로, 분기 없음 | assess 기반 인터뷰 위저드 제거 (→ 에이전트 도구로 대체) |
| 모든 메시지 스트리밍 | Task::Substantial의 외부 crystallize 제거 (→ 에이전트 내재적 planning) |
| LLM 호출 절약 (Task -1~2회) | 행동 변화 (에이전트가 분류를 스스로 수행) |
| Claude/Gemini와 동일 구조 | 외부 review만 유지 (acceptance_criteria 있을 때) |
| 가장 단순하고 우아함 | |

**채택 사유:** "가장 아름답고 우아한 구조"라는 사용자 요구에 부합. 하나의 경로, 하나의 LLM 호출(기본), 모든 가시성 확보.

---

## 6. 위험 및 완화

| 위험 | 확률 | 완화 |
|---|---|---|
| 에이전트가 단순 질문에 불필요한 도구 사용 | 중 | 시스템 프롬프트 강화: "단순 대화는 도구 없이 답변" |
| 모호성 감지 품질 저하 (assess LLM 제거) | 중 | `ask_user` / `pi-questionnaire` 도구로 보완. 에이전트가 질문하면 사용자가 답 |
| 외부 crystallize 제거로 directive 품질 저하 | 낮음 | 에이전트 시스템 프롬프트에 PLAN 단계 강화. thinking 패널로 planning 가시 |
| interview_response 자연어 변환 경로 단절 | 낮음 | chat.rs의 interview_response 처리는 유지. 에이전트가 pi-questionnaire 호출 시 동일 경로 |

---

## 7. 검증 계획

1. **Conversation 스트리밍**: "안녕" 전송 → token chunk가 도착하는지 (타자기 효과). `done`이 1회만(종료 시) 발생하는지.
2. **Task 도구 가시성**: "이 파일 읽어줘" 전송 → tool_start/tool_end chunk 도착.
3. **LLM 호출 수**: Conversation 1회, Trivial 1회 확인 (로그/metrics).
4. **Clarify 보존**: 모호한 작업 전송 → 에이전트가 `ask_user` 또는 `pi-questionnaire` 호출하는지.
5. **외부 review**: acceptance_criteria가 있는 작업 → review phase 이벤트 발생.
6. **기존 기능 회귀**: trajectory 영속화, reasoning 영속화, 토큰 사용량 표시 정상.

---

## 8. 마이그레이션 영향

### 제거되는 코드
- `orchestrator.rs`: assess() 호출, Assessment match 분기, crystallize() 호출 (handle 내)
- `engine.rs`: assess(), crystallize() 메서드 (또는 deprecated)
- `prompts.rs`: ASSESS_SYSTEM_PROMPT, CRYSTALLIZE_SYSTEM_PROMPT
- `HandleResponse::Reply`, `HandleResponse::Clarify` 변종

### 유지되는 코드
- `execute_directive()`, `run_streaming()`, `StreamingSinkRegistry`
- `verify_or_retry()` (외부 review)
- `Directive`, `MsgCtx`, `Exchange` 구조체
- `REVIEW_SYSTEM_PROMPT`, `review()`
- 인터뷰 위저드 프론트엔드 컴포넌트 (pi-questionnaire 경로)
- RFC-015 투명성 인프라 전체 (phase/tool/reasoning/usage)

### 프론트엔드
- `phase_reached` 처리 단순화 ("interview" 케이스 제거 가능)
- 인터뷰 위저드 트리거 경로 변경 (assess → 에이전트 도구)
- 스트리밍/타자기/thinking — 변경 없음

---

## 9. 부록: 버그 발견

FlowTrace 조사 중 발견: `src/api/routes/chat.rs` regular WS `"message"` 분기가 `IncomingMessage`를 빌드하고 `pending`에 넣지만 `incoming_tx.send()`를 호출하지 않음. **본 RFC 구현 중 전제 조건으로 수정** — regular 채팅이 오케스트레이터에 도달하지 못하는 근원 원인 중 하나였음.
