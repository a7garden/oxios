# Oxios 전체 이슈 수정 설계서

> **날짜:** 2026-05-28
> **범위:** 분석 보고서에서 식별된 23개 이슈 전체
> **원칙:** 각 수정은 독립적인 커밋 단위. 빌드/테스트가 각 커밋에서 통과해야 함.

---

## Phase 1: Critical — 즉시 수정 (5건)

### FIX-01: `sqlite-memory` 피처 활성화

**문제:** 루트 `Cargo.toml`이 `oxios-kernel`을 `default-features = false`로 지정하여
`sqlite-memory` 코드가 dead code.

**수정 파일:** `Cargo.toml` (루트)

**설계:**

```toml
# [features] 섹션에 추가
sqlite-memory = ["oxios-kernel/sqlite-memory"]
embedding-gguf = ["oxios-kernel/embedding-gguf"]

# default에 sqlite-memory 추가
default = ["web", "cli", "browser", "sqlite-memory"]
```

동시에 `native-browser` dead code도 정리:

```toml
# oxios-kernel/Cargo.toml [features]에 추가
native-browser = ["dep:oxibrowser-core"]  # 또는 단순 gate
```

만약 `native-browser`가 더 이상 필요 없다면:
- `crates/oxios-kernel/src/lib.rs:277`의 `#[cfg(feature = "native-browser")]` 블록 제거

**검증:** `cargo build -p oxios --features sqlite-memory` 성공.

---

### FIX-02: `ToastProvider` 마운트

**문제:** `ToastProvider`가 컴포넌트 트리에 렌더링되지 않아 모든 토스트가 조용히 무시됨.

**수정 파일:** `channels/oxios-web/web/src/routes/__root.tsx`

**설계:**

```tsx
import { ToastProvider } from '@/components/ui/sonner'
// ...
component: function RootComponent() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <AppLayout />
      </ToastProvider>
    </QueryClientProvider>
  )
}
```

**검증:** marketplace나 skills 페이지에서 토스트가 화면에 나타나는지 확인.

---

### FIX-03: `chat.tsx` API 호출에 인증 헤더 추가

**문제:** `SpaceSessionSidebar`가 raw `fetch()`를 사용하여 `Authorization` 헤더 누락.

**수정 파일:** `channels/oxios-web/web/src/routes/chat.tsx`

**설계:** raw `fetch()` → `api.get()` 교체.

```typescript
import { api } from '@/lib/api-client'

// Before (line ~211):
const { data: spacesData } = useQuery({
  queryKey: ['spaces'],
  queryFn: () =>
    fetch('/api/spaces').then((r) => r.json()) as Promise<{...}>,
})

// After:
const { data: spacesData } = useQuery({
  queryKey: ['spaces'],
  queryFn: () => api.get<{ items: Space[]; total: number }>('/api/spaces'),
})

// Before (line ~221):
const { data: sessionsData, refetch: refetchSessions } = useQuery({
  queryKey: ['sessions', activeSpaceId],
  queryFn: () =>
    fetch('/api/sessions').then((r) => r.json()) as Promise<{...}>,
})

// After:
const { data: sessionsData, refetch: refetchSessions } = useQuery({
  queryKey: ['sessions', activeSpaceId],
  queryFn: () =>
    api.get<{ items: Session[]; total: number }>('/api/sessions'),
})
```

**검증:** 인증이 활성화된 상태에서 채팅 페이지 로드 시 스페이스/세션 목록 정상 표시.

---

### FIX-04: `set_persona_prompt` 구현

**문제:** `OuroborosProtocol::set_persona_prompt()`가 no-op.

**수정 파일:**
- `crates/oxios-ouroboros/src/protocol.rs`
- `crates/oxios-ouroboros/src/lib.rs` (또는 프로토콜 구현체가 있는 파일)

**설계:** 트레이트 디폴트 메서드를 구현체에서 오버라이드.

