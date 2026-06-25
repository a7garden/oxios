# RFC-030: 런타임 태스크 감시 (Runtime Task Supervision)

> **Status:** Implemented (2026-06-25) — Parts A + B + D(supervisor metric). Part C dropped (see below).
> **Created:** 2026-06-25
> **Depends on:** RFC-024 (web↔daemon 연결 신뢰성 — 본 RFC는 그 "서버가 살아있다" 전제를 보장한다)
> **관계:** RFC-024 SP4(시작 readiness)가 시작 시점 1회 검증을 다룬다면, 본 RFC는 **시작 이후 런타임 내내** 태스크 생존을 감시한다.
>
> ## Implementation Status (as-built)
>
> **구현됨 (`src/supervisor.rs` + `cmd_serve`):**
> - **A:** `TaskSupervisor` — `FuturesUnordered` 기반 비블로킹 감시 루프(R7), `FailFast`/`ScopedRestart` 정책, bounded backoff. gateway·channel = FailFast, web = ScopedRestart.
> - **A5:** `CancellationToken`(tokio-util) 단일 종료 신호. `SurfaceContext.shutdown` 추가, 웹 서피스 자체 `ctrl_c` 제거.
> - **A6:** `ShutdownOutcome::{Graceful,Fatal}` 모두 동일 drain 수행 후, Fatal만 non-zero 종료(`process::exit(1)` → OS supervisor 재시작).
> - **B:** `WebSurfaceRestarter` — `unregister("web")` → `WebSurface::start()` 재호출 → `register`. 크래시 시(태스크 이미 종료) drain 없이 재기동.
> - **D:** `oxios_supervisor_restarts_total` 메트릭 추가(재시작 가시성).
>
> **제외됨 — Part C (CatchPanic):** release 프로필이 `panic = "abort"`이므로 `CatchPanicLayer`(catch_unwind 기반)가 production에서 무력화됨. 대신 production panic 복구 = **panic → 프로세스 즉시 abort → OS supervisor(systemd/launchd) 재시작**으로 자동 fail-fast가 이미 프로세스 레벨에서 작동. supervisor가 포착하는 것은 non-panic 실패 경로(`axum::serve` Err 반환 등) — 이것이 현실적·고가치 실패 모드.
>
> **검증:** 바이너리 테스트 87개 통과(supervisor 단위: fail-fast 감지·graceful·backoff 수학), clippy clean, 런타임 스모크(웹 기동 `/health 200` → SIGINT → graceful rc=0).

## Problem

데몬이 Web UI를 제공할 때, **웹 서버 태스크가 런타임에 에러/패닉로 종료되면 아무도 이를 감지·복구하지 못한다.** 데몬은 "반죽음(half-dead)" 상태로 무기한 방치된다.

### 근본 원인 (코드 검증 완료)

`cmd_serve`(`src/main.rs`)가 수집한 중요 태스크 핸들을 **런타임에 한 번도 관측하지 않는다.** fire-and-forget이다.

| 위치 | 코드 | 문제 |
|------|------|------|
| `src/main.rs:2842` | `gateway.run().await.expect(...)` 를 `tokio::spawn` | gateway 에러 시 panic. 핸들 미관측 → panic 조용히 삼켨짐 |
| `src/main.rs:2830` | `surface_tasks: Vec<JoinHandle>` | 런타임에 await 안 함. 종료 시(`2877`) `.abort()`로만 정리 |
| `src/main.rs:2870` | `tokio::signal::ctrl_c().await` | 메인 루프가 **오직 ctrl_c만 대기**. 태스크 사망 무감지 |
| `src/api/plugin.rs:350` | `axum::serve(...).await` Err 시 `tracing::error!`만 | 로그 남기고 태스크 종료. 재시작 없음 |

### 결과 — 반죽음 상태

1. 웹 서버 태스크 사망 → HTTP/WS/SSE 전부 단절
2. 그러나 프로세스는 살아있음 (메인 루프가 ctrl_c 대기 중) → gateway·kernel·in-flight agent 정상
3. `oxios status`는 PID 파일 기반으로 **여전히 "Running"** 보고
4. 사용자는 "갑자기 안 됨"만 겪고, 원인·복구 단서 없음

### OS supervisor도 무력하다

