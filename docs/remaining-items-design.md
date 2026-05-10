# 미구현 항목 설계서

> 공학적으로 아름다운 소프트웨어로의 길 — 남은 3개 항목

---

## 항목 1: Channel → Stream 전환

### 현재 문제

Gateway가 50ms 간격으로 모든 채널을 폴링한다. Channel trait이 `async fn receive(&self)` 만 제공해서
이벤트 드리븐이 불가능하다.

```rust
// 현재 Channel trait
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&self) -> Result<Option<IncomingMessage>>;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}
```

### 근본 원인

`receive(&self)`는 `&self`로 동작한다. `self`가 `Box<dyn Channel>`으로 Gateway에 저장되어 있고,
이 참조를 `tokio::spawn`으로 보낼 수 없다 (`'static` 필요).

### 해결: Channel 내부에 mpsc receiver를 노출하는 방식

Channel trait을 Stream으로 바꾸는 대신, **각 Channel 구현체가 자신의 incoming mpsc::Receiver를
Gateway에 전달**하는 방식.

```
현재: Gateway가 채널에 물어봄 (polling)
  Gateway → channel.receive()? → 메시지 있으면 처리

변경: Channel이 Gateway에게 알려줌 (push)
  Channel.incoming_rx() → mpsc::Receiver<IncomingMessage>
  Gateway → tokio::select! { 모든 receiver에서 동시 대기 }
```

### 설계

#### 1. Channel trait에 `into_receiver()` 추가

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;

    /// 이 채널의 incoming 메시지 수신기를 반환.
    ///
    /// Gateway는 이 receiver를 select!에 등록하여 이벤트 드리븐으로 동작.
    /// 이 메서드는 한 번만 호출 가능 (receiver를 consume).
    fn into_receiver(self: Box<Self>) -> tokio::sync::mpsc::Receiver<IncomingMessage>;

    /// Outgoing 메시지 전송.
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}
```

**문제**: `into_receiver(self: Box<Self>)`는 `async_trait`과 호환되지 않음.
`self: Box<Self>`는 trait에서 non-object-safe일 수 있음.

#### 2. 더 실용적 대안: Gateway가 채널당 spawn

```rust
// Gateway 변경
pub struct Gateway {
    channels: RwLock<HashMap<String, Box<dyn Channel>>>,
    orchestrator: Arc<oxios_kernel::Orchestrator>,
    incoming: tokio::sync::mpsc::Sender<IncomingMessage>,
    incoming_rx: Mutex<tokio::sync::mpsc::Receiver<IncomingMessage>>,
}