```rust
// protocol.rs — 트레이트 정의는 그대로 둠 (#[inline] default는 OK)

// 구현체 (OuroborosEngine 또는 struct가 있는 파일):
pub struct OuroborosEngine {
    // ... 기존 필드 ...
    persona_prompt: parking_lot::RwLock<Option<String>>,  // 추가
}

impl OuroborosProtocol for OuroborosEngine {
    fn set_persona_prompt(&self, prompt: Option<String>) {
        *self.persona_prompt.write() = prompt;
    }

    // generate_seed, execute 등에서 persona_prompt를 읽어 시스템 프롬프트에 prepend:
    async fn generate_seed(&self, interview: &InterviewResult) -> Result<Seed> {
        let persona = self.persona_prompt.read().clone();
        let system_prompt = match persona {
            Some(p) => format!("{p}\n\n{}", base_system_prompt),
            None => base_system_prompt.to_string(),
        };
        // ... 기존 로직
    }
}
```

**주의:** `OuroborosProtocol` 트레이트가 `+ Send + Sync`이므로 내부 가변성에 `RwLock` 필요.

**검증:** 페르소나 설정 후 LLM 호출 시 시스템 프롬프트에 페르소나가 포함되는지 로그 확인.

---

### FIX-05: 파일 크기 제한 재활성화

**문제:** 분석 시 제거된 것으로 보였으나, 실제 코드 확인 결과 `handle_workspace_file_put`
(line 122-126)에 **1MB 제한이 이미 활성화**되어 있음.

테스트 파일의 주석(`// if body.len() > MAX_FILE_SIZE { return PayloadTooLarge }`)은
테스트 내 주석일 뿐, 실제 프로덕션 코드는 정상 동작함.

**결론:** **이슈 없음 — False Positive.** 수정 불필요.

**추가 확인:** 다른 PUT 엔드포인트에도 크기 제한이 있는지 검토:
- `workspace.rs:573` — PayloadTooLarge ✅
- `workspace.rs:730` — PayloadTooLarge ✅

---

## Phase 2: High — 가까운 시일 내 수정 (9건)

### FIX-06: Audit Trail 연결 (TracingAuditSink → TrailAuditSink)

**문제:** `agent_runtime.rs:410`에서 `TracingAuditSink`만 사용, 실제 audit trail에 기록 안 됨.

**수정 파일:**
- `crates/oxios-kernel/src/agent_runtime.rs`
- `crates/oxios-kernel/src/access_manager/audit_sink.rs`

**설계:**

```rust
// audit_sink.rs — TrailAuditSink 추가
pub struct TrailAuditSink {
    audit_trail: Arc<AuditTrail>,
}

impl TrailAuditSink {
    pub fn new(trail: Arc<AuditTrail>) -> Self {
        Self { audit_trail: trail }
    }
}

impl AuditSink for TrailAuditSink {
    fn record(&self, event: AuditEvent) {
        let actor = &event.agent;
        let action = match &event {
            AuditEvent::ToolAccess { action, .. } => action.clone(),
            _ => AuditAction::Other { detail: format!("{:?}", event) },
        };
        let resource = format!("{:?}", event);
        self.audit_trail.append(actor, action, &resource);
    }
}
```

```rust
// agent_runtime.rs — build_agent_from_seed 함수
// 파라미터에 audit_trail: Option<Arc<AuditTrail>> 추가

let audit_sink: Arc<dyn AuditSink> = match audit_trail {
    Some(trail) => Arc::new(TrailAuditSink::new(trail)),
    None => Arc::new(TracingAuditSink),
};

let access_gate = Arc::new(AccessGate::new(
    kernel_handle.exec.access_manager().clone(),
    Arc::new(kernel_handle.exec.config().clone()),
    audit_sink,
));
```

호출 체인: `Kernel` → `Orchestrator` → `AgentRuntime::build_agent_from_seed()`에
`kernel.handle().security.audit_trail()` 전달.

**검증:** 에이전트 실행 후 audit trail에 레코드가 추가되었는지 확인.

---

### FIX-07: `attach_audit_trail` Lagged 에러 핸들링