`daemon.rs`의 launchd(`KeepAlive=true`)/systemd(`Restart=on-failure`)는 **프로세스 종료 시에만** 재시작한다. 프로세스가 죽지 않으므로(메인 루프 생존) supervisor는 영원히 감지 못 한다. **"OS supervisor가 이미 처리한다"는 거짓이다.**

### RFC-024와의 관계

RFC-024의 모든 불변조건(C1 응답보장, C2 재생, C3 멱등, C4 자산무결)은 **"서버가 살아있다"**를 전제로 한다. SP4마저 "시작 시 1회 readiness 게이트"이다. **그 전제 자체를 런타임에 보장하는 계층이 없다** — 연결 재연결 메커니즘이 아무리 정교해도, 서버 태스크가 죽으면 재연결할 대상이 사라진다. 본 RFC가 그 빈 칸을 채운다.

```
RFC-024 (연결 신뢰성)        본 RFC (런타임 감시)
┌──────────────────┐        ┌──────────────────────────┐
│ SSE/WS 재연결    │        │ TaskSupervisor           │
│ 자산 atomic swap │   ←──  │  select! over handles    │
│ readiness 게이트 │  전제:  │  fail-fast / scoped restart│
└──────────────────┘ 서버 생존 └──────────────────────────┘
```

## Design Overview

### 핵심 불변조건

> **C5 (Liveness — 감지):** supervisor가 추적하는 모든 중요 태스크는 예기치 않은 종료 시 **데드라인 내** 감지된다. 어떤 태스크도 조용히 죽지 않는다.
>
> **C6 (Observability):** 모든 예기치 않은 종료는 진단 가능한 컨텍스트와 함께 로깅되고, 조치(fail-fast/restart/tolerate)가 기록된다.
>
> **C7 (복구 계약):** 복구 가능한 태스크(서피스)는 bounded backoff로 자동 재시작된다. 복구 불가능한/위험한 태스크(gateway, 커널 핵심)는 프로세스 비정상 종료로 승격되어 OS supervisor가 known-good 상태로 재시작한다.

### 아키텍처

`cmd_serve`의 `tokio::signal::ctrl_c().await`를 **`TaskSupervisor` 메인 루프**로 교체한다. supervisor는 `ctrl_c` + 추적 중인 `JoinHandle`들을 동시에 `select!`/`FuturesUnordered`로 관측한다.

```
┌─────────────────────────────────────────────────────────────┐
│  cmd_serve (binary, src/main.rs)                            │
│                                                             │
│  TaskSupervisor::run()  ←── ctrl_c만 기다리던 구역을 대체    │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  select! / FuturesUnordered over:                     │  │
│  │    • ctrl_c                  → graceful shutdown      │  │
│  │    • gateway JoinHandle      → FailFast               │  │
│  │    • web surface JoinHandle  → ScopedRestart          │  │
│  │    • channel JoinHandles     → FailFast (기본)         │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────┬───────────────────────────┬───────────────┘
                  │                           │
        FailFast: process exit non-zero       ScopedRestart: surface.start() 재호출
                  │                           │ (unregister 구 채널 → re-bind → register)
                  ▼                           ▼
        OS supervisor 재시작            in-flight agent 생존, ~1s 내 UI 복귀
        (systemd/launchd)               (backoff: 0.5s→30s, 최대 5회)
```

### Key Decisions

| 결정 | 선택 | 근거 |
|------|------|------|
| **감시 위치** | 바이너리 크레이트(`src/supervisor.rs`) | 프로세스 레벨 오케스트레이션은 바이너리의 책임. gateway 크레이트는 라이브러리(자기 자신을 재시작하는 건 아님) |
| **gateway 정책** | **FailFast** | gateway는 모든 라우팅 상태를 보유. in-process 재시작은 불안전(반쪽짜리 상태). clean 프로세스 재시작이 안전 |
| **web 서피스 정책** | **ScopedRestart**(기본) | 재시작이 저렴(re-bind + 라우터 재구성). in-flight agent(kernel/gateway에 있음) 생존. UX 최적 |
| **channel 정책** | **FailFast**(기본) | CLI/Telegram 채널 사망 = 해당 채널 단절. daemon에서는 보통 단일 채널 세트이므로 clean 재시작이 단순 |
| **정책** | 설정 가능(`config.toml`) | 운영자가 환경에 맞게 fail-fast ↔ scoped 전환 가능 |

---

## Part A: TaskSupervisor (코어)

### A1. 위치와 책임

