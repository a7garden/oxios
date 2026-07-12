# RFC-040: 감시 계층 완결성 (Supervision Layer Completeness)

> **Status:** Proposed
> **Created:** 2026-07-12
> **Depends on:** RFC-030 (Runtime Task Supervision), RFC-024 (Web↔Daemon Reliability)
> **관계:** RFC-030이 태스크 completion 감시를 추가했다면, 본 RFC는 **completion으로 잡을 수 없는 실패 모드(hang)**와 **감시 계층 자체의 검증 부재**를 채운다.

## Problem

RFC-030이 TaskSupervisor를 추가했지만 5개의 gap이 식별됨:

| # | Gap | Severity | Root Cause |
|---|-----|----------|------------|
| 1 | Guardian hang 감지 불가 | HIGH | `tasks.next()`는 completion만 관측. Guardian은 `loop {}`라 completion이 없고, 4개 호출(verify_chain, is_overloaded, git_verify, commit_all)이 전부 **sync**라 worker thread를 점유 → completion이 영원히 안 옴 |
| 2 | cmd_serve cleanup에 timeout 없음 | MED | `mcp.shutdown_all()` / `flush_audit()`(sync)가 hang하면 process가 영원히 안 끝남 → OS supervisor 재시작 안 됨 |
| 3 | CI가 감시 계층을 검증 안 함 | HIGH | ubuntu 단일 매트릭스. macOS launchd 코드 미컴파일. spawn→probe→SIGTERM 에스컬레이션, web 재시작, fatal drain, Guardian heartbeat 미테스트 |
| 4 | OS supervisor thrashing 방지 없음 | MED | systemd `StartLimitBurst` / launchd `ThrottleInterval` 미설정 → crash 시 무한 빠른 재시작 |
| 5 | DST 전환일 schedule 계산 panic | LOW | `and_local_timezone().unwrap()`이 skip 시간에서 panic → abort → 재시작 (시간 단위로 자체 종료하므로 "무한"은 아니지만, 유효한 LOW nit) |

### Gap #1의 정확한 메커니즘

```
Guardian loop (kernel.rs:563)
  │
  ├─ sleep(300s).await     ← async, OK
  │
  ├─ verify_chain()        ← SYNC: audit_trail.verify() — file I/O
  ├─ is_overloaded()       ← SYNC: resource_monitor.snapshot() — syscall
  ├─ git_verify()          ← SYNC: git_layer.verify() — gix read
  └─ commit_all()          ← SYNC: git.commit_file() — gix write (★ hang 위험 최대)
```

이 4개 호출이 async worker thread에서 직접(또는 `async {}` 블록 내에서) 호출되면:

1. `tokio::time::timeout(60s, async { handle.commit_all(...) }).await`를 쓴다고 가정.
2. timeout future가 poll될 때, inner future가 `commit_all()`에 진입.
3. `commit_all()`은 동기 blocking call — **worker thread를 독점**.
4. timeout의 `Sleep`이 60s에 발화해도, task를 선점(preempt)할 수 없음 (cooperative async).
5. `commit_all()`이 반환할 때까지 worker thread는 막혀 있고, timeout은 poll될 기회가 없음.
6. `commit_all()`이 file lock deadlock으로 영원히 hang → **timeout은 결코 발화하지 않음**.

즉, "각 호출을 timeout으로 감싼다"는 설계는 false confidence를 준다. sync blocking call에는 `tokio::time::timeout`이 무력하다.

---

## Design Overview