**문제:** `while let Ok(event) = rx.recv().await`가 `Lagged` 시 루프 종료.

**수정 파일:** `crates/oxios-kernel/src/event_bus.rs`

**설계:**

```rust
pub fn attach_audit_trail(&self, audit: Arc<AuditTrail>) {
    let mut rx = self.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let actor = extract_agent_id(&event);
                    let action = kernel_event_to_audit_action(&event);
                    let resource = format!("{:?}", event);
                    audit.append(actor, action, resource);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        skipped = n,
                        "Audit trail subscriber lagged, skipping events"
                    );
                    continue; // 계속 수신
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("Audit trail event bus closed, exiting");
                    break;
                }
            }
        }
    });
}
```

**검증:** 이벤트 폭주 시 로그에 "lagged" 경고가 나타나고 audit trail이 계속 동작.

---

### FIX-08: 기본 스킬 디렉토리 연결

**문제:** `init_default_skills()`가 `defaults_dir`을 무시.

**수정 파일:** `src/kernel.rs`

**설계:**

```rust
pub async fn init_default_skills(&self, share_dir: &std::path::Path) -> Result<()> {
    let defaults_dir = share_dir.join("default-skills");
    self.skill_manager.init().await?;

    if defaults_dir.exists() {
        let mut entries = tokio::fs::read_dir(&defaults_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Err(e) = self.skill_manager.install_from_path(&path).await {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to install bundled skill"
                    );
                }
            }
        }
        tracing::info!("Default skills installed from {}", defaults_dir.display());
    } else {
        tracing::debug!("No default skills directory found");
    }

    Ok(())
}
```

**주의:** `skill_manager.install_from_path()` 메서드가 존재하는지 확인 필요.
없으면 `SkillManager`에 추가.

**검증:** `share/default-skills/`에 테스트 스킬 파일 넣고 실행 후 로드 확인.

---

### FIX-09: `dirs` 의존성 통일

**문제:** root/web은 `dirs = "5"`, kernel은 `dirs = "6"`.

**수정 파일:** `Cargo.toml` (루트), `crates/oxios-kernel/Cargo.toml`

**설계:**

```toml
# 루트 [workspace.dependencies]에 추가
dirs = "6"

# 루트 [dependencies] 변경
dirs = { workspace = true }

# oxios-kernel/Cargo.toml 변경
dirs = { workspace = true }
```

v5 → v6 마이그레이션 체크:
- `dirs::home_dir()`, `dirs::config_dir()` 등 API는 동일 (breaking change 없음)
- `dirs v6`은 내부 의존성 업데이트만

**검증:** `cargo build --workspace` 성공, `cargo tree -d | grep dirs`에 단일 버전만 표시.

---

### FIX-10: `native-browser` 피처 정리

**문제:** 정의되지 않은 피처로 인한 dead code.

**수정 파일:** `crates/oxios-kernel/src/lib.rs`, `crates/oxios-kernel/Cargo.toml`

**설계 — 옵션 A (피처 제거):**
```rust
// lib.rs에서 #[cfg(feature = "native-browser")] 블록을 항상 활성화하거나 제거
// oxibrowser-core는 이미 필수 의존성이므로 게이트가 불필요
```

**설계 — 옵션 B (피처 정의):**
```toml
# oxios-kernel/Cargo.toml
native-browser = ["dep:oxibrowser-core"]
```

oxibrowser-core가 이미 일반 의존성으로 있으므로 **옵션 A**가 맞음.
`#[cfg(feature = "native-browser")]` 블록에서 cfg 게이트 제거.

**검증:** `cargo build -p oxios-kernel` 성공.

---

### FIX-11: 뮤텍스 포이즈닝 방지

**문제:** `.lock().unwrap()`이 다른 스레드 패닉 시 전파.

**수정 파일:**
- `channels/oxios-cli/src/channel.rs`
- `crates/oxios-kernel/src/state_store.rs`