`src/supervisor.rs` (신규, 바이너리 크레이트). `cmd_serve`에서 호출. gateway 크레이트의 `Channel`/`Surface` trait은 변경하지 않는다(단, 서피스 재시작을 위해 `Gateway::unregister`를 사용 — 이미 존재, `gateway.rs:240`).

### A2. 정책 모델

```rust
/// supervisor가 예기치 않은 태스크 종료에 대응하는 방식.
pub enum SupervisionPolicy {
    /// 예기치 않은 종료 → 로깅 → cmd_serve가 Err 반환 → 프로세스 non-zero 종료.
    /// OS supervisor(systemd/launchd)가 known-good 상태로 재시작.
    FailFast,

    /// 예기치 않은 종료 → bounded backoff로 태스크 재시작.
    /// reset_window 내 안정 동작하면 재시도 카운터 리셋(폭주 방지).
    /// 재시도 예산 소진 → FailFast로 승격.
    ScopedRestart {
        max_retries: u32,           // 기본 5
        reset_after: Duration,      // 기본 5분 — 이 기간 안정 시 카운터 리셋
        initial_backoff: Duration,  // 기본 500ms
        max_backoff: Duration,      // 기본 30s
        jitter: bool,               // 기본 true — 동시 재시작 폭주 방지
    },

    /// 예기치 않은 종료 로깅만. 치명적 아님(예: 옵션 telemetry).
    Tolerate,
}
```

### A3. 추적 항목 모델

```rust
enum SupervisedTask {
    /// FailFast/Tolerate — 재시작 불가. 핸들 종료 시 정책 적용.
    Pinned {
        name: String,
        handle: JoinHandle<Result<()>>,  // Result를 반환하도록 변경 (panic = JoinError)
        policy: SupervisionPolicy,
    },
    /// ScopedRestart — 팩토리로 재생성 가능.
    Restartable {
        name: String,
        handle: JoinHandle<()>,
        factory: SurfaceFactory,   // 서피스 재시작용 클로저
        policy: ScopedRestartPolicy,
        retries: u32,
        last_start: Instant,
    },
}
```

> **gateway_task 변경:** 현재 `gateway.run().await.expect(...)` (`main.rs:2843`)를
> `gateway.run().await`(Result 전파)로 변경. supervisor가 `Ok(Ok(()))`(정상) vs
> `Ok(Err(e))`(gateway 에러 반환) vs `Err(JoinError)`(panic)를 구분하도록. `.expect()` 제거.

### A4. 감시 루프

```rust
impl TaskSupervisor {
    /// cmd_serve의 메인 대기 지점. **절대 블록하지 않는다 — 항상 select! 안에 있다.**
    /// (R7: backoff sleep을 인라인으로 하면 ctrl_c·gateway 사망이 최대 30초 블린드된다.
    ///  본 루프는 원 문제와 같은 부류의 버그(차단 대신 select)를 범하지 않는다.)
    pub async fn run(mut self) -> Result<ShutdownOutcome> {
        loop {
            let action = tokio::select! {
                // 1) 사용자 정상 종료
                _ = tokio::signal::ctrl_c() => return Ok(ShutdownOutcome::Graceful),
                // 2) 추적 중인 핸들 종료 — gateway 사망(FailFast)도 즉시 여기서 포착.
                //    web backoff sleep 중에 gateway가 죽어도 next_completion이 곧바로 발화.
                Some((idx, result)) = self.next_completion() => {
                    self.on_completion(idx, result).await?
                }
                // 3) 대기 중인 재시작의 backoff 기한 도래 — 슬립 대신 타이머 분기.
                //    pending이 없으면 무한 future(절대 발화 ❌)로 select에 무해.
                _ = self.restart_timer() => self.fire_pending_restart().await?,
            };
            match action {
                CompletionAction::Continue => continue,           // 재시작 예약/완료
                CompletionAction::Fatal { name, reason } => {
                    return Ok(ShutdownOutcome::Fatal { name, reason });
                }
                CompletionAction::Graceful => {
                    return Ok(ShutdownOutcome::Graceful);
                }
            }
        }
    }
}
```

**`on_completion` 의사코드 — 절대 블록/슬립하지 않음 (R7):**

