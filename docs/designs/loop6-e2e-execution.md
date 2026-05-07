# Loop 6: End-to-End Execution + Production Readiness

> **버전:** v0.3.0-alpha → v0.3.0
> **작성일:** 2026-05-07
> **목표:** 에이전트가 실제로 작동하는 Agent OS로 전환. 지금은 "모든 부품이 있지만 시계가 안 돈다".

---

## 0. 현재 상태 진단

### 무엇이 되는가

```
HTTP POST /api/chat
  → WebChannel.send_and_wait()          ✅ 작동
  → Gateway.route()                     ✅ 작동
  → Orchestrator.handle_message()       ✅ 작동
    → OuroborosEngine.interview()       ✅ LLM 호출 (실제 API)
    → OuroborosEngine.generate_seed()   ✅ LLM 호출 (실제 API)
    → AgentLifecycleManager.spawn_and_run()
      → BasicSupervisor.run_with_seed()
        → AgentRuntime.execute()        ✅ AgentLoop 실제 생성
          → ToolRegistry (Tier 1-5)     ✅ 도구 등록됨
          → AgentLoop.run()             ✅ oxi-agent 실행
            → LLM tool-calling loop     ✅ oxi-ai Provider
            → AgentEvent callback       ✅ Compaction → MemoryManager
          → ExecutionResult 반환        ✅
    → OuroborosEngine.evaluate()        ✅ LLM 평가
    → (필요시) evolve → 재실행          ✅
  → Gateway → WebChannel → HTTP 응답    ✅
```

### 무엇이 문제인가

**A. 컨테이너가 없으면 agent가 아무것도 못 함**

```
container_exec tool → active_container_name() → None → 에러
```

컨테이너가 시작되지 않은 상태에서는 에이전트가 `container_exec`를 쓸 수 없다.
`read`/`write`/`edit` 등 파일 도구는 **호스트 파일시스템**에 직접 접근한다.
이건 보안상 문제이지만, 현재는 이것마저 제대로 스코핑되지 않는다.

**B. host_exec은 실제로 작동하지만, 에이전트가 자발적으로 쓰지 않음**

`host_exec` 도구는 `HostExecBridge.exec()`를 통해 호스트 명령을 실행한다.
이건 작동한다. 하지만 에이전트의 system prompt에 "host_exec을 언제 써야 하는지"에 대한
명확한 가이드가 없다.

**C. 파일 도구(read/write/edit)의 작업 디렉토리가 명확하지 않음**

oxi-agent의 ReadTool/WriteTool/EditTool은 `current_dir` 기준으로 동작한다.
에이전트 프로세스의 cwd가 어디인지 명확하지 않다 → spawn_blocking 안에서 생성됨 →
서버 프로세스의 cwd를 상속. 이건 의도된 동작이 아니다.

**D. Gateway가 메인 루프에서 poll 방식으로만 동작**

`Gateway.run()`이 100ms마다 `receive()`를 폴링한다. `send_and_wait()` 경로도
결국 이 루프를 거쳐야 한다. 하지만 실제로는 `WebChannel.send_and_wait()`이
직접 incoming 채널에 넣고 oneshot으로 기다리므로, Gateway 루프이 처리하는 거다.

**E. Apple Container가 없는 환경에서는 전체 시스템이 무용지물**

Linux, 또는 container CLI가 없는 macOS에서는 컨테이너 생성/실행이 전부 실패한다.
현재 fallback이 없다.

---

## 1. 설계 원칙

1. **Graceful Degradation**: 컨테이너가 없으면 host_exec으로, 그것도 없으면 읽기 전용으로.
2. **Workspace Scoping**: 에이전트의 파일 작업은 반드시 특정 디렉토리 내로 제한.
3. **E2E 검증 먼저**: 모든 기능을 추가하기 전에 한 번 완전히 돌려본다.
4. **바로 작동하는 기본값**: `oxios run "hello"` 가 out-of-the-box로 동작해야 함.

---

## 2. 작업 항목

### Step 0: E2E 스모크 테스트 (검증)

**목표:** `cargo test`로 전체 파이프라인을 한 번 돌려본다.

```rust
// tests/e2e_test.rs
#[tokio::test]
async fn test_full_pipeline_with_mock_provider() {
    // MockProvider가 정해진 응답을 반환하도록 설정
    // 1. interview → "ready" 응답
    // 2. generate_seed → 고정 seed
    // 3. execute → AgentLoop이 도구 없이 완료
    // 4. evaluate → pass
    //
    // 검증: OrchestrationResult가 올바른 phase_reached와 response를 반환
}
```

이 테스트가 통과하면 전체 파이프라인이 연결되어 있음이 보장된다.

**산출물:**
- `tests/e2e_test.rs` (신규, ~200줄)
- MockProvider (oxi-ai에 이미 있으면 재사용)

---