```
┌──────────────────────────────────────────────────────────────┐
│                    TaskSupervisor::watch()                     │
│                                                                │
│  select! {                                                    │
│    biased;                                                    │
│    ctrl_c            → Graceful                               │
│    root.cancelled()  → Graceful                               │
│    tasks.next()      → on_completion (Critical: Fatal)        │
│    restart_timer     → fire_web_restart                        │
│    heartbeat_timer   → Guardian staleness check ★NEW          │
│  }                                                            │
│                                                                │
│  drain (both outcomes):                                       │
│    1. cancel root + await tracked tasks (timeout)             │
│    2. Kernel::cleanup(timeout) ★MOVED HERE                    │
└──────────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────┐
│  Guardian task (Critical + heartbeat)                          │
│                                                                │
│  loop {                                                       │
│    sleep(300s)                                                 │
│    spawn_blocking(guardian_tick_sync)  ← sync ops offloaded   │
│      └─ timeout(180s)                                          │
│    on completion: heartbeat.store(now)                         │
│    on timeout:   heartbeat NOT updated ← hang signal          │
│  }                                                            │
│                                                                │
│  Heartbeat watchdog (supervisor select! branch):              │
│    every 60s: if now - heartbeat > 900s → process::abort()    │
└──────────────────────────────────────────────────────────────┘
```

### Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Guardian 정책 | **Critical** + heartbeat | Guardian은 백그라운드 integrity checker. clean exit = Fatal (process restart). hang = heartbeat staleness → abort |
| Hang 감지 (주) | **Heartbeat watchdog** | `tasks.next()`는 completion만 관측. sync blocking call이 worker를 점유하면 completion이 영원히 안 옴. heartbeat는 kernel 호출 없이 `AtomicU64` 비교만 하므로 wedge된 kernel에서도 살아남 |
| Sync op 처리 | **spawn_blocking + timeout** | sync call을 async worker에서 직접 부르면 timeout future 자체가 poll될 기회를 얻지 못함. spawn_blocking으로 blocking pool에 오프로드해야 timeout이 의미를 가짐 |
| Stale 시 동작 | **`process::abort()`** | Guardian wedge = kernel 자체가 의심스러운 상태. graceful cleanup은 정상 kernel을 전제 → 더 오래 hang하거나 손상 위험. abort → OS supervisor가 known-good으로 재시작 (L4/L5 경로) |
| Cleanup 위치 | **`cmd_serve` 본체 → `Kernel::cleanup(timeout)`** | `supervisor.run()` 이후 순차 실행이므로 race 없음. timeout으로 hang 방지. flush_audit도 sync이므로 spawn_blocking |
| TaskSupervisor 일반화 | **제외 (YAGNI)** | Guardian=Critical+heartbeat, Web=기존 track_web. trait 추출 + HashMap은 식별된 gap을 해결하지 않음. 두 번째 Restartable이 필요해질 때 별도 RFC |

### Non-Goals

- **Windows 단일 인스턴스**: 정책 결정(support 여부)이지 기술 설계가 아님 → 별도 논의
- **세션 영속**: `SESSION-PERSISTENCE-DESIGN.md`가 이미 존재 → 별도 구현
- **TaskRestarter trait 일반화**: 현재 식별된 gap 중 해결하는 것 없음 → YAGNI

---

## Part A: Guardian Heartbeat Watchdog

### A1. 데이터 구조

`TaskSupervisor`에 필드 1개 + 메서드 1개 추가:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

const GUARDIAN_CHECK_INTERVAL: Duration = Duration::from_secs(60);
/// 3 × Guardian cycle (300s). Three consecutive missed cycles before
/// the watchdog fires — generous enough for slow cycles, tight enough
/// to catch real hangs well before a user notices.
const GUARDIAN_STALE_THRESHOLD_SECS: u64 = 900;

pub struct TaskSupervisor {
    // ... existing fields ...
    /// Guardian heartbeat (Unix epoch seconds). `None` when no Guardian
    /// is running (e.g. CLI subcommands that don't start the daemon).
    guardian_heartbeat: Option<Arc<AtomicU64>>,
}

impl TaskSupervisor {
    /// Register a Guardian heartbeat for watchdog monitoring. The
    /// supervisor checks staleness every 60s via the select! timer
    /// branch. The heartbeat must be updated by the Guardian loop
    /// every cycle; three missed cycles (900s) → process abort.
    pub fn watch_guardian(&mut self, heartbeat: Arc<AtomicU64>) {
        self.guardian_heartbeat = Some(heartbeat);
    }
}
```

### A2. Guardian 루프 재작성

핵심 변경: sync ops를 `spawn_blocking`으로 오프로드 + 사이클 완료 시 heartbeat 갱신.

```rust
const GUARDIAN_INTERVAL: Duration = Duration::from_secs(300);
/// 3 sync ops × ~60s worst-case each. On expiry, the tick is
/// abandoned (detached on the blocking pool) and heartbeat is NOT
/// updated — this is the hang signal the watchdog reads.
const GUARDIAN_TICK_TIMEOUT: Duration = Duration::from_secs(180);