- **Pinned + FailFast:** 어떤 이유(Ok(Err), Err(JoinError))로든 종료 → `Fatal` 반환
- **Pinned + Tolerate:** 로깅 후 `Continue`(핸들 제거)
- **Restartable (크래시 수신):**
  1. `last_start` 이후 `reset_after` 경과 시 `retries = 0` 리셋
  2. `retries >= max_retries` → `Fatal` 승격 (재시도 예산 소진)
  3. backoff deadline 계산(`2^retries * initial`, cap `max_backoff`, ±jitter)
  4. `{deadline, factory, policy, retries}`를 **pending-restart 슬롯에 기록** (슬립 ❌)
  5. `restart_timer`가 가장 이른 deadline을 가리키도록 갱신 → `Continue` 반환

**`fire_pending_restart` (restart_timer 발화 시):** Part B 시퀀스(`unregister` →
`start()` 재호출 → `register`) 실행. re-bind 실패 시 새 deadline 재기록(다음 backoff).
성공 시 새 핸들로 교체. **이 함수도 긴 슬립이 없다** — unregister/register는 짧은 lock
획득, `start()`는 포트 re-bind(빠름) + 태스크 spawn(비블록).

> **왜 중요한가:** 크래시한 웹 서피스의 backoff(최대 30s) 도중 gateway가 죽으면,
> `next_completion`이 즉시 발화해 `Fatal`이 지연 없이 전파된다. 반면 이전(인라인 sleep)
> 설계에서는 sleep 종료까지 gateway 사망이 최대 30초 숨겨졌다.

### A5. 공유 종료 신호 (CancellationToken) — graceful drain + 재시작 조정

> **리뷰에서 발견한 핵심 결함 (R1).** 본 RFC가 처음 놓쳤던 4번째 변경점이다.

**문제:** 웹 서피스는 자체 `ctrl_c`를 소비한다(`plugin.rs:353`).
```rust
// 현재 — 서피스가 독자적으로 ctrl_c를 소비
axum::serve(listener, app)
    .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok() })
```
supervisor가 ctrl_c를 소유하면 **독립된 ctrl_c 소비자가 2개** 공존한다. 게다가
`cmd_serve` 종료 시 `task.abort()`(`main.rs:2877`)로 서피스 태스크를 잘라내는데,
이는 axum의 in-flight 요청 drain을 끊는다 — "graceful" shutdown이 장시간
SSE/WS/agent API 호출을 drop한다. ScopedRestart 후에는 새 웹 태스크가 또 자체
ctrl_c를 가지므로 supervisor가 그것을 멈출 유일한 수단이 abort뿐이다 (graceful 경로 없음).

**해결:** `tokio_util::sync::CancellationToken`을 **단일 종료 신호**로 사용한다.

- supervisor가 **루트 토큰**을 소유. ctrl_c 수신 시 `root.cancel()` → 모든 자식 토큰 연쇄 취소.
- `SurfaceContext`에 `shutdown: CancellationToken` 필드 추가 (또는 `start` 인자).
  `WebSurface::start`는 이를 axum에 전달(자체 ctrl_c 제거):
  ```rust
  axum::serve(listener, app)
      .with_graceful_shutdown(ctx.shutdown.cancelled())
      .await
  ```
- **크래시 재시작 vs graceful 전체 종료 — drain은 후자만:**
  - **크래시 재시작(R7와 정합):** 서피스 태스크는 이미 종료됨. cancel할 live 인스턴스가
    없다. supervisor는 단순히 stale 채널 `unregister` + 새 `child = root.child_token()`으로
    `start()` 재호출 + `register`. **drain/await 없음** — 블록 포인트를 만들지 않는다.
  - **graceful 전체 종료:** `root.cancel()` → 모든 live 자식(웹 인스턴스 포함) 연쇄 취소 →
    axum이 in-flight 요청 drain → `cmd_serve`가 핸들을 **timeout과 함께 await**(abort 아님).
    이 drain-await는 cmd_serve의 **최종 cleanup**에서 일어나며(supervisor 루프 밖),
    루프의 select! 블린드를 유발하지 않는다.
  - abort는 graceful timeout 초과 시에만 **최후 수단**.

`SupervisedTask::Restartable`에 per-instance `token: CancellationToken` 필드 추가.
이로써 **종료 신호 원천이 단일**이고, 재시작 간에도 일관된 graceful 경로가 유지된다.

### A6. 종료 시퀀스 정합성 — Fatal도 cleanup을 수행한다

> **리뷰에서 발견한 결함 (R2).**

