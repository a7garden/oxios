# 미구현 항목 설계서

> 공학적으로 아름다운 소프트웨어로의 길 — 남은 3개 항목
>
> v2 — 리뷰 피드백 반영 (2025-05-10)

---

## 항목 간 의존관계

```
항목 3 (agent_id 버그) ──✅ 완료
       ↓ (선행 조건)
항목 2a (ExecTool 프로덕션 연결) — ✅ 완료
       ↓
항목 2b (ExecTool 접근 제어) — ✅ 완료
       ↓ (독립)
항목 1 (Channel Stream) ← 0.3.0
```

---

## 항목 1: Channel → Stream 전환 (0.3.0)

### 현재 상태

Gateway가 adaptive drain 폴링으로 동작. CPU 오버헤드 ≈ 0%, 50ms 이하 응답성.
현재 아키텍처로 충분함.

### 현재 Channel trait

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&self) -> Result<Option<IncomingMessage>>;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}
```

### 근본 원인

Gateway가 `RwLock<HashMap<String, Box<dyn Channel>>>`에 채널을 저장하고,
`run()`에서 read lock을 잡은 채 `receive()`를 호출.
`receive()`와 `send()`가 모두 `&self`이므로, 하나의 채널을 receive용 태스크와
send용 태스크로 분리하려면 소유권 분할이 필요.

### A경로: 최소 변경 — `take_receiver()` (권장)

Web, CLI 채널은 이미 내부에 `mpsc::Receiver<IncomingMessage>`를 가지고 있다.
Telegram만 예외 (HTTP long polling으로 직접 수신).

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&self) -> Result<Option<IncomingMessage>>;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;

    /// 내부 mpsc::Receiver를 반환 (있는 경우).
    /// 기본 구현: None → Gateway가 기존 폴링으로 처리.
    /// Web, CLI는 재정의하여 receiver 반환.
    fn take_receiver(&self) -> Option<tokio::sync::mpsc::Receiver<IncomingMessage>> {
        None
    }
}
```

**이점**:
- trait 변경 최소 (선택적 메서드 1개 추가, 기본 `None`)
- Web, CLI는 `Mutex<Option<Receiver>>`에서 `take()`로 반환
- Telegram은 `None` 반환 → 기존 폴링 유지
- Gateway::run()만 `select!` 기반으로 전환

**Gateway 변경**:

```rust
pub async fn run(&self) -> Result<()> {
    // Phase 1: receiver가 있는 채널은 select!에 등록
    // Phase 2: receiver가 없는 채널(Telegram)은 기존 폴링 유지
    loop {
        tokio::select! {
            // receiver 보유 채널 (Web, CLI)
            msg = self.incoming_rx.recv() => { ... }
            // receiver 없는 채널 (Telegram) — 타이머 기반 폴링
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                // 기존 drain 로직
            }
        }
    }
}
```

### B경로: trait 분리 ( Receiver / Sender )

A경로로 충분하지 않게 되면 (채널 5개+, latency-critical), trait을 분리:

```rust
pub trait ChannelReceiver: Send + Sync {
    async fn receive(&mut self) -> Result<Option<IncomingMessage>>;
}

pub trait ChannelSender: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}

pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    fn split(self: Box<Self>)
        -> (Box<dyn ChannelReceiver>, Box<dyn ChannelSender>);
}
```

영향 범위: `channel.rs`, `gateway.rs`, Web/CLI/Telegram 전부 재작성.

### 선행 작업: Telegram CancellationToken

Telegram의 `receive()`가 내부에 `loop { poll_updates() }` 무한 루프를 돈다.
spawn으로 분리하려면 graceful shutdown이 불가능하므로, 먼저 CancellationToken 추가 필요.

```rust
pub struct TelegramChannel {
    // ... 기존 필드
    cancel: tokio_util::sync::CancellationToken,  // ← 추가
}
```

### 구현 계획 (0.3.0)

| 단계 | 내용 | 경로 |
|------|------|------|
| 0 | Telegram에 CancellationToken 추가 | 공통 |
| 1 | `Channel::take_receiver()` 추가 | A |
| 2 | Web, CLI에 `take_receiver()` 구현 | A |
| 3 | Gateway::run()을 select! 기반으로 전환 | A |
| 4 | 필요시 B경로로 확장 | B |

---

## 항목 2: ExecTool 접근 제어 — ✅ 완료

이전에는 ExecTool이 프로덕션에 연결되어 있지 않았음 (`with_exec_tool()` 미호출).
ProgramTool도 ExecTool에 의존하므로 프로그램 도구도 동작하지 않았음.

## 항목 2a: ExecTool 프로덕션 연결 — ✅ 완료

kernel.rs에서 ExecConfig + AccessManager를 AgentRuntime에 전달.
AgentRuntime은 실행 시점에 `ExecTool::for_agent()`로 에이전트별 인스턴스를 생성.

## 항목 2b: ExecTool 접근 제어 — ✅ 완료

- `ExecTool::for_agent()`: agent_name을 생성 시점에 고정
- `structured_exec`: `can_use_tool(name, binary)` 사전 권한 검증
- `shell_exec`: agent_name 기반 audit 로깅
- Mutex 타입을 `parking_lot::Mutex`로 통일

---

## 항목 3: can_access_path_in_workspace agent_id 버그 — ✅ 완료

### 수정 내용

시그니처를 `AgentId` 직접 수신으로 변경:

```rust
// 이전 (버그)
pub fn can_access_path_in_workspace(
    &mut self,
    agent_name: &str,       // ← 이름은 받지만
    path: &str,
    workspace: Option<&str>,
) -> bool {
    let subject = Subject::Agent(AgentId::new_v4()); // ← 매번 무작위 UUID!
}

// 이후 (수정)
pub fn can_access_path_in_workspace(
    &mut self,
    agent_id: &AgentId,     // ← 실제 AgentId
    agent_name: &str,       // ← path/workspace 검색용
    path: &str,
    workspace: Option<&str>,
) -> bool {
    let subject = Subject::Agent(*agent_id);  // ← 올바른 RBAC 검증
}
```

### 현재 상태

- **구현 완료** (commit `3c67c1a`)
- **호출자 없음** (dead code): 현재 이 메서드를 호출하는 곳이 없음.
  항목 2b에서 ExecTool이 `can_use_tool()`과 함께 이 메서드를 호출하게 되면
  첫 호출자가 생김.
- `agent_id`: RBAC `Subject` 생성용
- `agent_name`: path 권한(`can_access_path`) 및 workspace 조회(`agent_workspaces`)용
  — 서로 다른 레이어에서 사용되므로 두 파라미터 모두 필요

---

## 우선순위

| 항목 | 상태 | 위험도 | 난이도 | 시기 |
|------|------|--------|--------|------|
| **3. agent_id 버그** | ✅ 완료 | 🔴 보안 | 🟢 | 완료 |
| **2a. ExecTool 연결** | ✅ 완료 | 🟡 기능 | 🟢 | 완료 |
| **2b. 접근 제어** | ✅ 완료 | 🟡 보안 | 🟡 | 완료 |
| **1. Channel Stream** | 📋 예정 | 🟢 성능 | 🔴 | 0.3.0 |
