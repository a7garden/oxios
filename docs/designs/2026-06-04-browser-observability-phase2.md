# Browser Observability — Phase 2: oxios Integration

> **상태:** Ready (Phase 0 + Phase 1 완료, Phase 2 미착수)
> **선행:** `oxibrowser-core 0.12.0` ✅, `oxi-sdk 0.27.0` ✅, `oxi-sdk = "0.27.0"`로 oxios Cargo.toml 갱신 ✅
> **범위:** oxios-kernel + oxios-web + oxios-web 프런트
> **연관 문서:** [oxibrowser/docs/designs/2026-06-04-oxibrowser-observability.md](../../oxibrowser/docs/designs/2026-06-04-oxibrowser-observability.md) (Phase 0+1 원안), [rfc-015-chat-transparency.md](../rfc-015-chat-transparency.md) (기존 chat transparency)

---

## 1. 배경 — 여기까지 온 길

Phase 0 (`oxibrowser-core 0.12.0`)와 Phase 1 (`oxi-sdk 0.27.0`)는 완료되어 crates.io에 publish됐다. 이벤트 흐름은 이제 이 상태다:

```
BrowserEvent (oxibrowser-core)
    ↓
ProgressForwarder (oxi-agent, native-browser feature)
    ↓
AgentEvent::ToolExecutionUpdate (기존 infra, oxi-agent/src/agent_loop/tool_exec.rs:441)
    ↓                                    ← 여기가 막혀 있음
KernelEvent (oxios-kernel, 변환 필요)
    ↓
WS chunk tool_progress (oxios-web chat.rs)
    ↓
ActivityCard with <Loader2 /> + progress text (oxios-web 프런트)
```

**이 문서는 마지막 다리 — `AgentEvent::ToolExecutionUpdate` → UI까지 — 를 완성한다.**

## 2. 남은 작업 (4개)

### 2.1 `oxios-kernel` 변환

**`crates/oxios-kernel/src/event_bus.rs`**
- `KernelEvent` enum에 variant 추가:
  ```rust
  /// A tool execution is making progress (real-time, RFC-015+).
  ToolExecutionProgress {
      session_id: String,
      tool_call_id: String,
      tool_name: String,
      /// Short human-readable progress message from the tool.
      progress: String,
  },
  ```
- `kernel_event_to_audit_action`에 매핑 추가 (`AuditAction::Other` + `detail: "tool_progress:..."`)

**`crates/oxios-kernel/src/agent_runtime.rs`** (라인 ~680-750 사이)
- 기존 `ToolExecutionStart` / `ToolExecutionEnd` arm 사이에 `ToolExecutionUpdate` arm 추가:
  ```rust
  AgentEvent::ToolExecutionUpdate {
      tool_call_id,
      tool_name,
      partial_result,
  } => {
      if let Some(ref sid) = transparency_session {
          let _ = kernel_handle_for_cb.infra.publish(
              KernelEvent::ToolExecutionProgress {
                  session_id: sid.clone(),
                  tool_call_id: tool_call_id.clone(),
                  tool_name: tool_name.clone(),
                  progress: partial_result,
              },
          );
      }
  }
  ```

**버전 bump**: `oxios-kernel` 1.0.2 → 1.0.3, `oxios-web` 1.0.2 → 1.0.3

### 2.2 `oxios-web` 백엔드 (Rust)

**`surface/oxios-web/src/routes/events.rs`** — `sanitize_event` 함수
- `KernelEvent::ToolExecutionProgress` arm 추가, SSE 페이로드:
  ```rust
  KernelEvent::ToolExecutionProgress {
      session_id, tool_call_id, tool_name, progress
  } => serde_json::json!({
      "type": "tool_progress",
      "session_id": session_id,
      "tool_call_id": tool_call_id,
      "tool_name": tool_name,
      "progress": progress,
  }),
  ```

**`surface/oxios-web/src/routes/chat.rs`** — `kernel_event_to_ws_chunk` (라인 ~691)
- `event_session_id` 매치에 `ToolExecutionProgress` 추가 (세션 필터용)
- 본문 match에 `tool_progress` 청크 emit 추가:
  ```rust
  KernelEvent::ToolExecutionProgress {
      tool_call_id, tool_name, progress, ..
  } => Some(serde_json::json!({
      "type": "tool_progress",
      "tool_call_id": tool_call_id,
      "tool_name": tool_name,
      "progress": progress,
  })),
  ```
- `rfc015_tests` 모듈에 `tool_progress_emits_tool_progress_chunk` 테스트 추가 (기존 wire-contract 보장 패턴 따름)

### 2.3 `oxios-web` 프런트 (TypeScript/React)

**`web/src/types/index.ts`** — `StreamChunk` union 확장
```typescript
| { type: 'tool_progress'; tool_call_id: string; tool_name: string; progress: string }
```