impl Kernel {
    pub fn start_guardian(
        &self,
        web_dist: oxios_gateway::ActiveWebDist,
        heartbeat: Arc<AtomicU64>,
    ) {
        let handle = self.handle();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(GUARDIAN_INTERVAL).await;

                // All four Guardian ops (verify_chain, is_overloaded,
                // git_verify, commit_all) are synchronous. Calling them
                // on an async worker starves the runtime — and
                // tokio::time::timeout cannot preempt a blocking call.
                // spawn_blocking moves them to the blocking pool where
                // the timeout JoinHandle can actually be polled.
                let h = handle.clone();
                let result = tokio::time::timeout(
                    GUARDIAN_TICK_TIMEOUT,
                    tokio::task::spawn_blocking(move || guardian_tick_sync(&h)),
                ).await;

                match result {
                    Ok(Ok(())) | Ok(Err(_)) => {
                        // Cycle completed (success or error from
                        // individual ops) — heartbeat alive.
                        heartbeat.store(now_secs(), Ordering::Relaxed);
                    }
                    Err(_) => {
                        // Tick hung past 180s. Do NOT update heartbeat.
                        // The detached spawn_blocking task continues on
                        // the blocking pool; the loop moves to next
                        // sleep. If hangs persist across cycles,
                        // heartbeat goes stale → watchdog aborts.
                        tracing::warn!(
                            "Guardian tick timed out after {:?} — heartbeat not updated",
                            GUARDIAN_TICK_TIMEOUT
                        );
                    }
                }
            }
        });

        self.start_daily_health_check(web_dist);
    }
}

/// Synchronous Guardian tick — runs on the blocking pool.
/// Extracted from the old inline loop body (kernel.rs:567-602).
fn guardian_tick_sync(handle: &oxios_kernel::KernelHandle) {
    use oxi_sdk::AuditAction;

    if let Ok(valid) = handle.security.verify_chain()
        && !valid
    {
        handle.security.audit(
            "guardian",
            AuditAction::Other { detail: "AUDIT CHAIN BROKEN".into() },
            "guardian",
        );
    }

    if handle.infra.is_overloaded() {
        let snap = handle.infra.resource_snapshot();
        handle.security.audit(
            "guardian",
            AuditAction::Other {
                detail: format!("OVERLOADED: cpu={:.1}%", snap.cpu_percent),
            },
            "guardian",
        );
    }

    if let Ok(valid) = handle.infra.git_verify()
        && !valid
    {
        handle.security.audit(
            "guardian",
            AuditAction::Other { detail: "GIT REPOSITORY CORRUPTED".into() },
            "guardian",
        );
    }

    let _ = handle.commit_all("guardian: periodic checkpoint");
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
```

### A3. Watchdog 분기 (supervisor select!)

`watch()` 루프에 heartbeat 체크 분기 추가. 패턴은 기존 restart_timer와 동일 — `pending`이 없을 때 `pending::<()>()`로 무해한 future를 넣는 것과 같음:

```rust
async fn watch(&mut self) -> ShutdownOutcome {
    loop {
        let restart_timer = self.pending.as_ref()
            .map(|p| tokio::time::sleep_until(p.deadline.into()));

        // Heartbeat timer: sleep when Guardian is registered, pending
        // forever when not — same select!-safety pattern as restart_timer.
        let hb = self.guardian_heartbeat.clone();
        let hb_timer = if hb.is_some() {
            tokio::time::sleep(GUARDIAN_CHECK_INTERVAL)
        } else {
            std::future::pending::<()>()
        };

        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => return ShutdownOutcome::Graceful,
            _ = self.root.cancelled() => return ShutdownOutcome::Graceful,
            Some((tag, result)) = self.tasks.next() => {
                if let Some(outcome) = self.on_completion(tag, result).await {
                    return outcome;
                }
            }
            _ = async {
                match restart_timer {
                    Some(s) => s.await,
                    None => std::future::pending::<()>().await,
                }
            } => {
                if let Some(outcome) = self.fire_web_restart().await {
                    return outcome;
                }
            }
            // ★ Guardian heartbeat watchdog. Fires every 60s when a
            // heartbeat is registered. Only reads an atomic and
            // compares integers — zero kernel calls, survives a
            // wedged kernel.
            _ = hb_timer => {
                if let Some(ref hb) = self.guardian_heartbeat {
                    let last = hb.load(Ordering::Relaxed);
                    let now = now_secs();
                    let stale = now.saturating_sub(last);
                    if stale > GUARDIAN_STALE_THRESHOLD_SECS {
                        tracing::error!(
                            last_seen = last,
                            now = now,
                            stale_secs = stale,
                            threshold = GUARDIAN_STALE_THRESHOLD_SECS,
                            "Guardian heartbeat stale — aborting process"
                        );
                        std::process::abort();
                    }
                }
            }
        }
    }
}
```

### A4. 왜 `process::abort()` 인가

`root.cancel()` + graceful exit가 아니라 `abort()`:

- Guardian이 wedge되었다는 건 kernel 자체가 의심스러운 상태.
- graceful cleanup(agents kill, MCP shutdown, audit flush)은 **정상 kernel을 전제** — wedge된 kernel에서 cleanup을 시도하면 더 오래 hang하거나 손상.
- `abort()` = 즉시 프로세스 사망 → OS supervisor가 known-good 상태로 재시작.
- `panic=abort`와 동일한 경로(L4/L5 협력)를 타므로 일관적.

### A5. cmd_serve 연결

```rust
// cmd_serve (main.rs)
let guardian_heartbeat = Arc::new(AtomicU64::new(now_secs()));
kernel.start_guardian(active_web_dist.clone(), guardian_heartbeat.clone());