**설계:** `parking_lot::Mutex`는 포이즈닝이 없음! 표준 `std::sync::Mutex`만 포이즈닝됨.

확인 결과: 커널은 `parking_lot::Mutex` 사용 (workspace dep에 `parking_lot = "0.12"`).
CLI는 `std::sync::Mutex` 사용 가능성 있음.

```rust
// channel.rs — std::sync::Mutex인 경우:
let session = self.session.lock().unwrap_or_else(|e| {
    tracing::error!("Session mutex poisoned: {e}");
    e.into_inner() // 포이즈닝되어도 데이터 복구
});

// state_store.rs — parking_lot::Mutex인 경우:
// parking_lot은 포이즈닝이 없으므로 .unwrap()이 절대 패닉하지 않음
// 그대로 두거나 가독성을 위해 주석 추가:
let mut guard = self.last_prune.lock(); // parking_lot: never poisons
```

**검증:** 각 파일의 `use` 문에서 `std::sync` vs `parking_lot` 확인 후 결정.

---

### FIX-12: 오케스트레이터 `best_eval.unwrap()` 제거

**문제:** 3곳의 `.unwrap()`이 런타임 패닉 가능.

**수정 파일:** `crates/oxios-kernel/src/orchestrator.rs`

**설계:**

```rust
// line 789 — 첫 번째
if evaluation.score >= threshold || iteration == max_iterations {
    // ...
    let best = best_eval.ok_or_else(|| anyhow::anyhow!(
        "No evaluation produced after {iteration} iterations (seed={})",
        current_seed.id
    ))?;
    return Ok((best_result, best, best_seed));
}

// line 794 — 두 번째
if max_iterations == 0 {
    let best = best_eval.ok_or_else(|| anyhow::anyhow!(
        "No evaluation produced for seed={}",
        current_seed.id
    ))?;
    return Ok((best_result, best, best_seed));
}

// line 828 — 세 번째 (evolve가 None을 반환한 후)
let best = best_eval.ok_or_else(|| anyhow::anyhow!(
    "Evolve returned None but no evaluation exists (seed={})",
    current_seed.id
))?;
return Ok((best_result, best, best_seed));
```

**검증:** `cargo test -p oxios-kernel -- orchestrator` 통과.

---

### FIX-13: 툴 종료 시그널 핸들링

**문제:** 모든 툴이 `_signal`/`_shutdown` 파라미터를 무시.

**수정 파일:** `crates/oxios-kernel/src/tools/exec_tool.rs` (우선순위 가장 높음)

**설계:** 장시간 실행되는 셸 명령에만 우선 적용:

```rust
async fn execute(
    &self,
    _tool_call_id: &str,
    params: Value,
    shutdown: Option<oneshot::Receiver<()>>,
    _ctx: &ToolContext,
) -> Result<AgentToolResult, String> {
    // ... 파라미터 파싱 ...

    match mode {
        "shell" => {
            // 기존 shell_exec 대신 shutdown-aware 버전 사용
            match self.shell_exec_with_shutdown(command, timeout_ms, shutdown).await {
                Ok(result) => { /* 동일 */ }
                Err(e) => Ok(AgentToolResult::error(format!("exec (shell): {e}"))),
            }
        }
        "structured" => { /* 동일하게 shutdown 전달 */ }
    }
}

async fn shell_exec_with_shutdown(
    &self,
    command: &str,
    timeout_ms: u64,
    mut shutdown: Option<oneshot::Receiver<()>>,
) -> Result<ExecResult, anyhow::Error> {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    tokio::select! {
        status = child.wait() => {
            // 정상 완료
            let output = status?;
            // ... 기존 로직
        }
        _ = tokio::time::sleep(Duration::from_millis(timeout_ms)) => {
            child.kill().await?;
            Ok(ExecResult { exit_code: -1, output: "Timeout".into(), timed_out: true })
        }
        _ = async {
            if let Some(ref mut rx) = shutdown {
                let _ = rx.await;
            } else {
                std::future::pending::<()>().await;
            }
        } => {
            child.kill().await?;
            Ok(ExecResult { exit_code: -1, output: "Cancelled by shutdown".into(), timed_out: false })
        }
    }
}
```