impl Gateway {
    pub async fn register(&self, channel: Box<dyn Channel>) {
        let name = channel.name().to_owned();
        
        // 폴링 태스크를 spawn
        let tx = self.incoming.clone();
        tokio::spawn(async move {
            loop {
                match channel.receive().await {
                    Ok(Some(msg)) => {
                        if tx.send(msg).await.is_err() {
                            break; // Gateway shut down
                        }
                    }
                    Ok(None) => {
                        // Channel closed
                        tracing::info!(channel = %name, "Channel closed");
                        break;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Channel receive error");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
        
        // ...
    }

    pub async fn run(&self) -> Result<()> {
        let mut rx = self.incoming_rx.lock().await;
        loop {
            match rx.recv().await {
                Some(msg) => {
                    if let Err(e) = self.route(msg).await {
                        tracing::error!(error = %e, "Failed to route message");
                    }
                }
                None => {
                    // All senders dropped
                    tracing::info!("All channels closed");
                    break;
                }
            }
        }
        Ok(())
    }
}
```

**문제**: `channel.receive()`가 `&self`이고, `Box<dyn Channel>`을 spawn으로 보내려면
`Send + 'static`이어야 함. 그런데 `Channel`은 이미 `Send + Sync`이므로 가능!
단, `&self`가 아닌 `self`를 소유해야 함.

#### 3. 최종 설계: register에서 channel 소유권 이전

```rust
pub async fn register(&self, name: String, channel: Box<dyn Channel>) {
    let tx = self.incoming.clone();
    let ch_name = name.clone();
    
    // channel의 send()를 나중에 쓰기 위해 Arc로 보관
    // 하지만 Channel은 trait object라 Arc<Box<dyn Channel>> 필요
    
    // 문제: send()는 &self인데, receive()도 &self임.
    // spawn이 channel을 move하면 send()를 못 함.
}
```

**핵심 딜레마**: `receive()`와 `send()`가 모두 `&self`로 동작.
Channel 하나를 두 태스크에서 공유해야 함 → `Arc<Mutex<dyn Channel>>` 필요.

Mutex 안에서 receive()를 부르면 다른 태스크의 send()가 블록됨.

#### 4. 진짜 최종 설계: Channel을 두 부분으로 분리

```rust
/// 채널 수신부 — Gateway가 소유.
pub trait ChannelReceiver: Send + Sync {
    /// 메시지 수신. None이면 채널 종료.
    async fn receive(&mut self) -> Result<Option<IncomingMessage>>;
}

/// 채널 송신부 — Gateway가 route 시 사용.
pub trait ChannelSender: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, msg: OutgoingMessage) -> Result<()>;
}

/// 채널 분리.
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    fn split(self: Box<Self>) -> (Box<dyn ChannelReceiver>, Box<dyn ChannelSender>);
}
```

이렇게 하면:
- `ChannelReceiver`는 spawn된 태스크가 소유 (`&mut self` → 단독 접근)
- `ChannelSender`는 Gateway가 보관 (`&self` → 공유 가능)

**하지만**: 기존 세 채널(Web, CLI, Telegram) 모두 재작성 필요.
`ChannelReceiver`가 `&mut self`를 받으므로 async_trait이 필요하고,
이건 object-safe함.

### 결정

이 설계는 올바르지만 **영향 범위가 크다**:

| 파일 | 변경 |
|------|------|
| `gateway/src/channel.rs` | trait을 3개로 분리 |
| `gateway/src/gateway.rs` | `run()` 재작성 |
| `oxios-web/src/channel.rs` | `split()` 구현 |
| `oxios-cli/src/channel.rs` | `split()` 구현 |
| `oxios-telegram/src/lib.rs` | `split()` 구현 |
| `gateway/src/plugin.rs` | `ChannelBundle` 변경 |

**리스크**: async_trait + object safety + `&mut self` 조합이 까다로움.
기존 50ms adaptive drain은 실제 부하에서 충분히 잘 동작.

**추천**: 이 항목은 **0.3.0 마일스톤**으로 미룸.
현재 폴링은 CPU 0%에 가까운 오버헤드이고, 50ms 이하 응답성은 채팅/대시보드에 충분.
채널이 5개 이상이 되거나 latency-critical한 채널(음성, 실시간 스트리밍)이 추가되면 그때 진행.

---

## 항목 2: ExecTool 접근 제어 활성화

### 현재 문제

```rust
pub struct ExecTool {
    config: Arc<ExecConfig>,
    access: Arc<Mutex<AccessManager>>,  // ← 저장만 되고 사용 안 함
}
```

`access` 필드가 생성자에서 저장만 되고, `shell_exec()` / `structured_exec()`에서
한 번도 참조되지 않는다. ExecTool을 통과하는 모든 명령이 접근 제어 없이 실행된다.

### 설계

ExecTool이 실행 전에 AccessManager를 통해 에이전트의 권한을 확인.

#### 문제: ExecTool이 agent context를 모름

`AgentTool::execute()` 시그니처:

```rust
async fn execute(
    &self,
    _tool_call_id: &str,
    params: Value,          // ← JSON 파라미터만 받음
    _signal: Option<oneshot::Receiver<()>>,
) -> Result<AgentToolResult, String>;
```

agent_id, workspace 등의 컨텍스트가 없다.
oxi-agent의 AgentTool trait은 agent context를 전달하지 않는다.

#### 해결: ExecConfig에 컨텍스트 주입

ExecTool 생성 시 에이전트 컨텍스트를 설정하고, 실행 시 검증.

```rust
pub struct ExecTool {
    config: Arc<ExecConfig>,
    access: Arc<Mutex<AccessManager>>,
    /// 현재 에이전트 컨텍스트 (agent_runtime에서 설정)
    agent_context: Arc<RwLock<Option<AgentContext>>>,
}

/// 에이전트 실행 컨텍스트.
pub struct AgentContext {
    pub agent_name: String,
    pub workspace: Option<String>,
    pub allowed_tools: Vec<String>,
}
```

AgentRuntime에서 ExecTool을 생성할 때 컨텍스트를 주입:

```rust
// agent_runtime.rs
let exec_tool = Arc::new(ExecTool::new(exec_config, exec_access));
// fork 시점에 컨텍스트 설정
exec_tool.set_context(AgentContext {
    agent_name: agent_name.clone(),
    workspace: Some(workspace.clone()),
    allowed_tools: permissions.allowed_tools.clone(),
});
```

#### 검증 로직

```rust
impl ExecTool {
    pub async fn shell_exec(&self, command: &str, timeout_ms: u64) -> Result<ExecResult, String> {
        // 1. 컨텍스트 확인
        let ctx = self.agent_context.read().await.clone();
        if let Some(ref ctx) = ctx {
            // 2. 접근 제어 검증
            let mut access = self.access.lock();
            
            // 2a. 도구 권한 확인
            if !ctx.allowed_tools.contains(&"bash".to_string()) {
                return Err("shell_exec: agent does not have 'bash' permission".to_string());
            }
            
            // 2b. 워크스페이스 경로 검증 (명령어에서 경로 추출)
            if let Some(ref workspace) = ctx.workspace {
                // command에서 경로를 완벽히 추출하는 건 불가능 (arbitrary shell).
                // 대신 audit 로깅으로 추적
                tracing::warn!(
                    agent = %ctx.agent_name,
                    command = %command.chars().take(200).collect::<String>(),
                    workspace = %workspace,
                    "ExecTool: shell command executed in workspace context"
                );
            }
        }
        
        // ... 기존 실행 로직 ...
    }
    
    pub async fn structured_exec(&self, binary: &str, args: Vec<String>, timeout_ms: u64) -> Result<ExecResult, String> {
        // structured 모드는 binary가 명확하므로 더 강력한 검증 가능
        let ctx = self.agent_context.read().await.clone();
        if let Some(ref ctx) = ctx {
            let mut access = self.access.lock();
            
            // binary가 allowlist에 있는지 (기존 config.is_binary_allowed보다 더 엄격하게)
            if !access.can_use_tool(&ctx.agent_name, binary) {
                return Err(format!(
                    "structured_exec: agent '{}' cannot execute '{}'",
                    ctx.agent_name, binary
                ));
            }
            
            // 경로 순회 방지 (args에서 .. 검사)
            for arg in &args {
                if arg.contains("..") {
                    return Err(format!(
                        "structured_exec: path traversal detected in argument '{}'",
                        arg
                    ));
                }
            }
        }
        
        // ... 기존 실행 로직 ...
    }
}
```

### 주의사항

- **shell_exec는 임의 셸 명령이므로 완벽한 샌드박싱이 불가능**. 
  대신 audit 로깅으로 추적 (사후 검증).
- **structured_exec는 binary가 명확하므로 사전 차단 가능**.
- AgentTool trait이 agent_id를 전달하지 않으므로, ExecTool에 별도 컨텍스트 주입이 필요.

### 구현 계획

| 단계 | 내용 |
|------|------|
| 1 | `AgentContext` 구조체 정의 |
| 2 | ExecTool에 `agent_context` 필드 추가 |
| 3 | `set_context()` 메서드 추가 |
| 4 | `structured_exec`에 binary 권한 검증 추가 |
| 5 | `shell_exec`에 audit 로깅 추가 |
| 6 | AgentRuntime에서 fork 시 context 주입 |
| 7 | 테스트 작성 |

---

## 항목 3: can_access_path_in_workspace agent_id 버그 수정

### 현재 문제

```rust
pub fn can_access_path_in_workspace(
    &mut self,
    agent_name: &str,       // ← 에이전트 이름을 받음
    path: &str,
    workspace: Option<&str>,
) -> bool {
    let subject = Subject::Agent(AgentId::new_v4()); // ← 무작위 UUID 생성!
    // ...
}
```

`agent_name`을 받아놓고, RBAC 검사에서는 `AgentId::new_v4()`로 무작위 UUID를 생성한다.
이러면 RBAC 규칙이 절대 매치되지 않는다 (매번 다른 ID이므로).

### 설계

#### 문제 분석

이 메서드의 호출자를 찾아야 함:

```bash
grep -rn "can_access_path_in_workspace" crates/
```

현재 이 메서드를 호출하는 곳이 있는지 확인 필요.
ExecTool이 사용하지 않으므로, 현재는 사실상 dead code일 가능성이 높음.

#### 해결: agent_name을 그대로 사용

```rust
pub fn can_access_path_in_workspace(
    &mut self,
    agent_name: &str,
    path: &str,
    workspace: Option<&str>,
) -> bool {
    // agent_name을 사용하여 Subject 생성.
    // agent_name은 고유 식별자로 사용됨.
    let agent_id = AgentId::parse_str(agent_name)
        .unwrap_or_else(|_| AgentId::new_v4());
    let subject = Subject::Agent(agent_id);
    let action = Action::AccessPath(path.to_string());
    let rbac_allowed = self.rbac.check_permission(&subject, &action, path);
    // ...
}
```

하지만 `agent_name`이 항상 UUID 형식은 아님 ("code-agent" 같은 이름도 가능).
Subject::Agent가 UUID를 요구하므로, 두 가지 접근이 있음:

**접근 A**: `Subject`에 name variant 추가

```rust
pub enum Subject {
    User(String),
    Agent(AgentId),
    AgentNamed(String),  // ← 추가
    System,
}
```

**접근 B**: agent_name → 등록된 agent_id 조회

```rust
// AccessManager에 agent name → id 맵 추가
agent_name_to_id: HashMap<String, AgentId>,
```

**접근 C (권장)**: 시그니처를 AgentId를 받도록 변경

```rust
pub fn can_access_path_in_workspace(
    &mut self,
    agent_id: &AgentId,        // ← AgentId 직접 받음
    path: &str,
    workspace: Option<&str>,
) -> bool {
    let subject = Subject::Agent(*agent_id);
    // ...
}
```

호출자가 AgentId를 가지고 있으므로 (Supervisor가 fork 시 생성),
이쪽에서 변환할 필요가 없음.

### 구현 계획

| 단계 | 내용 |
|------|------|
| 1 | `can_access_path_in_workspace` 시그니처를 `agent_id: &AgentId`로 변경 |
| 2 | `Subject::Agent(*agent_id)` 사용 |
| 3 | 기존 `agent_name` 기반 검색이 필요한 경우 별도 메서드 유지 |
| 4 | 호출자(있다면) 업데이트 |
| 5 | 테스트 업데이트 |

---

## 우선순위

| 항목 | 위험도 | 난이도 | 추천 시기 |
|------|--------|--------|----------|
| **3. agent_id 버그** | 🔴 보안 | 🟢 쉬움 | **지금** |
| **2. ExecTool 접근 제어** | 🟡 보안 | 🟡 중간 | 0.2.0 |
| **1. Channel Stream 전환** | 🟢 성능 | 🔴 어려움 | 0.3.0 |

항목 3은 간단하고 보안 수정이므로 즉시 진행. 항목 2는 설계를 따로 리뷰 후 진행.
항목 1은 아키텍처 변경이 크므로 마일스톤으로 미룸.