let mut supervisor = TaskSupervisor::new(root.clone(), RestartConfig::default());
supervisor.watch_guardian(guardian_heartbeat);      // ★NEW: primary hang defense
supervisor.with_gateway_stop(move || gateway_for_stop.signal_shutdown());
supervisor.track_critical("gateway", gateway_task);

// Guardian task handle — secondary safety net for clean exit
// (not hang; hang is caught by heartbeat watchdog above).
let guardian_task = /* handle from start_guardian */;
supervisor.track_critical("guardian", guardian_task);

for task in channel_tasks {
    supervisor.track_critical("channel", task);  // TODO: per-channel naming
}
```

> **참고:** `start_guardian`은 현재 `JoinHandle`을 반환하지 않음. 반환하도록 변경 필요 —
> Guardian 태스크 핸들을 `track_critical`에 전달하기 위함. 이것이 있으면 Guardian이
> clean exit(panic=abort가 아닌 드문 정상 종료)할 때 supervisor가 Fatal로 감지.
> hang은 heartbeat가 잡고, clean exit은 track_critical이 잡고, panic은 abort가 잡는다 —
> 세 가지 실패 모드, 세 가지 감지 경로.

---

## Part B: Cleanup 통합 + Timeout

### B1. 문제

`cmd_serve` 본체 cleanup (`main.rs:3219–3250`):

```rust
// Phase 2: MCP shutdown — async, 하지만 MCP server가 응답 안 하면 hang
if let Err(e) = handle.mcp.shutdown_all().await { ... }