`ShutdownOutcome::Fatal`이 `cmd_serve`에 곧바로 에러로 전파되면, 현재의 cleanup 단계
(agent 종료 `main.rs:2893`, MCP shutdown `2915`, audit flush `2920`)을 건너뛴다.
→ orphaned agent + flush 안 된 audit = 손상 위험.

**수정:** `cmd_serve`는 `supervisor.run()` 결과(Graceful/Fatal **모두**)에 대해
**동일한 cleanup 파이프라인**을 수행한 뒤, Fatal일 때만 non-zero로 종료한다:

```rust
match supervisor.run().await? {
    ShutdownOutcome::Graceful => {
        run_cleanup(kernel, graceful=true).await;   // phases 1-4 (기존)
        Ok(())
    }
    ShutdownOutcome::Fatal { name, reason } => {
        run_cleanup(kernel, graceful=false).await;  // 동일 cleanup, 단축 timeout
        Err(anyhow!("critical task '{name}' exited: {reason}"))
    }
}
```

cleanup은 결과(Graceful/Fatal)와 무관하게 항상 수행. **종료 코드만 다르다.**
(`main.rs:1717` 검증: `run()`이 `Err`면 `std::process::exit(1)` → OS supervisor 재시작 트리거. C7 계약 충족.)

---

## Part B: 서피스 범위 재시작 계약

### B1. 재시작 시퀀스

`WebSurface`는 unit struct이고 `Surface::start(&self)`는 상태가 없으므로 재호출 가능(검증 완료, `plugin.rs:214`). 재시작 시:

```rust
// 1. 구 채널 해제 — gateway registry에서 제거 + 구 receive 태스크 정지
gateway.unregister("web").await?;

// 2. 서피스 재시작 — 새 WebBridge, 새 AppState, re-bind, 새 axum 태스크
let handle = web_surface.start(ctx).await?;

// 3. 신 채널 등록
if let Some(channel) = handle.channel {
    gateway.register(channel).await?;
}

// handle.tasks[0]을 supervisor에 새 핸들로 교체
```

`Gateway::unregister`(`gateway.rs:240`)와 `register`(`gateway.rs:216`)가 이미 존재하므로 추가 게이트웨이 API 불필요.

### B2. 포트 re-bind 경쟁

구 listener drop 직후 같은 포트 re-bind 시 `TIME_WAIT` 경쟁 가능. 완화:
- tokio `TcpListener`는 Unix에서 기본 `SO_REUSEADDR` (이미 켜져 있음)
- ScopedRestart backoff가 자연스럽게 간극 제공 (재시도 전 대기)
- re-bind `Err` 시 별도 에러로 분류 → backoff 증가 후 재시도 (일반 panic/에러와 구분)

### B3. 재시작 중 가시성

재시작 시 `tracing::warn!`으로 이름·재시도 번호·backoff·원인 로깅. supervisor 메트릭(`supervisor_task_restarts_total`) 증가.

---

## Part C: 핸들러 패닉 격리 (defense-in-depth)

### C1. CatchPanic 레이어

`tower_http::catch_panic::CatchPanicLayer`를 라우터에 추가. 핸들러 panic → 500 JSON 응답(연결 drop 아님).

```rust
use tower_http::catch_panic::CatchPanicLayer;
let app = Router::new()
    .merge(api_routes)
    .merge(spa_routes)
    .layer(CatchPanicLayer::new())   // ← 신규
    .layer(cors);
```

> **범위 정확성:** axum/tokio는 연결별 태스크이므로 핸들러 panic 자체가 서버를 죽이지는 않는다(해당 연결만 drop). CatchPanic은 단일 요청 panic이 연결 reset으로 나타나는 것을 500으로 바꾸는 방어막이다. **서버 태스크 사망 자체는 Part A/B의 supervisor가 담당한다.** 둘은 직교하는 방어층.

### C2. panic 후 상태 일관성

CatchPanic으로 500을 반환한 뒤에도 공유 상태(AppState, kernel)는 손상 가능. panic = 버그 신호이므로, 심각한 경우 500 응답과 함께 metric 카운트(`handler_panics_total`). 임계치 초과 시 운영자 알림(메트릭 대시보드에서).

---

## Part D: 관측 가능성

### D1. 로깅

모든 태스크 종료 이벤트:
- `INFO`: 정상 종료(expected, graceful shutdown)
- `WARN`: 예기치 않은 종료 + 재시작 시도 (이름, 재시도 n/max, backoff, 원인)
- `ERROR`: 재시도 예산 소진 → fail-fast 승격 / 치명적 태스크 사망

