# LobeHub Chat Port — Remaining Work

> **Last updated:** 2026-07-21 (20 commits on `main` + 1 commit on `oxi`)
> **Companion to:**
> - `docs/designs/2026-07-21-lobehub-chat-port-design.md` (frontend, 6 phases — shipped)
> - `docs/designs/2026-07-21-lobehub-backend-streaming-design.md` (backend, A/B/E/D — shipped)
> - `oxi/docs/designs/2026-07-21-expose-streaming-lifecycle-events.md` (oxi request — filed)

---

## ✅ 완료된 작업

### Frontend (Phases 1-6 + cleanup)
- Phase 1: StreamProcessor + adapter (`4ae63dd53`)
- Phase 2: AssistantMessage pipeline + role dispatcher (`487fc53b4`)
- Phase 3: Tool render 4-tier registry (`cf8000492`)
- Phase 4: MessageActionBar + ErrorCard (`4e674a115`)
- Phase 5: Slash commands 3→10 (`cfe332520`)
- Phase 6: Thinking plugin + rehype-sanitize (`a5aca03cd`)
- Quick-ask StreamProcessor + done/error fix (`8d84eef6b`)
- Reasoning subtype adapter 처리 (`89b8e69ee`)

### Backend (Phases A/B/E/D)
- Phase A: message_id per WS chunk (`bd19e985d`)
- Phase B: reasoning.start/end lifecycle markers (`5d72da29c`)
- Phase E: grounding chunk — JSON-first + favicon (`89b822745`, `8e6a2090a`)
- Phase D: HumanIntervention metadata + ExecTool HitL approval (`e84bdbbaa`, `fec94d588`)

### 추가 구현
- 14개 kernel tool render (knowledge, send_email, a2a, calendar, ActionTool generic) (`fb59cecf3`)
- Markdown plugins: artifact cards + link previews (`f53a59400`)

---

## 🔴 oxi 구현 완료 후 해야 할 일 (oxi PR merge 대기)

> oxi 측 요청서: `oxi/docs/designs/2026-07-21-expose-streaming-lifecycle-events.md`
> 필요한 변경: `AgentEvent::ToolCallDelta` (P0), `AgentEvent::ThinkingEnd` (P1)

### A. oxi 버전 업데이트

```bash
# 1. oxi의 새 버전이 crates.io에 publish된 후:
cd /Volumes/MERCURY/PROJECTS/oxios
# Cargo.toml (workspace)에서 oxi-sdk 버전 업
# 예: oxi-sdk = "0.57.0" (또는 해당 버전)
cargo update -p oxi-sdk
cargo check --workspace
```

### B. `AgentEvent::ThinkingEnd` 연동 (oxi에서 구현된 경우)

**파일:** `crates/oxios-kernel/src/agent_runtime.rs`

**현재 상태:** `AgentEvent::Thinking` → `StreamDelta::Thinking` (start)는 처리 중. end는 없음.

**변경:** oxi에서 `AgentEvent::ThinkingEnd`가 추가되면:

```rust
// agent_runtime.rs의 AgentEvent match 블록에 추가:
AgentEvent::ThinkingEnd => {
    if let Some(ref sid) = transparency_session
        && let Some(tx) = streaming_sinks_for_cb.lookup(sid)
    {
        let _ = tx.try_send(StreamDelta::ThinkingEnd);  // ← 새 variant
    }
}
```

**gateway.rs (collector) 변경:**

```rust
// StreamDelta enum에 새 variant 추가:
pub enum StreamDelta {
    Text(String),
    Model(String),
    Thinking,
    ThinkingDelta(String),
    ThinkingEnd,  // ← 새 variant
}

// collector의 match 블록에 추가:
StreamDelta::ThinkingEnd => {
    was_reasoning = false;
    let mut end_msg = OutgoingMessage::with_id(/* ... */);
    end_msg.metadata.insert("stream_kind".into(), "reasoning.end".into());
    // ... 기존 reasoning.end 방출 로직과 동일
}
```

**chat.rs 변경:** 불필요 — 이미 `reasoning.end` stream_kind을 처리하는 branch가 있음.

**검증 항목:**
- [ ] oxi에서 `ThinkingEnd` 이벤트가 발생하는지 확인
- [ ] gateway collector가 `reasoning.end` chunk를 방출하는지 확인
- [ ] 프론트엔드 Thinking 블록이 정확한 타이밍에 접히는지 확인
- [ ] interleaved reasoning 모델(Claude 4, o3)에서 여러 reasoning span이 각각 start/end를 받는지 확인