// Phase 3: audit flush — SYNC (file I/O blocking)
if let Err(e) = kernel.flush_audit() { ... }
```

둘 다 timeout이 없음. hang 시 process가 영원히 안 끝남 → OS supervisor 재시작 안 됨.

### B2. 설계

`Kernel::cleanup` 메서드 추가 — 단일 timeout으로 모든 cleanup 단계를 감싸고, sync 호출(flush_audit)은 spawn_blocking으로 오프로드:

```rust
impl Kernel {
    /// Shutdown kernel resources: agents, MCP, audit. All wrapped in
    /// a single timeout — on expiry, remaining work is abandoned and
    /// the process exits (the OS supervisor restarts to known-good).
    pub async fn cleanup(&self, timeout: Duration) {
        let handle = self.handle();

        let _ = tokio::time::timeout(timeout, async {
            // Phase 1: terminate running agents (parallel kill)
            if let Ok(agents) = handle.agents.list().await
                && !agents.is_empty()
            {
                tracing::info!(count = agents.len(), "Terminating agents...");
                let kills: Vec<_> = agents.iter().map(|a| {
                    let h = handle.clone();
                    let id = a.id.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = h.agents.kill(&id).await {
                            tracing::warn!(agent = %id, error = %e, "Failed to kill agent");
                        }
                    })
                }).collect();
                for k in kills { let _ = k.await; }
                tracing::info!(count = agents.len(), "Agents terminated");
            }

            // Phase 2: MCP shutdown (async, but may hang on unresponsive server)
            if let Err(e) = handle.mcp.shutdown_all().await {
                tracing::warn!(error = %e, "MCP shutdown error");
            }

            // Phase 3: audit flush — SYNC (audit_trail.flush_to).
            // Must offload to blocking pool for the outer timeout to
            // function (same rationale as Guardian's spawn_blocking).
            let kh = handle.clone();
            let _ = tokio::task::spawn_blocking(move || kh.flush_audit()).await;
        }).await;
    }
}
```

`cmd_serve`의 cleanup 섹션은 한 줄로:

```rust
let outcome = supervisor.run().await;
// ... logging ...

let cleanup_timeout = match &outcome {
    ShutdownOutcome::Graceful => Duration::from_secs(3),
    ShutdownOutcome::Fatal { .. } => Duration::from_secs(1),
};
kernel.cleanup(cleanup_timeout).await;

match outcome {
    ShutdownOutcome::Graceful => Ok(()),
    ShutdownOutcome::Fatal { name, reason } => Err(anyhow!(...)),
}
```

### B3. 왜 동일 함수 순차 실행이 race-free인가

`supervisor.run()` 내부의 `drain()`과 `kernel.cleanup()`은 같은 `async fn cmd_serve`의 **순차 문장**. `supervisor.run().await`가 반환된 후 `kernel.cleanup()`이 호출됨 — 동시성 없음. (이전 평가에서 HIGH "race 가능성"으로 오진했으나, 같은 async fn의 순차 실행이므로 race window는 존재하지 않음.)

실제 문제는 **timeout 부재**였고, 본 설계가 해결한다.

---

## Part C: CI + 통합 테스트

### C1. macOS CI job 추가

현재 CI는 `ubuntu-latest` 단일 매트릭스. macOS launchd 코드(`daemon.rs:399–449`, `519–545`, `681–693`, `723–744`)가 **CI에서 한 번도 컴파일된 적 없음**.

```yaml
# .github/workflows/ci.yml — 새 job
test-macos:
  name: test (macOS)
  runs-on: macos-latest
  steps:
    - uses: actions/checkout@v5
    - name: Read rust-toolchain
      id: rust-channel
      run: |
        if grep -q 'channel = "' rust-toolchain.toml 2>/dev/null; then
          CHANNEL=$(grep 'channel = "' rust-toolchain.toml | sed 's/.*= "\([^"]*\)".*/\1/')
          echo "channel=$CHANNEL" >> $GITHUB_OUTPUT
        else
          echo "channel=stable" >> $GITHUB_OUTPUT
        fi
    - uses: dtolnay/rust-toolchain@master
      with:
        toolchain: ${{ steps.rust-channel.outputs.channel }}
    - uses: Swatinem/rust-cache@v2
    - name: Test
      run: cargo test --workspace
```

### C2. 통합 테스트: `tests/supervision.rs`

감시 계층의 **실제 동작**을 검증 (단위 테스트가 아닌):

```rust
//! Supervision layer integration tests.
//!
//! Verifies the runtime behavior that unit tests can't: task crashes,
// restart loops, drain timeouts, heartbeat staleness.

// ── Heartbeat watchdog ─────────────────────────────────────────