**`web/src/types/index.ts`** — `ChatActivity.tool_call` variant 확장
- `progress?: string` (optional)
- `isRunning?: boolean` (optional)

**`web/src/stores/chat.ts`** — `chunkToActivity` (라인 ~89)
- 신규 `case 'tool_progress'` 추가 → `progress: chunk.progress, isRunning: true`
- 기존 `case 'tool_start'` → `isRunning: true` 설정
- 기존 `case 'tool_end'` → `isRunning: false` (또는 undefined) 설정

**`web/src/components/chat/activity-card.tsx`** — `ActivityCard`
- `lucide-react`의 `Loader2` import
- 헤더에 `isRunning`일 때 `<Loader2 className="h-3 w-3 animate-spin" />` 표시
- `activity.progress`가 있으면 진행 텍스트 라인 표시

**`web/src/__tests__/stores.test.ts`** — `chunkToActivity` 테스트 추가
- `tool_progress` 청크 → `progress` + `isRunning: true` ChatActivity
- `tool_start` → `isRunning: true`
- `tool_end` → `isRunning: false`

### 2.4 `oxios-web` 프런트 dist 재생성 + GitHub Release

- `cd surface/oxios-web/web && bun run build`
- 새 `web-dist.zip` 생성 → GitHub Release에 v1.0.3으로 attach
- `~/.oxios/web/dist/` 가 자동으로 새 버전 다운로드

---

## 3. oxios 빌드 노이즈 정리 (선결 조건)

Phase 2를 시작하기 전에 **반드시 정리해야 할** pre-existing 이슈:

### 3.1 문제

`oxios` (binary crate) 빌드 실패:
```
error[E0425]: cannot find function `build_marketplace_api_value` in this scope
   --> src/kernel.rs:858:17
    |
858 |                 build_marketplace_api_value(&config),
    |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
```

main 브랜치 (`8f99e3d docs(rfc-017): memory extraction strategy`)도 깨져 있음 — 다른 에러 (`chunking` module 없음).

### 3.2 원인 추정

워킹트리에 **commit되지 않은 다수의 변경**이 있음:
```
AGENTS.md, CONTRIBUTING.md, LICENSE*, NOTICE.md (deletions)
Cargo.toml, README.md, CHANGELOG.md
crates/oxibrowser-*/src/domains/mod.rs, lib.rs, frame.rs, page.rs, session.rs, tab.rs
crates/oxibrowser/tests/smoke.rs
docs/ARCHITECTURE.md, docs/DESIGN.md, docs/designs/v0.3-*, v0.4-*, roadmap-v0.5.md
D docs/COMPARISON_REPORT.md
?? icon.png, logo-readme.png
```

이 변경들은 내가 한 게 아니다. 하지만 내 작업(`oxi-sdk = "0.27.0"`)과 **섞여서** 빌드를 깨고 있다. oxi-sdk 0.27.0에는 내가 추가한 `BrowserEvent` 재내보내기가 있는데, 그게 어떤 모듈에서 사용되어서 추가 에러를 유발하고 있을 수 있다.

### 3.3 권장 정리 순서

1. `git stash` (혹은 `git checkout -- .` 로 워킹트리 복원)
2. 깨끗한 main에서 `cargo build` 시도
3. 깨끗한 main이 빌드되면, oxi-sdk 0.27.0 변경만 다시 적용:
   ```bash
   # oxios/Cargo.toml
   -oxi-sdk = "0.26.2"
   +oxi-sdk = "0.27.0"
   ```
4. 빌드 → 깨끗한 변경 메시지 확인
5. 깨지지 않으면 stash 내용 중 의미 있는 것만 cherry-pick
6. 깨지면 어디서 깨졌는지 보고 수동 분리

**이 노이즈를 정리하지 않고 Phase 2를 시작하면, Phase 2 작업이 노이즈와 섞여서 커밋이 지저분해진다.**

---

## 4. 작업량 추정

| 작업 | LoC | 비고 |
|------|-----|------|
| `oxios-kernel/src/event_bus.rs` | +12 | enum variant + audit mapping |
| `oxios-kernel/src/agent_runtime.rs` | +18 | match arm 추가 |
| `oxios-web/src/routes/events.rs` | +12 | SSE sanitizer arm |
| `oxios-web/src/routes/chat.rs` | +22 | WS converter arm + 테스트 |
| `oxios-web/web/src/types/index.ts` | +4 | StreamChunk union |
| `oxios-web/web/src/stores/chat.ts` | +20 | chunkToActivity cases |
| `oxios-web/web/src/components/chat/activity-card.tsx` | +12 | Loader2 + progress line |
| `oxios-web/web/src/__tests__/stores.test.ts` | +30 | 신규 테스트 |
| **합계 (코드)** | **~130 LoC** | |
| 테스트 | ~80 LoC | |
| **총합** | **~210 LoC** | |