### C. `AgentEvent::ToolCallDelta` 연동 (oxi에서 구현된 경우) — **핵심**

**파일:** `crates/oxios-kernel/src/agent_runtime.rs`

**현재 상태:** `AgentEvent::ToolExecutionStart { args: Value }`만 처리. args가 이미 파싱된 상태로 옴.

**변경:** oxi에서 `AgentEvent::ToolCallDelta { tool_call_id, args_delta }`가 추가되면:

```rust
// 1. agent_runtime.rs의 AgentEvent match 블록에 추가:
AgentEvent::ToolCallDelta { tool_call_id, args_delta } => {
    if let Some(ref sid) = transparency_session {
        let _ = kernel_handle_for_cb.infra.publish(
            KernelEvent::ToolArgsDelta {
                session_id: sid.clone(),
                tool_call_id: tool_call_id.clone(),
                args_delta: args_delta.clone(),
            },
        );
    }
}
```

**파일:** `crates/oxios-kernel/src/event_bus.rs`

```rust
// KernelEvent enum에 새 variant 추가:
pub enum KernelEvent {
    // ... 기존 variants ...
    /// Phase C: partial tool-call args from the LLM stream.
    ToolArgsDelta {
        session_id: String,
        tool_call_id: String,
        args_delta: String,
    },
}
```

**파일:** `src/api/routes/chat.rs`

```rust
// kernel_event_to_ws_chunk에 새 match arm 추가:
KernelEvent::ToolArgsDelta {
    tool_call_id, args_delta, ..
} => Some(serde_json::json!({
    "type": "tool_call_delta",
    "tool_call_id": tool_call_id,
    "args_delta": args_delta,
})),
```

**파일:** `web/src/types/index.ts`

```typescript
// StreamChunk type union에 추가:
| 'tool_call_delta'

// StreamChunk 인터페이스에 필드 추가 (이미 tool_call_id, args는 있음):
// args_delta?: string  ← 새 필드
```

**파일:** `web/src/stores/chat.ts`

```typescript
// KNOWN_CHUNK_TYPES에 추가:
'tool_call_delta',
```

**파일:** `web/src/lib/stream/ChatEvent.ts`

```typescript
// ChatEvent union에 추가:
| { kind: 'tool.args_delta'; messageId: string; toolCallId: string; argsDelta: string }
```

**파일:** `web/src/lib/stream/adapter.ts`

```typescript
// adaptChunk에 새 case 추가:
case 'tool_call_delta':
  return [{
    kind: 'tool.args_delta',
    messageId: mid,
    toolCallId: raw.tool_call_id ?? '',
    argsDelta: raw.args_delta ?? raw.content ?? '',
  }]
```

**파일:** `web/src/lib/stream/StreamProcessor.ts`

```typescript
// handleEvent에 새 case 추가:
case 'tool.args_delta': {
  const cur = this.tools.get(ev.toolCallId)
  if (!cur) {
    // 아직 ToolExecutionStart가 안 온 상태 — placeholder 생성
    this.tools.set(ev.toolCallId, {
      id: ev.toolCallId,
      identifier: 'kernel',
      apiName: '(constructing...)',
      arguments: ev.argsDelta,
      status: 'loading',
      startedAt: Date.now(),
    })
  } else {
    // 기존 tool에 args delta 누적
    this.tools.set(ev.toolCallId, {
      ...cur,
      arguments: (cur.arguments as string ?? '') + ev.argsDelta,
    })
  }
  return { patch: { toolCalls: this.toolsList() } }
}
```

**파일:** `web/src/components/chat/messages/components/ToolCallList.tsx`

```typescript
// tool.apiName이 '(constructing...)'일 때 스트리밍 표시 (깜빡이는 커서 등)
// tool.status가 'loading'이고 arguments가 string이면 partial JSON 표시
```

**검증 항목:**
- [ ] oxi에서 `ToolCallDelta` 이벤트가 발생하는지 확인 (mock provider로 테스트)
- [ ] WS chunk `{ type: "tool_call_delta", tool_call_id, args_delta }`가 브라우저에 도달하는지 확인
- [ ] 프론트엔드에서 tool 인자가 실시간으로 축적되어 표시되는지 확인
- [ ] `ToolExecutionStart`가 도달하면 args가 파싱된 값으로 교체되는지 확인
- [ ] stores.test.ts에 tool_call_delta 통합 테스트 추가