/// Stale heartbeat past threshold → abort decision logic.
/// (Directly tests the AtomicU64 comparison without spawning.)
#[test]
fn heartbeat_stale_past_threshold() {
    let hb = Arc::new(AtomicU64::new(0)); // epoch = very stale
    let now = now_secs();
    let stale = now.saturating_sub(hb.load(Ordering::Relaxed));
    assert!(stale > GUARDIAN_STALE_THRESHOLD_SECS);
}

/// Fresh heartbeat under threshold → no abort.
#[test]
fn heartbeat_fresh_under_threshold() {
    let hb = Arc::new(AtomicU64::new(now_secs()));
    let now = now_secs();
    let stale = now.saturating_sub(hb.load(Ordering::Relaxed));
    assert!(stale <= GUARDIAN_STALE_THRESHOLD_SECS);
}

// ── spawn_blocking timeout behavior ────────────────────────────

/// Guardian tick timeout: heartbeat NOT updated when spawn_blocking
/// exceeds 180s. Uses a mock slow sync function.
#[tokio::test]
async fn guardian_timeout_skips_heartbeat() {
    let hb = Arc::new(AtomicU64::new(now_secs()));
    let hb_clone = hb.clone();

    let result = tokio::time::timeout(
        Duration::from_millis(100),
        tokio::task::spawn_blocking(move || {
            std::thread::sleep(Duration::from_secs(1)); // exceed timeout
        }),
    ).await;

    assert!(result.is_err(), "timeout should fire");
    // Simulate: on timeout, don't update heartbeat
    // (In real code, this is the `Err(_) =>` branch)
    let stale = now_secs().saturating_sub(hb_clone.load(Ordering::Relaxed));
    assert!(stale < GUARDIAN_STALE_THRESHOLD_SECS, // still fresh — only one miss
            "single timeout should not immediately trigger stale");
}

// ── TaskSupervisor behavior ────────────────────────────────────

/// Critical task exit → Fatal outcome.
#[tokio::test]
async fn critical_task_exit_is_fatal() {
    let root = CancellationToken::new();
    let mut sup = TaskSupervisor::new(root.clone(), RestartConfig::default());
    sup.track_critical("test-critical", tokio::spawn(async {}));
    let outcome = sup.run().await;
    assert!(matches!(outcome, ShutdownOutcome::Fatal { name, .. } if name == "test-critical"));
}

/// ctrl_c (simulated as root.cancel()) → Graceful.
#[tokio::test]
async fn root_cancel_is_graceful() {
    let root = CancellationToken::new();
    let mut sup = TaskSupervisor::new(root.clone(), RestartConfig::default());
    let task_token = root.clone();
    sup.track_critical("alive", tokio::spawn(async move {
        task_token.cancelled().await;
    }));
    root.cancel();
    let outcome = sup.run().await;
    assert!(matches!(outcome, ShutdownOutcome::Graceful));
}

/// Drain timeout: tasks that don't finish are detached.
#[tokio::test]
async fn drain_detaches_unfinished_tasks() {
    let root = CancellationToken::new();
    let mut sup = TaskSupervisor::new(root.clone(), RestartConfig::default());
    // Task that never completes (infinite loop without cancel check).
    sup.track_critical("stuck", tokio::spawn(async {
        std::future::pending::<()>().await;
    }));
    root.cancel();
    // run() will drain with timeout — should not hang.
    let start = Instant::now();
    let _ = sup.run().await;
    assert!(start.elapsed() < Duration::from_secs(15),
            "drain should timeout, not hang");
}
```

### C3. 통합 테스트: `tests/daemon_lifecycle.rs`

데몬 라이프사이클 (Unix):

```rust
//! Daemon lifecycle integration tests.

use oxios_kernel::{DaemonManager, DaemonStatus};

// ── PID file lifecycle ─────────────────────────────────────────