### D2. 메트릭 (`/metrics`, RFC-024와 통합)

| 메트릭 | 유형 | 의미 |
|--------|------|------|
| `supervisor_task_exits_total{task, outcome}` | counter | expected / restarted / escalated |
| `supervisor_task_restarts_total{task}` | counter | 서피스 재시작 횟수 |
| `supervisor_restart_backoff_seconds{task}` | histogram | 실제 backoff 대기 시간 |
| `handler_panics_total` | counter | CatchPanic이 잡은 핸들러 panic |

### D3. `oxios status` liveness 향상 (선택)

현재 `oxios status`는 PID 파일만 검사(`daemon.rs:52`). 반죽음 상태 감지를 위해 웹 서버 포트 TCP probe 추가(`wait_until_listening`과 동일 기법). PID 살아있음 + 포트 응답 없음 = "degraded" 보고. (별도 소스프로젝트; 본 RFC 코어 아님.)

---

## Configuration (`config.toml`)

```toml
[supervisor]
# 웹 서피스 재시작 정책
web_restart_enabled = true            # false 시 fail-fast
web_restart_max_retries = 5
web_restart_reset_after_secs = 300    # 5분 안정 시 카운터 리셋
web_restart_initial_backoff_ms = 500
web_restart_max_backoff_ms = 30000
```

기본값은 daemon 모드에 적합하게 설계됨(자동 복구 우선). `oxios --foreground` 수동 디버깅 시에도 동일하게 동작하며, 재시도 예산 소진 시 프로세스가 에러와 함께 종료되므로 사용자가 원인을 즉시 인지.

---

## Testing Strategy

### 단위 테스트 (`src/supervisor.rs`)

- **정책 전이:** FailFast 태스크 종료 → `Fatal` 반환. Tolerate → `Continue`. ScopedRestart 재시도 카운터/리셋/backoff 계산 정확성.
- **backoff 계산:** `2^n * initial`, cap, jitter 범위. `reset_after` 경과 시 카운터 리셋.
- **재시도 예산 소진:** `max_retries` 도달 → `Fatal` 승격.

### 통합 테스트 (workspace `tests/`)

- **C5 감지:** 가짜 웹 서피스 태스크를 즉시 종료 → supervisor가 backoff 후 재시작하는지 확인. 카운터 증가.
- **C7 복구:** 웹 서피스 크래시 → 1초 내 새 axum 인스턴스가 같은 포트 재수용 → HTTP 200 회복. in-flight agent(별도 태스크)는 계속 동작.
- **fail-fast:** gateway 태스크 크래시 → supervisor가 `Fatal` 반환 → `cmd_serve`가 Err → (테스트에서는 프로세스 종료 대신 에러 전파 검증).
- **CatchPanic:** panic 유발 핸들러 요청 → 500 JSON 응답, 서버는 계속 정상 응답.
- **R7 (비블로킹 감시):** 웹 서피스 backoff 대기 중 gateway(FailFast) 태스크 종료 → backoff sleep 만료 **전** `Fatal`이 즉시 전파되는지 (최대 30s 지연 ❌). backoff 중 ctrl_c도 즉시 응답하는지.

### 카오스/스트레스

- 웹 서피스 연속 크래시(10회) → 재시도 예산 소진까지 backoff 증가 → 최종 fail-fast. 재시작 폭주(storm) 없음.
- 재시작 중 동시 HTTP 요청 → 일시적 connection refused 후 backoff 내 회복.
- 5분 안정 후 단발 크래시 → 재시도 카운터가 리셋되어 예산이 소진되지 않음.

---

## Non-Goals (명시적 범위 제한)

- **gateway in-process 재시작은 안 한다.** gateway는 치명적 → fail-fast(프로세스 재시작)만. in-process gateway 재시작은 불안전(공유 라우팅 상태).
- **커널 자체 재시작은 안 한다.** kernel 크래시 = 프로세스 크래시 = fail-fast.
- **분산/다중 인스턴스 감시는 안 한다.** 단일 데몬 인스턴스 한정.
- **`oxios status` 포트 probe는 선택 소스프로젝트** (Part D3). 본 RFC 코어는 supervisor 자체.
- **무한 재시작은 안 한다.** 항상 bounded. 예산 소급 시 fail-fast.

---