**검증:** 장시간 명령(`sleep 60`) 실행 중 에이전트 종료 시 프로세스가 정리되는지 확인.

---

### FIX-14: SSE 클라이언트 자동 재연결

**문제:** `SseClient`에 재연결 로직 없음.

**수정 파일:** `channels/oxios-web/web/src/lib/sse-client.ts`

**설계:**

```typescript
export class SseClient {
  private controller: AbortController | null = null
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private reconnectAttempts = 0
  private static MAX_RECONNECT_ATTEMPTS = 10
  private static BASE_DELAY_MS = 1000

  async connect(
    path: string,
    onEvent: (event: string, data: unknown) => void,
    onError?: (error: Error) => void,
  ) {
    this.disconnect()
    this.reconnectAttempts = 0
    await this.doConnect(path, onEvent, onError)
  }

  private async doConnect(
    path: string,
    onEvent: (event: string, data: unknown) => void,
    onError?: (error: Error) => void,
  ) {
    this.controller = new AbortController()
    const token = localStorage.getItem('oxios-api-key')
    const protocol = window.location.protocol
    const url = `${protocol}//${window.location.host}${path}`

    try {
      const response = await fetch(url, {
        headers: { Authorization: `Bearer ${token}` },
        signal: this.controller.signal,
      })

      const reader = response.body?.getReader()
      if (!reader) return

      const decoder = new TextDecoder()
      let buffer = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        // ... 기존 파싱 로직 동일 ...
      }

      // 스트림 정상 종료 → 재연결 시도
      this.scheduleReconnect(path, onEvent, onError)
    } catch (err) {
      if ((err as Error).name !== 'AbortError') {
        onError?.(err as Error)
        this.scheduleReconnect(path, onEvent, onError)
      }
    }
  }

  private scheduleReconnect(
    path: string,
    onEvent: (event: string, data: unknown) => void,
    onError?: (error: Error) => void,
  ) {
    if (this.reconnectAttempts >= SseClient.MAX_RECONNECT_ATTEMPTS) {
      console.warn('SSE max reconnect attempts reached')
      return
    }
    const delay = SseClient.BASE_DELAY_MS * Math.pow(2, this.reconnectAttempts)
    this.reconnectAttempts++
    console.log(`SSE reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`)
    this.reconnectTimer = setTimeout(() => {
      this.doConnect(path, onEvent, onError)
    }, delay)
  }

  disconnect() {
    this.controller?.abort()
    this.controller = null
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    this.reconnectAttempts = 0
  }
}
```

**검증:** 네트워크 끊김 후 자동 재연결되는지 확인 (DevTools → Offline 시뮬레이션).

---

## Phase 3: Medium — 개선 권장 (9건)

### FIX-15: StateStore 임시 파일명에 UUID 추가

**수정 파일:** `crates/oxios-kernel/src/state_store.rs`

```rust
// line ~253, ~311
let temp_path = dir.join(format!(
    "{}.{}.{}.tmp",
    name,
    std::process::id(),
    uuid::Uuid::new_v4()
));
```

---

### FIX-16: 메모리 서브시스템 에러 로깅

**수정 파일:** `memory/sqlite_store.rs`, `memory/database.rs`, `memory/dream.rs`

**패턴:** 모든 `.filter_map(|r| r.ok())`에 경고 로그 추가:

```rust
// Before:
rows.filter_map(|row| row.ok())