/// Stale pidfile (dead PID) → Stale status → cleanup → Stopped.
#[test]
fn stale_pidfile_cleaned_on_start() {
    let tmp = tempfile::tempdir().unwrap();
    let pid_file = tmp.path().join("oxios.pid");
    std::fs::write(&pid_file, "999999").unwrap(); // nonexistent PID
    let dm = DaemonManager::new(pid_file.to_str().unwrap(), "/tmp");
    assert!(matches!(dm.status(), DaemonStatus::Stale { .. }));
    dm.cleanup().unwrap();
    assert!(matches!(dm.status(), DaemonStatus::Stopped));
}

/// Fresh pidfile (current PID) → Running status.
#[test]
fn fresh_pidfile_reports_running() {
    let tmp = tempfile::tempdir().unwrap();
    let pid_file = tmp.path().join("oxios.pid");
    std::fs::write(&pid_file, std::process::id().to_string()).unwrap();
    let dm = DaemonManager::new(pid_file.to_str().unwrap(), "/tmp");
    assert!(matches!(dm.status(), DaemonStatus::Running { .. }));
    dm.cleanup().unwrap();
}

// ── Orphan detection (Unix) ────────────────────────────────────

/// Port probe detects a listener when no pidfile exists.
#[cfg(unix)]
#[test]
fn orphan_detection_finds_listener() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let dm = DaemonManager::new("/tmp/oxios-orphan-test.pid", "/tmp")
        .with_probe_port(port);
    assert!(matches!(dm.status(), DaemonStatus::Orphaned { port: p } if p == port));
}

// ── SIGTERM → SIGKILL escalation (Unix) ────────────────────────

/// Process that ignores SIGTERM gets SIGKILL within SIGTERM_GRACE + slack.
#[cfg(unix)]
#[tokio::test]
async fn kill_pid_escalates_to_sigkill() {
    use std::process::Command;
    // Spawn a child that ignores SIGTERM
    let child = Command::new("perl")
        .args(["-e", "$SIG{TERM}=sub{}; sleep(30);"])
        .spawn()
        .or_else(|_| Command::new("bash")
            .args(["-c", "trap '' TERM; sleep 30"])
            .spawn())
        .expect("need perl or bash");
    let pid = child.id();
    let dm = DaemonManager::new("/tmp/oxios-killtest.pid", "/tmp");
    let start = std::time::Instant::now();
    // kill_pid sends SIGTERM → waits SIGTERM_GRACE(5s) → SIGKILL
    dm.kill_pid_for_test(pid).ok();
    let elapsed = start.elapsed();
    assert!(elapsed >= std::time::Duration::from_secs(5),
            "should wait SIGTERM_GRACE before SIGKILL");
    assert!(elapsed < std::time::Duration::from_secs(8),
            "SIGKILL should finish quickly after grace");
    // Process should be dead
    assert!(!dm.is_alive_for_test(pid));
}
```

> **참고:** `kill_pid_for_test` / `is_alive_for_test`는 기존 private 메서드를 테스트용
> 노출(`#[cfg(test)] pub(crate)` 또는 별도 테스트 헬퍼). 또는 `kill_pid` 자체를
> `pub(crate)`로 전환.

---

## Part D: Thrashing 방지 + DST 안전화

### D1. systemd unit에 start limit 추가

`daemon.rs:483–498`:

```ini
[Unit]
Description=Oxios Agent Operating System
After=network.target
StartLimitBurst=5
StartLimitIntervalSec=60

[Service]
Type=simple
ExecStart={exe} --foreground
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

연속 5회 실패 / 60s → systemd가 재시작 중단. 관리자 개입 필요.
(기본 start limit이 있지만 명시적 설정이 더 안전 — systemd 버전 간 default 차이 회피.)

### D2. launchd plist에 throttle 추가

`daemon.rs:410–434`:

```xml
<key>KeepAlive</key>
<dict>
    <key>SuccessfulExit</key>
    <false/>