## Risks & Mitigations

| 리스크 | 영향 | 완화 |
|--------|------|------|
| ScopedRestart가 근본 원인을 가림 | 진단 | backoff 전 원인 로깅 + `supervisor_task_restarts_total` 메트릭으로 반복 크래시 가시화. 임계치 알림 |
| 포트 re-bind TIME_WAIT 경쟁 | 재시작 실패 | tokio SO_REUSEADDR + backoff + re-bind 에러 별도 분류 |
| 재시작 중 짧은 가용성 공백 | UX | backoff 최소화(기본 500ms). in-flight agent는 kernel에 있어 영향 없음 |
| supervisor 자체 버그 → 데몬 안 올라옴 | 가용성 | supervisor 로직 단순 유지 + 단위 테스트. supervisor는 시작 검증 없이 바로 동작(추가 게이트 없음) |
| pending-restart 타이머/상태머신 버그 | 재시작 누락·중복 | `restart_timer` 단위 테스트: deadline 갱신, 동시 크래시, pending 없을 때 무한 future(select에 무해) |
| CatchPanic이 panic 후 상태 손상을 숨김 | 정확성 | 500 응답 + `handler_panics_total` 카운트로 가시화. panic = 버그 신호 |
| OS supervisor 미설치 시 fail-fast = 수동 재시작 | 운영 | 데몬 권장 설치 안내. ScopedRestart 기본값이 대부분 자동 복구 |

---

## Build Order

```
  ┌──────────────────────────────────┐
  │ A: TaskSupervisor 코어            │  ← 정책 모델 + 이벤트 기반 루프 (R7)
  │  (3분기 select! + CancellationToken) │     (gateway .expect 제거 + Fatal cleanup 포함)
  └─────────────┬────────────────────┘
                │ 의존 (supervisor 프레임워크)
                ▼
  ┌──────────────────────────────────┐
  │ B: 서피스 범위 재시작             │  ← unregister/re-start/register
  │  (WebSurface 재호출 계약)         │     시퀀스 + re-bind 처리
  └──────────────────────────────────┘

  ┌──────────────────┐
  │ C: CatchPanic    │  ← 독립, A/B와 병렬 (tower-http 레이어만)
  │  (defense-in-d)  │
  └──────────────────┘
```

권장 순서: **C(빠른 승리) → A → B.** C는 라우터 1줄 추가로 즉시 효과. A는 이벤트 기반 감시 + CancellationToken + gateway fail-fast로 가장 위험한 반죽음(gateway 사망)을 막는다. B는 web 자동 복구로 UX 완성.

---

## Implementation Checklist

- [ ] **A:** `src/supervisor.rs` 신규 — `SupervisionPolicy`, `TaskSupervisor`, 감시 루프
- [ ] **A:** `cmd_serve`의 `ctrl_c().await`를 `TaskSupervisor::run()`으로 교체
- [ ] **A:** `gateway_task`의 `.expect()` 제거 → Result 전파 (panic = JoinError)
- [ ] **A:** gateway/channel 핸들 `FailFast`로 등록
- [ ] **A:** 감시 루프를 완전 이벤트 기반으로 (R7) — backoff 인라인 sleep ❌, deadline 기록 + `restart_timer` select 분기. pending 없으면 무한 future
- [ ] **A:** `CancellationToken`(tokio_util) 스레딩 — `SurfaceContext.shutdown` 추가 + 웹 서피스 자체 `ctrl_c` 제거 (A5)
- [ ] **A:** `ShutdownOutcome::Fatal`도 cleanup 수행 (A6) — agent 종료·audit flush 후 non-zero 종료 (`process::exit(1)` 확인)
- [ ] **B:** `SurfaceFactory` + `unregister`/`re-start`/`register` 재시작 시퀀스
- [ ] **B:** re-bind `TIME_WAIT` 에러 분류 + backoff 연동
- [ ] **B:** web 서피스 `ScopedRestart` 정책 연결
- [ ] **C:** `CatchPanicLayer` 라우터 추가 + panic → 500 JSON 매핑
- [ ] **D:** 메트릭 4종 추가 (`supervisor_task_exits_total` 등)
- [ ] config 스키마(`[supervisor]`) + 기본값
- [ ] 단위 테스트 (정책 전이, backoff, 예산 소진)
- [ ] 통합 테스트 (C5 감지, C7 복구, fail-fast, CatchPanic)