// After:
rows.filter_map(|row| match row {
    Ok(r) => Some(r),
    Err(e) => {
        tracing::warn!(error = %e, "Failed to deserialize memory row, skipping");
        None
    }
})
```

영향 범위: 약 10개 파일, ~15개 호출점. 각각 개별 커밋.

---

### FIX-17: 프론트엔드 Mutation 글로벌 에러 핸들링

**수정 파일:** `channels/oxios-web/web/src/main.tsx`

```typescript
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30000,
      retry: 1,
    },
    mutations: {
      onError: (error) => {
        console.error('Mutation failed:', error)
        // ToastProvider가 FIX-02에서 마운트되었으므로 useToast 사용 가능
        // 단, React 컴포넌트 외부이므로 직접 DOM 조작 또는 이벤트 사용
        const event = new CustomEvent('oxios:mutation-error', {
          detail: { message: error.message || 'Unknown error' },
        })
        window.dispatchEvent(event)
      },
    },
  },
})
```

또는 `ToastProvider`에 글로벌 이벤트 리스너 추가:

```typescript
// sonner.tsx — ToastProvider 내부
React.useEffect(() => {
  const handler = (e: CustomEvent) => {
    addToast(e.detail.message, 'destructive')
  }
  window.addEventListener('oxios:mutation-error', handler as EventListener)
  return () => window.removeEventListener('oxios:mutation-error', handler as EventListener)
}, [addToast])
```

---

### FIX-18: `useChatStream` Dead Code 제거

**수정 파일:** `channels/oxios-web/web/src/hooks/use-chat-stream.ts` — **파일 삭제**

```bash
rm channels/oxios-web/web/src/hooks/use-chat-stream.ts
```

**검증:** `bun run build` 성공 (import 없으므로 영향 없음).

---

### FIX-19: 사용하지 않는 npm 의존성 제거

**수정 파일:** `channels/oxios-web/web/package.json`

제거 대상:
- `react-hook-form` — import 없음
- `@hookform/resolvers` — import 없음
- `zod` — import 없음
- `shadcn` — CLI 도구, `devDependencies`로 이동하거나 제거 (컴포넌트 이미 생성됨)
- `eslint`, `@eslint/js`, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`, `typescript-eslint` — Biome 사용 중

```bash
cd channels/oxios-web/web
bun remove react-hook-form @hookform/resolvers zod shadcn
bun remove -D eslint @eslint/js eslint-plugin-react-hooks eslint-plugin-react-refresh typescript-eslint
```

**검증:** `bun run build` 성공.

---

### FIX-20: 오케스트레이터 `#[allow(clippy::await_holding_lock)]` 축소

**수정 파일:** `crates/oxios-kernel/src/orchestrator.rs`

```rust
// Before:
#[allow(clippy::await_holding_lock)]
pub async fn handle_message(...) -> Result<...> {

// After — 함수 레벨 제거, 필요한 블록에만:
pub async fn handle_message(...) -> Result<...> {
    // ... 모든 락은 명시적 drop으로 이미 안전함
    // clippy가 정말 불평한다면 해당 블록에만:
    #[allow(clippy::await_holding_lock)]
    let sm_opt = {
        let sm_guard = self.space_manager.read();
        sm_guard.as_ref().cloned()
    };
    // 단, 실제로는 .await 전에 drop되므로 allow 불필요할 가능성 높음
```

`space/manager.rs:177`도 동일하게 축소.

**검증:** `cargo clippy -p oxios-kernel` 경고 없음.

---

### FIX-21: WebSocket 토큰 전달 방식 개선

**수정 파일:**
- `channels/oxios-web/web/src/stores/chat.ts`
- `channels/oxios-web/src/routes/chat_routes.rs` (또는 WS 핸들러)

**설계 — 단기 티켓 방식:**

```typescript
// chat.ts — 연결 전 일회성 티켓 요청
async connect() {
  // 1. 티켓 발급
  const ticket = await api.post<{ ticket: string }>('/api/chat/ticket')
  // 2. WS 연결 (티켓은 30초 유효, 1회용)
  const url = `${protocol}//${host}/api/chat/stream?ticket=${ticket.ticket}`
  wsInstance = new WebSocket(url)
}
```

```rust
// chat_routes.rs — 티켓 발급 엔드포인트
async fn handle_ticket(state: State<Arc<AppState>>) -> Json<serde_json::Value> {
    let ticket = state.kernel.generate_ws_ticket().await?; // 30초 TTL, 1회용
    Json(json!({ "ticket": ticket }))
}