이전 문서(2026-06-04-oxibrowser-observability.md)의 Appendix A는 350 LoC으로 추정했는데, oxios-kernel/oxios-web 변경은 실제로는 더 적다. 차이는 Cargo.toml 변경 등 잡일.

---

## 5. 의존성 순서 (publish)

```
1. oxios-kernel 1.0.3   (KernelEvent::ToolExecutionProgress)
2. oxios 1.0.3          (버전 bump + re-export)
3. oxios-web 1.0.3      (백엔드 라우트)
4. web-dist.zip         (프런트 번들)
```

oxios-kernel → oxios 순서 중요 (oxios가 oxios-kernel을 dep으로 가지니까). 모두 같은 PR에서 bump 가능.

---

## 6. 테스트 전략

### 단위 테스트 (Rust)

| 테스트 | 위치 | 검증 |
|--------|------|------|
| `tool_progress_emits_tool_progress_chunk` | `oxios-web/src/routes/chat.rs` (rfc015_tests 확장) | wire contract |
| `tool_progress_passes_sanitize_event` | `oxios-web/src/routes/events.rs` | SSE 페이로드 형태 |

### 단위 테스트 (TypeScript)

| 테스트 | 위치 | 검증 |
|--------|------|------|
| `chunkToActivity_tool_progress` | `oxios-web/web/src/__tests__/stores.test.ts` | `progress` + `isRunning: true` |
| `chunkToActivity_tool_start_sets_running` | 같은 파일 | `isRunning: true` |
| `chunkToActivity_tool_end_clears_running` | 같은 파일 | `isRunning: false` |

### 수동 smoke test

```bash
# 1. oxios 빌드
cd /Volumes/MERCURY/PROJECTS/oxios
cargo build --release

# 2. oxios 데몬 시작
./target/release/oxios run --foreground

# 3. 브라우저로 http://localhost:8080 접속
# 4. 채팅에서 "https://news.ycombinator.com 으로 가서 헤드라인 5개 요약해줘" 입력
# 5. ActivityCard에 다음이 보이면 성공:
#    - "browse" 툴 카드 + 회전 스피너
#    - "Opening https://news.ycombinator.com…" 진행 텍스트
#    - (JS 실행 후) "Loaded "Hacker News" — 200 · 12 KB · 3 scripts · 245 ms"
#    - 최종 요약
```

---

## 7. 리스크

| 리스크 | 가능성 | 영향 | 완화 |
|--------|--------|------|------|
| `oxios` 빌드 노이즈가 Phase 2 작업과 섞여서 커밋이 더러워짐 | **High** | Medium | §3 정리 후 착수 |
| `ChunkToActivity` 변경이 mobile/exporter 같은 다른 consumer에 영향 | Low | Low | `isRunning`은 optional, `undefined`는 `false`와 동등 — 기존 consumer 안전 |
| `ActivityCard` 변경이 디자인 시스템과 충돌 | Low | Low | `<Loader2>`는 `lucide-react` 기존 import 패턴 따름, 색상은 `text-muted-foreground` |
| GitHub Release에 web-dist.zip 업로드 자동화 안 됨 | Medium | Low | 수동 업로드 — oxios의 `~/.oxios/web/dist/` 자동 다운로드 메커니즘이 fallback 제공 |

---

## 8. 디자인 결정 — 왜 `isRunning`/`progress` 둘 다인가

원안(`2026-06-04-oxibrowser-observability.md`) §10에서 "레거시 호환 신경쓰지 말고 아름다운 구조로"라는 피드백을 받았다. 그래서:

- `isRunning`을 별도 필드로 만들지 않고, 기존 `isError` 옆에 자연스럽게 추가
- `progress`는 optional (없으면 미표시) → UI가 "이 카드는 지금 돌고 있나, 끝났나"를 `isRunning`으로 명확히 분기
- `tool_start` → `isRunning: true`, `tool_end` → `isRunning: false` (또는 undefined)
- `tool_progress` → `isRunning: true` 유지 + `progress` 텍스트 갱신

이렇게 하면 state machine이 명확하다:
```
도구 호출:  none → tool_start (isRunning=true) → ... tool_progress (progress=X) ... → tool_end (isRunning=false)
```

UI는:
- `isRunning` true → 스피너 표시
- `progress` 있으면 진행 텍스트 표시 (없으면 비움)
- `outputSummary` 있으면 결과 표시 (tool_end 이후)

---

## 9. 정리 (Wrap-up)

Phase 0 (`oxibrowser-core 0.12.0`) + Phase 1 (`oxi-sdk 0.27.0`) + publish + commit 완료.

Phase 2는:
- **선결**: §3 빌드 노이즈 정리
- **본체**: §2.1 - §2.4 (oxios 변환)
- **마무리**: §5 publish + §6 테스트 + §7 리스크 관리

이 문서가 다음 세션의 이정표.