</dict>
<key>ThrottleInterval</key>
<integer>10</integer>
```

- `SuccessfulExit=false`: clean exit(0) 시 재시작 안 함 → `oxios stop`이 exit 0으로 끝나도 launchd가 재시작 안 함.
- `ThrottleInterval=10`: crash 시 최소 10s 대기 (launchd default는 10s이지만 명시적 설정이 안전).

> **주의:** 현재 `KeepAlive=true`는 clean exit도 재시간. `SuccessfulExit=false`로
> 변경하면 `oxios stop`(graceful → exit 0) 후 재시작을 막을 수 있음. 단 SIGKILL
> 에스컬레이션(non-zero exit)은 여전히 재시작하므로 `stop()`의 bootout은 유지.

### D3. DST schedule 계산 안전화

`kernel.rs:617–622`:

```rust
// Before:
let mut next = now.date_naive()
    .and_hms_opt(3, 0, 0).unwrap()
    .and_local_timezone(chrono::Local)
    .unwrap();  // ← DST skip 시간(3:00이 존재하지 않는 시간대)에서 panic

// After:
let mut next = now.date_naive()
    .and_hms_opt(3, 0, 0).unwrap()
    .and_local_timezone(chrono::Local)
    .earliest()  // Ambiguous → earliest; None → None
    .unwrap_or(now + chrono::Duration::hours(24));  // skip 시간 → 24h 후로 폴백
```

`and_hms_opt(3,0,0).unwrap()`은 range check(3,0,0은 항상 유효)이므로 panic 불가.
DST 취약점은 `and_local_timezone().unwrap()`만 — 주요 시간대(US/EU)는 2:00 skip이라 3:00는 안전하지만, 일부 시간대에서는 발생 가능.

---

## Implementation Order

```
Part D (thrashing + DST)  ──→  즉시 안전망 (10줄 변경, 독립적)
         │
         ↓
Part A (Guardian heartbeat)  ──→  핵심 HIGH 수정
         │
         ↓
Part B (cleanup timeout)  ──→  A 이후, Kernel 메서드
         │
         ↓
Part C (CI + tests)  ──→  A/B 완료 후 검증
```

**권장 순서:** D → A → B → C. D는 독립적이고 즉시 효과. A가 핵심. B는 A의 패턴(spawn_blocking)을 공유. C는 모든 구현 완료 후 검증.

---

## Testing Strategy

| Test | Type | File | Covers |
|------|------|------|--------|
| Heartbeat stale logic | Unit | `tests/supervision.rs` | AtomicU64 threshold 계산 |
| spawn_blocking timeout skips heartbeat | Integration | `tests/supervision.rs` | sync op timeout → heartbeat 미갱신 |
| Critical task exit → Fatal | Integration | `tests/supervision.rs` | supervisor fail-fast |
| root.cancel() → Graceful | Integration | `tests/supervision.rs` | supervisor graceful |
| Drain timeout detaches | Integration | `tests/supervision.rs` | drain hang 방지 |
| Stale pidfile → cleanup | Unit | `tests/daemon_lifecycle.rs` | DaemonManager status |
| Orphan detection | Unit (Unix) | `tests/daemon_lifecycle.rs` | port probe + comm |
| SIGTERM → SIGKILL | Integration (Unix) | `tests/daemon_lifecycle.rs` | kill escalation |
| launchd plist valid XML | Unit (macOS) | `tests/daemon_lifecycle.rs` | `#[cfg(target_os="macos")]` |
| macOS CI compile | CI | `.github/workflows/ci.yml` | launchd 코드 첫 CI 컴파일 |

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Watchdog false positive (slow Guardian cycle ≠ hang) | 불필요한 process abort | threshold 900s (3× interval)로 여유. 정상 cycle이 900s를 넘을 수 없음 (timeout 180s + sleep 300s = 480s max) |
| spawn_blocking detached task accumulates | blocking pool 소진 | abort → process 재시작으로 전체 정리. 정상 cycle에서는 detached task 없음 |
| macOS CI runner 비용 | 빌드 시간 증가 | rust-cache 사용. job 분리로 병렬화 |
| `SuccessfulExit=false` 변경이 기존 동작에 영향 | launchd가 clean exit를 재시작 안 함 | 이것이 의도 (`oxios stop` 후 재시작 방지). SIGKILL(non-zero)은 여전히 재시작 |
| heartbeat 타이머가 select!에 분기 추가 | watch() 복잡도 | 패턴은 기존 restart_timer와 동일. `pending::<()>()` fallback으로 Guardian 미등록 시 무해 |