// WS 업그레이드 시 티켓 검증
async fn handle_ws_upgrade(ws: WebSocketUpgrade, Query(params): Query<TicketParams>) -> ... {
    let valid = state.kernel.validate_ws_ticket(&params.ticket).await?;
    if !valid { return Err(()) }
    // ... 기존 WS 업그레이드
}
```

**검증:** 서버 로그에 토큰이 평문으로 남지 않는지 확인.

---

### FIX-22: 스토어 persist 방식 통일

**수정 파일:** `stores/knowledge.ts`, `stores/auth.ts`, `stores/sidebar.ts`, `stores/theme.ts`

**설계:** 수동 localStorage → Zustand `persist` 미들웨어로 통일.

```typescript
// knowledge.ts — Before:
const savedWidth = Number(localStorage.getItem('oxios-knowledge-sidebar-width')) || 280
const savedSidebarOpen = localStorage.getItem('oxios-knowledge-sidebar-open') !== 'false'

// knowledge.ts — After:
export const useKnowledgeStore = create<KnowledgeState>()(
  persist(
    (set, get) => ({
      sidebarWidth: 280,
      sidebarOpen: true,
      // ... 나머지 상태
    }),
    {
      name: 'oxios-knowledge',
      partialize: (state) => ({
        sidebarWidth: state.sidebarWidth,
        sidebarOpen: state.sidebarOpen,
      }),
    },
  ),
)
```

SSR 안전성을 위해 `localStorage` 접근이 `persist` 미들웨어 내부로 캡슐화됨.
(`persist` 미들웨어는 브라우저에서만 localStorage 사용)

**검증:** localStorage에 `oxios-knowledge` 키로 JSON 저장 확인.

---

### FIX-23: `#[ignore]` 테스트 활성화

**수정 파일:** `crates/oxios-kernel/src/space/detection.rs`

**설계:** 주석에 명시된 이슈 해결:

```rust
// line 313 — regex pattern in full context
#[test]
fn test_regex_pattern_detection() {
    // 기존에 실패하던 이유: regex 패턴이 올바른 컨텍스트에서 매칭되지 않음
    // 수정: 테스트 데이터에 올바른 컨텍스트 제공
    let result = detect_intent("파일에서 email 패턴을 찾아줘");
    assert!(matches!(result.intent, Intent::Search));
}

// line 321 — keyword matching
#[test]
fn test_keyword_matching() {
    let result = detect_intent("오늘 할 일 목록 보여줘");
    assert!(matches!(result.intent, Intent::List));
}
```

`#[ignore]` 제거 후 테스트가 실제 코드와 일치하도록 수정.

**검증:** `cargo test -p oxios-kernel -- test_regex_pattern_detection test_keyword_matching` 통과.

---

## Phase 4: 의존성 정리 (작업 순서 무관)

### DEP-01: 워크스페이스 의존성 통일

**수정 파일:** `Cargo.toml` (루트 `workspace.dependencies`)

추가할 항목:

```toml
[workspace.dependencies]
# 기존 항목 아래에 추가
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
zip = "2"
dirs = "6"
inquire = "0.9"
console = "0.15"
libc = "0.2"
clap = { version = "4", features = ["derive"] }
tempfile = "3"
once_cell = "1.19"

# 버전 통일이 필요한 크레이트
serde_yaml = "0.9"
```

각 하위 크레이트의 개별 버전 명시를 `{ workspace = true }`로 교체.

---

### DEP-02: 버전 정렬

| 크레이트 | 현재 | 정렬 |
|---------|------|------|
| `oxios-mcp` | 0.1.0 | 0.4.0 |
| `oxios-bench` | 0.1.2 | 0.4.0 |

또는 별도 버전 관리가 의도적이면 주석 추가.

---

### DEP-03: Stray `hello_world/` 정리

```bash
rm -rf hello_world/
```