### D. oxi 버전 bump 후 전체 검증

```bash
cargo check --workspace
cargo test --workspace
cd web && bun run build && bun run test
```

---

## 🟡 oxi 무관 — 독립적으로 진행 가능한 남은 작업

### 1. Composer ActionBar config-map + @-mention 확장

**현재:** slash commands 10개 확장됨. ActionBar는 하드코딩. @-mention은 knowledge/memory/mounts만.

**해야 할 일:**
- `chat-input-action-bar.tsx`를 config 배열 + actionMap 패턴으로 재구성
- Tiptap suggestion plugin에 skills, topics 카테고리 추가
- `/api/skills` / `/api/sessions` API와 연동

**예상 노력:** 2-3일

### 2. Mobile responsive 감사

**현재:** Tailwind responsive 유틸리티 사용 중이지만 실제 모바일 뷰포트 테스트 안 함.

**알려진 문제:** `UserMessage` textarea의 `min-w-[300px]`가 320px 미만 화면에서 오버플로우.

**해야 할 일:**
- Chrome DevTools 모바일 뷰포트(375px, 320px)에서 전체 페이지 감사
- `min-w-[300px]` → `min-w-0 sm:min-w-[300px]` 등 반응형 조정
- 채팅 입력 영역의 터치 타겟 크기(44px 이상) 확인

**예상 노력:** 반나절

### 3. `message_id`를 KernelEvent variant에 직접 추가

**현재:** WS handler에서 per-connection `active_message_id` 추적 (Phase A). 동시 스트림 시 last-seen wins.

**해야 할 일:**
- `KernelEvent::ToolExecutionStarted/Finished/Progress`, `TokenUsageUpdate`, `ReasoningFragment`에 `message_id: Option<String>` 필드 추가
- `agent_runtime.rs`에서 발행 시점에 message_id 설정
- `kernel_event_to_ws_chunk`에서 필드 그대로 전달
- WS handler의 `active_message_id` 추적 제거 (또는 fallback으로만 유지)

**예상 노력:** 1일 (20개 construction site + 3개 match consumer 업데이트)

### 4. E2E chunk sequence 테스트

**현재:** 단위 테스트만 있음 (kernel 143개, chat 11개, frontend 42개).

**해야 할 일:**
- mock oxi-sdk agent로 전체 스트림 시뮬레이션
- chunk 시퀀스 검증: `model → reasoning.start → reasoning.delta* → reasoning.end → token* → tool_start → tool_end → grounding? → done`
- 각 chunk의 `message_id` 일관성 검증

**예상 노력:** 1일 (test harness 구축)

### 5. Quick-ask 레이아웃 개선

**현재:** quick-ask는 `MessageView`로 새 파이프라인을 사용하지만 다이얼로그 레이아웃은 구형.

**해야 할 일:**
- QuickAsk 다이얼로그 헤더/스크롤/푸터 CSS 개선
- 모델 선택 UI를 다이얼로그에 통합 (기존 QuickAsk의 별도 모델 선택기와 통합)

**예상 노력:** 반나절

---

## ⚫ 명시적 범위 외 (사용자 결정, 2026-07-21)

- **Message branching** (`parentId` 체인) — 별도 설계 필요
- **Follow-up chips** (AI 제안 질문) — 백엔드 LLM 호출 필요
- **멀티모달 content parts** (이미지/오디오/비디오 스트리밍) — oxi-sdk + Oxios 모두 변경 필요
- **12개 message role types** (supervisor, agentCouncil 등) — 아키텍처 방향성 상이

---

## 우선순위 권고

oxi PR이 merge된 직후:
1. **oxi 버전 bump + ThinkingEnd 연동** (B절) — 즉시, 저위험
2. **ToolCallDelta 연동** (C절) — 핵심 기능, 중간 노력
3. **E2E 테스트 작성** (독립 #4) — 회귀 방지

oxi PR과 무관하게 언제든:
4. **Composer 개선** (독립 #1) — 사용자 체감 효과 높음
5. **Mobile 감사** (독립 #2) — 빠른 승리
6. **message_id KernelEvent 직접 추가** (독립 #3) — 동시 스트림 필요 시점에