### Step 1: Workspace Scoping — 에이전트 작업 디렉토리 제한

**문제:** 에이전트의 파일 도구가 호스트 파일시스템 전체에 접근 가능.

**해결:** `AgentRuntime.execute()`에서 `std::env::set_current_dir()`로
에이전트의 작업 디렉토리를 workspace로 제한.

```rust
// agent_runtime.rs — run_agent_loop() 시작 부분
fn run_agent_loop(...) -> Result<(String, usize, bool)> {
    // 컨테이너가 있으면 컨테이너 workspace 사용
    // 없으면 host workspace 사용
    let workspace = match container.active_container_name().now_or_never() {
        Some(Some(name)) => container.workspace_path(&name),
        _ => {
            // Fallback: ~/.oxios/workspace/agent-workspace/
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(format!("{home}/.oxios/workspace/agent-workspace"))
        }
    };

    // workspace가 존재해야 함
    std::fs::create_dir_all(&workspace).ok();

    // 에이전트의 cwd를 workspace로 설정
    // oxi-agent의 ReadTool/WriteTool/EditTool은 상대경로를 사용하므로
    // cwd 설정으로 스코핑됨
    if let Err(e) = std::env::set_current_dir(&workspace) {
        tracing::warn!(error = %e, "Failed to set agent workspace dir");
    }
    // ... 기존 코드 ...
}
```

**추가:**
- `ContainerManager.workspace_path(name)` 메서드 추가
- `AgentLoopConfig`에 `cwd` 필드가 있으면 거기에 전달
- oxi-agent의 파일 도구가 cwd를 존중하는지 확인 필요

**산출물:**
- `agent_runtime.rs`: workspace scoping
- `container_manager.rs`: `workspace_path()` 메서드
- ~50줄 변경

---

### Step 2: No-Container Fallback (Host Execution Mode)

**문제:** 컨테이너가 없으면 `container_exec`가 항상 에러.

**해결:** 두 가지 실행 모드 도입.

```rust
// config.rs
pub enum ExecutionMode {
    /// 컨테이너 내부에서 실행 (프로덕션)
    Container,
    /// 호스트에서 직접 실행 (개발/테스트)
    Host,
    /// 자동 감지: 컨테이너 백엔드가 있으면 Container, 없으면 Host
    Auto,
}
```

`container_exec` 도구가 **Host 모드**일 때는 `host_exec` 브릿지를 통해 직접 실행:

```rust
// tools/container_exec.rs 수정
async fn execute(...) -> Result<AgentToolResult, ToolError> {
    let container_name = self.container.active_container_name().await;

    match container_name {
        Some(name) => {
            // 컨테이너 내부 실행 (기존 경로)
            self.exec_in_container(&container_name, &self.container, &params).await
        }
        None => {
            // Fallback: host_exec으로 직접 실행
            // 보안: allowlist에 있는 명령만 실행
            if let Some(bridge) = &self.host_exec_bridge {
                let cmd = params["command"].as_str().unwrap_or("");
                // 검증 후 실행
                match bridge.exec("bash", vec!["-c".into(), cmd.into()], 30_000).await {
                    Ok(result) => Ok(AgentToolResult::success(
                        format!("{}\n{}", result.stdout, result.stderr)
                    )),
                    Err(e) => Ok(AgentToolResult::error(format!("Host exec failed: {e}"))),
                }
            } else {
                Ok(AgentToolResult::error(
                    "No container running and no host exec bridge available. \
                     Start a garden with 'oxios garden up <name>'."
                ))
            }
        }
    }
}
```

**산출물:**
- `config.rs`: `ExecutionMode` enum
- `container_exec.rs`: fallback 경로
- `container_manager.rs`: `host_exec_bridge()` 접근자
- ~80줄 변경

---

### Step 3: Rate Limiting

**문제:** API에 요청 제한이 없음. 악의적 클라이언트가 무한 요청 가능.

**해결:** `tower-governor` 대신 간단한 토큰 버킷 직접 구현.
(외부 의존성 최소화, `tower-governor`가 axum 0.8과 호환되지 않을 수 있음)

```rust
// middleware.rs — 기존 파일에 추가
use std::sync::Arc;
use std::time::Instant;
use parking_lot::Mutex;

/// Simple token-bucket rate limiter.
pub struct RateLimiter {
    /// (token_count, last_refill_time)
    state: Mutex<(f64, Instant)>,
    /// Maximum tokens (burst size).
    max_tokens: f64,
    /// Tokens per second refill rate.
    refill_rate: f64,
}

impl RateLimiter {
    pub fn new(max_requests_per_minute: u32) -> Self {
        Self {
            state: Mutex::new((max_requests_per_minute as f64, Instant::now())),
            max_tokens: max_requests_per_minute as f64,
            refill_rate: max_requests_per_minute as f64 / 60.0,
        }
    }

    pub fn try_acquire(&self) -> bool {
        let mut state = self.state.lock();
        let now = Instant::now();
        let elapsed = (now - state.1).as_secs_f64();
        state.0 = (state.0 + elapsed * self.refill_rate).min(self.max_tokens);
        state.1 = now;

        if state.0 >= 1.0 {
            state.0 -= 1.0;
            true
        } else {
            false
        }
    }
}
```