---

### DEP-04: 미사용 import 제거

- `crates/oxios-kernel/src/agent_runtime.rs:36` — `register_tools_from_cspace`
- `crates/oxios-kernel/src/agent_runtime.rs:38` — `crate::config::ExecConfig`
- `crates/oxios-kernel/src/orchestrator.rs:735` — unused variable `session_id`

---

### DEP-05: ESLint 패키지 제거 (프론트엔드)

FIX-19에서 이미 다룸.

---

## 구현 순서 (권장 커밋 순서)

```
1. DEP-03  — hello_world/ 정리                    (5분)
2. DEP-04  — 미사용 import 제거                    (10분)
3. FIX-09  — dirs 의존성 통일                     (15분)
4. FIX-10  — native-browser dead code 정리         (10분)
5. FIX-01  — sqlite-memory 피처 활성화             (10분)
6. DEP-01  — 워크스페이스 의존성 통일              (30분)
7. DEP-02  — 크레이트 버전 정렬                    (5분)
8. FIX-15  — StateStore 임시 파일명 UUID           (10분)
9. FIX-12  — 오케스트레이터 unwrap 제거            (15분)
10. FIX-11 — 뮤텍스 포이즈닝 방지                  (15분)
11. FIX-07 — EventBus Lagged 에러 핸들링           (10분)
12. FIX-20 — await_holding_lock allow 축소         (10분)
13. FIX-16 — 메모리 에러 로깅                      (30분)
14. FIX-13 — 툴 종료 시그널 핸들링                 (45분)
15. FIX-06 — Audit Trail 연결                      (30분)
16. FIX-04 — set_persona_prompt 구현               (30분)
17. FIX-08 — 기본 스킬 디렉토리 연결               (20분)
18. FIX-02 — ToastProvider 마운트                   (5분)
19. FIX-03 — chat.tsx 인증 헤더                    (10분)
20. FIX-05 — 파일 크기 제한 (false positive, 스킵)
21. FIX-17 — Mutation 글로벌 에러 핸들링            (20분)
22. FIX-18 — useChatStream 삭제                    (5분)
23. FIX-19 — 미사용 npm 의존성 제거                (10분)
24. FIX-14 — SSE 자동 재연결                       (30분)
25. FIX-21 — WebSocket 토큰 개선                   (45분)
26. FIX-22 — 스토어 persist 통일                   (30분)
27. FIX-23 — ignore 테스트 활성화                  (15분)
```

**총 예상 시간:** ~7시간

**그룹핑 제안:**

| PR | 포함 | 설명 |
|----|------|------|
| PR 1 | DEP-03, DEP-04, FIX-09, FIX-10, FIX-01, DEP-01, DEP-02 | 의존성/빌드 정리 |
| PR 2 | FIX-15, FIX-12, FIX-11, FIX-07, FIX-20 | Rust 안전성/정확성 |
| PR 3 | FIX-16, FIX-13, FIX-06, FIX-04, FIX-08 | 커널 기능 완성 |
| PR 4 | FIX-02, FIX-03, FIX-17, FIX-18, FIX-19, FIX-14, FIX-22 | 프론트엔드 수정 |
| PR 5 | FIX-21, FIX-23 | 보안/테스트 |

---

## 리스크 및 주의사항

1. **FIX-09 (dirs v6):** `dirs` v5→v6은 API 호환되지만, CI에서 macOS + Linux 모두 테스트 필수
2. **FIX-13 (shutdown signal):** `tokio::select!` 분기가 기존 타임아웃 로직과 상호작용 — 통합 테스트 필수
3. **FIX-21 (WS 티켓):** 백엔드에 새 엔드포인트 필요 — 프론트엔드만으로는 불가
4. **FIX-06 (Audit Trail):** `AuditTrail` → `AuditSink` 연결 시 `Send + Sync + 'static` 제약 확인
5. **FIX-04 (persona):** 모든 LLM 호출 경로에 persona prepend가 누락되지 않았는지 전수 조사 필요