**산출물:**
- `middleware.rs`: RateLimiter + axum 미들웨어
- `server.rs`: rate limiter를 AppState에 추가
- `config.rs`: SecurityConfig에 rate_limit_per_minute 필드
- ~100줄

---

### Step 4: Audit Log 영속화

**문제:** AccessManager의 감사 로그가 인메모리만. 재시작 시 전부 사라짐.

**해결:** append-only 파일로 영속화.

```rust
// access_manager.rs 수정
pub struct AccessManager {
    permissions: HashMap<String, AgentPermissions>,
    audit_log: Vec<AuditEntry>,
    audit_log_path: Option<PathBuf>,
    max_audit_entries: usize,
}

impl AccessManager {
    /// 감사 로그를 파일에 append (fire-and-forget)
    fn persist_audit_entry(&self, entry: &AuditEntry) {
        if let Some(path) = &self.audit_log_path {
            let line = match serde_json::to_string(entry) {
                Ok(s) => s,
                Err(_) => return,
            };
            let path = path.clone();
            std::thread::spawn(move || {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    let _ = writeln!(f, "{}", line);
                }
            });
        }
    }
}
```

**산출물:**
- `access_manager.rs`: persist_audit_entry, audit_log_path
- `config.rs`: SecurityConfig에 audit_log_path 필드
- ~60줄

---

### Step 5: oxi 의존성 안정화

**문제:** `path = "../oxi/oxi-ai"` — 빌드 재현 불가.

**해결:** git tag 기반 의존성으로 전환.

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
oxi-ai = { git = "https://github.com/a7garden/oxi", tag = "v0.5.0" }
oxi-agent = { git = "https://github.com/a7garden/oxi", tag = "v0.5.0" }
```

개발 중에는 path 의존성 유지:
```toml
# 개발용 (Cargo.toml.local)
[workspace.dependencies]
oxi-ai = { path = "../oxi/oxi-ai" }
oxi-agent = { path = "../oxi/oxi-agent" }
```

**산출물:**
- `Cargo.toml`: workspace.dependencies에 git tag 지정
- `.gitignore`: `Cargo.toml.local` 추가
- README에 개발 설정 안내

---

### Step 6: 릴리즈 파이프라인

**문제:** 배포 메커니즘이 없음.

**해결:**

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
        with:
          path: oxios
      - uses: actions/checkout@v4
        with:
          repository: a7garden/oxi
          path: oxi
          ref: v0.5.0
      - uses: dtolnay/rust-toolchain@stable
      - name: Build release
        working-directory: oxios
        run: cargo build --release
      - uses: softprops/action-gh-release@v2
        with:
          files: oxios/target/release/oxios
```

**산출물:**
- `.github/workflows/release.yml`
- `scripts/install.sh`
- ~80줄

---

## 3. 구현 순서

```
Step 0: E2E 스모크 테스트        → 전체 파이프라인 검증     (1일)
Step 1: Workspace Scoping          → 보안 기본               (반나절)
Step 2: No-Container Fallback      → 개발 편의성             (반나절)
Step 3: Rate Limiting              → 보안 완성               (반나절)
Step 4: Audit Log 영속화           → 컴플라이언스            (반나절)
Step 5: oxi 의존성 안정화          → 빌드 재현성             (10분)
Step 6: 릴리즈 파이프라인          → 배포 준비              (반나절)
```

총 예상: **3-4일**

---

## 4. 성공 기준

1. **`oxios run "hello world"`** 가 컨테이너 없이도 응답을 반환한다
2. **`cargo test --workspace`** 가 270+ 테스트를 통과한다
3. **E2E 테스트**가 mock provider로 전체 Ouroboros 사이클을 검증한다
4. **Rate limiter**가 분당 60요청 이상을 차단한다
5. **Audit log**가 `~/.oxios/workspace/audit.log`에 append된다
6. **`cargo build`** 가 `../oxi` 없이도 git tag로 빌드된다

---

## 5. 이후 로드맵 (Loop 7+)

| Loop | 주제 | 내용 |
|------|------|------|
| 7 | **Agent Tools 심화** | 컨테이너 내부 toolchain 설치, workspace 초기화 |
| 8 | **Metrics & Observability** | Prometheus 메트릭, 구조화된 로깅 |
| 9 | **TUI** | ratatui 기반 로컬 대시보드 |
| 10 | **v1.0.0** | 크레이트 게시, 안정 API, 문서화 |
