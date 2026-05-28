# RFC-015: 보안 모델 통합

> **상태:** 📝 설계 (v2 — 구조 재설계)
> **날짜:** 2026-05-27
> **우선순위:** P0-P1
> **범위:** `access_manager/`, `capability/`, `tools/exec_tool.rs`, `tools/registration.rs`, `audit_trail.rs`
> **선행:** 없음
> **후행:** RFC-018 (Configuration UX)

---

## 1. 동기

### 1.1 현황

현재 **4개**의 독립적인 보안 계층이 서로 다른 시점에 체크되며,
각각 자체 audit log를 유지하고 서로를 인식하지 못한다:

```
Layer 0: CSpace (Capability)       — registration.rs가 도구 등록 여부 결정
         ↓ 등록된 도구만 실행 가능
Layer 1: RBAC (RbacManager)        — Subject + Role + Action, HitL 승인
         ↓ Role이 허용하는 Action만 통과
Layer 2: Agent Permissions          — 도구/경로/네트워크/fork/시간/메모리
         ↓ AgentPermissions의 allowed_tools/denied_paths만 통과
Layer 3: ExecConfig                 — 바이너리 허용목록, 셸 모드, 타임아웃
         ↓ 허용된 바이너리만 실행
```

하지만 이 위계가 **코드에서 명시적**으로 드러나지 않는다.
계층 간 호출이 분산되어 있고, 어떤 계층이 어디서 호출되는지 추적이 어렵다.

### 1.2 발견된 간격

| # | 간격 | 원인 | 심각도 |
|---|------|------|--------|
| G1 | `ExecTool::new()` (agent_name=None)가 권한 체크 완전 우회 | 타입 레벨 보장 부재 | 🔴 |
| G2 | ExecTool은 Agent Permissions만 체크, RBAC 무시 | 계층별 분산 체크 | 🔴 |
| G3 | Always-on 도구(read/write/edit/grep/find/ls)가 AccessManager 통과 안함 | CSpace가 도구 등록만 결정, 실행 시 권한 체크 없음 | 🟡 |
| G4 | RBAC audit log가 `RbacManager.audit_log: Vec<_>`에만 존재 (비영속) | AuditTrail과 분리 | 🟡 |
| G5 | 기본 `allowed_commands`에 `osascript`, `open` 포함 | default-config.toml | 🟡 |
| G6 | `AccessManager.audit_log: Vec<AuditEntry>`와 `AuditTrail`이 이중 유지 | 감사 로그 삼중 분열 | 🟡 |
| G7 | `RbacManager`가 `AccessManager` 내부에 숨겨져 별도 참조 불가 | 구조적 응집도 과다 | 🟢 |
| G8 | HitL 승인(ApprovalRequested)이 EventBus에 emit되지만, 권한 게이트와 연결 안 됨 | 이벤트 흐름 단절 | 🟡 |

### 1.3 근본 원인

**보안 결정이 4개의 서로 다른 위치에서 독립적으로 이루어진다:**

| 위치 | 체크하는 계층 |
|------|--------------|
| `registration.rs::register_tools_from_cspace()` | CSpace만 |
| `exec_tool.rs::shell_exec()` | Agent Permissions만 |
| `exec_tool.rs::structured_exec()` | Agent Permissions + ExecConfig |
| `access_manager/mod.rs::can_access_path_in_workspace()` | RBAC + Permissions + Workspace |

단일 진입점이 없다. 새로운 도구를 추가하면 개발자가 어디서 권한을 체크해야 하는지 알 수 없다.

---

## 2. 설계 원칙

| 원칙 | 의미 |
|------|------|
| **단일 게이트** | 모든 권한 결정이 `AccessGate` 하나를 통과한다 |
| **계층 위계** | CSpace → RBAC → Permissions → ExecConfig 순으로, 상위 계층이 하위를 포함 |
| **타입으로 보장** | `#[cfg(test)]`가 아닌 newtype으로 컴파일 타임에 bypass 원천 차단 |
| **감사 단일화** | 모든 보안 이벤트가 `AuditTrail` (Merkle chain) 하나에 기록된다 |
| **합성 패턴** | `can_access_path_in_workspace()` 같은 God method를 분해한다 |

---

## 3. 설계

### 3.1 `AgentContext` newtype — 타입 레벨 bypass 차단

`#[cfg(test)]`는 테스트 빌드에서만 안전을 보장한다.
테스트와 프로덕션이 동일 바이너리로 빌드되는 integration test에서는 무력화된다.
대신 **newtype**으로 에이전트 신원을 강제한다:

```rust
// access_manager/context.rs (신규)

/// 에이전트의 보안 신원 — KernelHandle에서만 생성 가능.
///
/// 이 타입이 존재한다는 것 자체가 커널이 에이전트를 인증했다는 증거.
/// 빈 생성자가 없으므로, 권한 없는 코드에서는 만들 수 없다.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// 에이전트 고유 식별자.
    pub agent_id: AgentId,
    /// 에이전트 사람-readable 이름.
    pub agent_name: String,
    /// 에이전트의 capability space.
    pub cspace: Arc<CSpace>,
}

// KernelHandle만 AgentContext를 생성할 수 있다.
// (agent_lifecycle.rs에서 fork 시 이미 에이전트를 인증하므로)
```

`ExecTool`의 생성자를 변경:

```rust
// tools/exec_tool.rs — 변경 전
pub fn new(config: Arc<ExecConfig>, access: Arc<Mutex<AccessManager>>) -> Self {
    Self { config, access, agent_name: None }
}

// tools/exec_tool.rs — 변경 후
impl ExecTool {
    /// 유일한 프로덕션 생성자 — AgentContext 필수.
    pub fn new(
        config: Arc<ExecConfig>,
        access: Arc<Mutex<AccessManager>>,
        ctx: AgentContext,
    ) -> Self {
        Self {
            config,
            access,
            agent_context: Some(ctx),
        }
    }

    /// KernelHandle에서 생성 (기존 from_kernel 대체).
    pub fn from_kernel(kernel: &KernelHandle, ctx: AgentContext) -> Self {
        Self::new(
            Arc::new(kernel.exec.config().clone()),
            kernel.exec.access_manager().clone(),
            ctx,
        )
    }
}

// 테스트에서는 AgentContext::test_fixture() 사용
impl AgentContext {
    #[cfg(test)]
    pub fn test_fixture(name: &str) -> Self {
        Self {
            agent_id: AgentId::new_v4(),
            agent_name: name.to_string(),
            cspace: Arc::new(CSpace::new(AgentId::new_v4())),
        }
    }
}
```

**왜 `#[cfg(test)]`보다 나은가:**

| | `#[cfg(test)]` | `AgentContext` newtype |
|---|---|---|
| 컴파일 타임 보장 | cfg attribute = 조건부 컴파일 | 타입 존재 자체가 증명 |
| integration test | `#[cfg(test)]` 비활성화됨 | 동일하게 작동 |
| API 오용 가능성 | `new()`를 그대로 호출 가능 | `AgentContext` 없으면 컴파일 안 됨 |
| 의도 전달 | 암묵적 | 명시적 |

### 3.2 `AccessGate` — 단일 권한 게이트

`AccessManager`를 래핑하지 않고, **그 자체로 게이트 역할**을 하도록 재설계한다.
`RbacManager`를 `AccessManager` 밖으로 빼지 않는다 — 이미 내부에 있으므로.

```rust
// access_manager/gate.rs (신규)

/// 모든 권한 결정의 단일 진입점.
///
/// 사용 예:
/// ```rust,ignore
/// let gate = AccessGate::new(access_manager, audit_sink);
///
/// // 도구 접근
/// gate.check(CheckRequest::tool(&ctx, "exec")).await?;
///
/// // 경로 접근
/// gate.check(CheckRequest::path(&ctx, "/workspace/file.rs", PathMode::Read)).await?;
///
/// // 실행 권한
/// gate.check(CheckRequest::exec(&ctx, "git", &["push"])).await?;
/// ```
pub struct AccessGate {
    /// 권한 관리 (내부에 RBAC 포함).
    access: Arc<Mutex<AccessManager>>,
    /// 실행 설정 (허용 바이너리, 타임아웃).
    exec_config: Arc<ExecConfig>,
    /// 감사 이벤트 싱크 (Merkle chain + 파일).
    audit: Arc<dyn AuditSink>,
}

/// 권한 체크 요청 — 체크 대상과 필요한 권한을 명시.
#[derive(Debug)]
pub enum CheckRequest<'a> {
    /// 도구 사용 권한.
    Tool {
        context: &'a AgentContext,
        tool_name: &'a str,
    },
    /// 경로 접근 권한.
    Path {
        context: &'a AgentContext,
        path: &'a Path,
        mode: PathMode,
    },
    /// 명령 실행 권한.
    Exec {
        context: &'a AgentContext,
        binary: &'a str,
        args: &'a [String],
    },
    /// 네트워크 접근 권한.
    Network {
        context: &'a AgentContext,
    },
    /// 에이전트 fork 권한.
    Fork {
        context: &'a AgentContext,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMode {
    Read,
    Write,
    Execute,
}

/// 권한 거부 결과 — 이유와 사용자 제안 포함.
#[derive(Debug, Clone)]
pub struct AccessDenied {
    pub agent: String,
    pub resource: String,
    pub layer: DenyLayer,
    pub reason: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum DenyLayer {
    /// CSpace에 필요한 Capability 없음.
    Capability,
    /// RBAC Role이 Action 허용 안 함.
    Rbac,
    /// AgentPermissions에서 거부.
    Permission,
    /// ExecConfig에서 거부 (바이너리 비허용, 메타문자).
    ExecPolicy,
}

impl AccessGate {
    pub fn new(
        access: Arc<Mutex<AccessManager>>,
        exec_config: Arc<ExecConfig>,
        audit: Arc<dyn AuditSink>,
    ) -> Self {
        Self { access, exec_config, audit }
    }

    /// 단일 권한 체크 메서드 — 모든 보안 결정이 이곳을 통과.
    ///
    /// 체크 순서 (짧은 회로):
    ///   1. CSpace — capability 존재 확인
    ///   2. RBAC   — role이 action 허용 확인
    ///   3. Permissions — allowed_tools / allowed_paths 확인
    ///   4. ExecPolicy — 바이너리 허용 + 메타문자 검사
    ///
    /// 어느 계층에서든 거부되면 즉시 반환.
    /// 통과 시 audit에 기록.
    pub async fn check(&self, req: CheckRequest<'_>) -> Result<(), AccessDenied> {
        let result = self.run_checks(&req).await;

        // 감사 기록 (허용/거부 모두)
        self.audit.record(AuditEvent::from_check(&req, &result));

        result
    }

    fn run_checks(&self, req: &CheckRequest<'_>) -> Result<(), AccessDenied> {
        match req {
            CheckRequest::Tool { context, tool_name } => {
                self.check_tool(context, tool_name)
            }
            CheckRequest::Path { context, path, mode } => {
                self.check_path(context, path, *mode)
            }
            CheckRequest::Exec { context, binary, args } => {
                self.check_exec(context, binary, args)
            }
            CheckRequest::Network { context } => {
                self.check_network(context)
            }
            CheckRequest::Fork { context } => {
                self.check_fork(context)
            }
        }
    }

    fn check_tool(&self, ctx: &AgentContext, tool: &str) -> Result<(), AccessDenied> {
        // Layer 0: CSpace
        let resource = ResourceRef::KernelDomain { domain: tool.to_string() };
        if !ctx.cspace.can(&resource, Rights::EXECUTE) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: tool.to_string(),
                layer: DenyLayer::Capability,
                reason: format!("CSpace에 '{}' 도구에 대한 EXECUTE capability 없음", tool),
                suggestion: Some(format!(
                    "에이전트의 Seed에 '{}' capability를 추가하세요.",
                    tool
                )),
            });
        }

        // Layer 1+2: RBAC + Permissions (AccessManager 내부에서 체크)
        let mut access = self.access.lock();
        if !access.can_use_tool(&ctx.agent_name, tool) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: tool.to_string(),
                layer: DenyLayer::Permission,
                reason: format!("Agent '{}'의 allowed_tools에 '{}' 없음", ctx.agent_name, tool),
                suggestion: Some(format!(
                    "관리자에게 '{}' 에이전트의 '{}' 도구 권한을 요청하세요.",
                    ctx.agent_name, tool
                )),
            });
        }

        Ok(())
    }

    fn check_path(&self, ctx: &AgentContext, path: &Path, mode: PathMode) -> Result<(), AccessDenied> {
        let path_str = path.to_string_lossy();

        // Layer 0: CSpace (파일 도구는 KernelDomain { domain: "fs" })
        let resource = ResourceRef::KernelDomain { domain: "fs".to_string() };
        let required = match mode {
            PathMode::Read => Rights::READ,
            PathMode::Write | PathMode::Execute => Rights::WRITE,
        };
        if !ctx.cspace.can(&resource, required) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: path_str.to_string(),
                layer: DenyLayer::Capability,
                reason: format!("CSpace에 파일 시스템 {:?} 권한 없음", mode),
                suggestion: Some("Seed에 파일 시스템 capability를 추가하세요.".into()),
            });
        }

        // Layer 1+2: RBAC + Permissions + Workspace sandbox
        let mut access = self.access.lock();
        let workspace = access.get_workspace_for_agent(&ctx.agent_name);

        // RBAC 체크 (AccessManager 내부에서 수행)
        let rbac_subject = Subject::Agent(ctx.agent_id);
        let rbac_action = Action::AccessPath(path_str.to_string());
        if !access.rbac_manager_mut().check_permission(&rbac_subject, &rbac_action, &path_str) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: path_str.to_string(),
                layer: DenyLayer::Rbac,
                reason: "RBAC 정책이 경로 접근을 허용하지 않음".into(),
                suggestion: Some("RBAC 정책을 확인하세요.".into()),
            });
        }

        // Path permissions + Workspace boundary
        if !access.can_access_path(&ctx.agent_name, &path_str) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: path_str.to_string(),
                layer: DenyLayer::Permission,
                reason: format!("경로 '{}'이(가) 허용 목록에 없거나 거부 목록에 포함됨", path_str),
                suggestion: Some("allowed_paths / denied_paths 설정을 확인하세요.".into()),
            });
        }

        // Workspace sandbox (할당된 경우)
        if let Some(ws) = workspace {
            if !access.is_path_in_workspace(&ws, &path_str) {
                return Err(AccessDenied {
                    agent: ctx.agent_name.clone(),
                    resource: path_str.to_string(),
                    layer: DenyLayer::Permission,
                    reason: format!("경로 '{}'이(가) 워크스페이스 '{}' 경계를 벗어남", path_str, ws),
                    suggestion: None,
                });
            }
        }

        Ok(())
    }

    fn check_exec(&self, ctx: &AgentContext, binary: &str, args: &[String]) -> Result<(), AccessDenied> {
        // Layer 0: CSpace
        let resource = ResourceRef::Exec { mode: "structured".to_string() };
        if !ctx.cspace.can(&resource, Rights::EXECUTE) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: binary.to_string(),
                layer: DenyLayer::Capability,
                reason: "CSpace에 Exec capability 없음".into(),
                suggestion: Some("Seed에 Exec capability를 추가하세요.".into()),
            });
        }

        // Layer 1+2: Permissions (bash 또는 binary 이름)
        let tool_name = if binary == "bash" { "bash" } else { binary };
        let mut access = self.access.lock();
        if !access.can_use_tool(&ctx.agent_name, tool_name) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: binary.to_string(),
                layer: DenyLayer::Permission,
                reason: format!("에이전트가 '{}' 실행 권한 없음", binary),
                suggestion: None,
            });
        }

        // Layer 3: ExecConfig — 바이너리 허용 + 메타문자 검사
        if !self.exec_config.is_binary_allowed(binary) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: binary.to_string(),
                layer: DenyLayer::ExecPolicy,
                reason: format!("바이너리 '{}'이(가) 허용 목록에 없음", binary),
                suggestion: Some("exec.allowed_commands에 추가하세요.".into()),
            });
        }

        if has_metacharacters(args) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: binary.to_string(),
                layer: DenyLayer::ExecPolicy,
                reason: "인수에 셸 메타문자 또는 경로 순회 패턴 포함".into(),
                suggestion: None,
            });
        }

        Ok(())
    }

    fn check_network(&self, ctx: &AgentContext) -> Result<(), AccessDenied> {
        let mut access = self.access.lock();
        if !access.can_access_network(&ctx.agent_name) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: "<network>".into(),
                layer: DenyLayer::Permission,
                reason: "네트워크 접근이 비활성화됨".into(),
                suggestion: Some("permissions.network_access를 true로 설정하세요.".into()),
            });
        }
        Ok(())
    }

    fn check_fork(&self, ctx: &AgentContext) -> Result<(), AccessDenied> {
        // CSpace check
        let resource = ResourceRef::KernelDomain { domain: "agent".to_string() };
        if !ctx.cspace.can(&resource, Rights::EXECUTE) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: "fork".into(),
                layer: DenyLayer::Capability,
                reason: "CSpace에 에이전트 관리 capability 없음".into(),
                suggestion: None,
            });
        }

        let access = self.access.lock();
        if !access.can_fork(&ctx.agent_name) {
            return Err(AccessDenied {
                agent: ctx.agent_name.clone(),
                resource: "fork".into(),
                layer: DenyLayer::Permission,
                reason: "에이전트 fork 권한 없음".into(),
                suggestion: Some("permissions.can_fork를 true로 설정하세요.".into()),
            });
        }
        Ok(())
    }
}

/// 메타문자 검사 (exec_tool.rs에서 gate.rs로 이동).
fn has_metacharacters(args: &[String]) -> bool {
    const SHELL_METACHARS: &[char] = &[
        '|', '&', ';', '$', '`', '<', '>', '(', ')', '{', '}', '\n', '\r', '\0',
    ];
    for arg in args {
        if arg.contains("..") || SHELL_METACHARS.iter().any(|&c| arg.contains(c)) {
            return true;
        }
    }
    false
}
```

**핵심 인사이트:** `AccessGate`는 `Arc<RbacManager>`를 별도로 갖지 않는다.
`AccessManager` 내부의 `rbac` 필드에 접근할 뿐이다.
중복 없이 기존 구조를 활용한다.

### 3.3 `AuditSink` — 감사 로그 삼중 분열 해소

현재 감사 이벤트가 세 곳에 흩어져 있다:

| 위치 | 타입 | 영속 |
|------|------|------|
| `AccessManager.audit_log` | `Vec<AuditEntry>` | 메모리만 |
| `RbacManager.audit_log` | `Vec<RbacAuditEntry>` | 메모리만 |
| `AuditTrail` | `Vec<AuditEntry>` (Merkle chain) | 메모리 + flush 시 파일 |

세 시스템을 하나의 싱크로 통합한다:

```rust
// access_manager/audit_sink.rs (신규)

/// 감사 이벤트 싱크 — 모든 보안 이벤트가 이곳으로 흐름.
pub trait AuditSink: Send + Sync {
    /// 감사 이벤트를 기록.
    fn record(&self, event: AuditEvent);
}

/// 모든 감사 이벤트의 통합 타입.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AuditEvent {
    /// 도구 접근 결정.
    ToolAccess {
        timestamp: DateTime<Utc>,
        agent: String,
        tool: String,
        allowed: bool,
        layer: Option<String>, // 거부 시 어느 계층에서
        reason: Option<String>,
    },
    /// 경로 접근 결정.
    PathAccess {
        timestamp: DateTime<Utc>,
        agent: String,
        path: String,
        mode: String,
        allowed: bool,
        layer: Option<String>,
        reason: Option<String>,
    },
    /// 실행 결정.
    ExecAccess {
        timestamp: DateTime<Utc>,
        agent: String,
        binary: String,
        allowed: bool,
        layer: Option<String>,
        reason: Option<String>,
    },
    /// RBAC 권한 결정.
    RbacDecision {
        timestamp: DateTime<Utc>,
        subject: String,
        action: String,
        resource: String,
        allowed: bool,
        reason: Option<String>,
    },
    /// 샌드박스 위반.
    SandboxViolation {
        timestamp: DateTime<Utc>,
        agent: String,
        path: String,
        workspace: String,
    },
    /// HitL 승인 요청/결정.
    Approval {
        timestamp: DateTime<Utc>,
        approval_id: String,
        subject: String,
        action: String,
        status: String, // "requested", "approved", "rejected", "expired"
    },
}

/// 기본 구현: AuditTrail (Merkle chain) + 파일 영속.
pub struct TrailAuditSink {
    /// Merkle chain — 변조 탐지.
    trail: Arc<AuditTrail>,
    /// 비동기 파일 writer (bounded channel).
    file_tx: tokio::sync::mpsc::Sender<String>,
}

impl TrailAuditSink {
    pub fn new(trail: Arc<AuditTrail>, audit_path: PathBuf) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1000);

        // 백그라운드 파일 writer
        tokio::spawn(async move {
            if let Ok(mut file) = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&audit_path)
                .await
            {
                use tokio::io::AsyncWriteExt;
                while let Some(line) = rx.recv().await {
                    let _ = file.write_all(line.as_bytes()).await;
                    let _ = file.write_all(b"\n").await;
                }
            }
        });

        Self { trail, file_tx: tx }
    }
}

impl AuditSink for TrailAuditSink {
    fn record(&self, event: AuditEvent) {
        // 1. Merkle chain에 추가 (변조 탐지)
        let action = match &event {
            AuditEvent::ToolAccess { tool, allowed, .. } => AuditAction::Other {
                detail: format!("tool_access:{}:allowed={}", tool, allowed),
            },
            AuditEvent::PathAccess { path, allowed, .. } => AuditAction::Other {
                detail: format!("path_access:{}:allowed={}", path, allowed),
            },
            AuditEvent::RbacDecision { subject, action, allowed, .. } => AuditAction::Other {
                detail: format!("rbac:{}:{}:allowed={}", subject, action, allowed),
            },
            _ => AuditAction::Other {
                detail: format!("{:?}", event.kind()),
            },
        };

        let actor = match &event {
            AuditEvent::ToolAccess { agent, .. } => agent.clone(),
            AuditEvent::PathAccess { agent, .. } => agent.clone(),
            AuditEvent::ExecAccess { agent, .. } => agent.clone(),
            AuditEvent::RbacDecision { subject, .. } => subject.clone(),
            AuditEvent::SandboxViolation { agent, .. } => agent.clone(),
            AuditEvent::Approval { subject, .. } => subject.clone(),
        };

        self.trail.append(actor.clone(), action, "access_gate".into());

        // 2. JSONL 파일에 영속 (fire-and-forget)
        if let Ok(line) = serde_json::to_string(&event) {
            let _ = self.file_tx.try_send(line);
        }
    }
}

/// 테스트용 no-op 싱크.
#[cfg(test)]
pub struct NoOpAuditSink;

#[cfg(test)]
impl AuditSink for NoOpAuditSink {
    fn record(&self, _event: AuditEvent) {}
}
```

**마이그레이션:** `AccessManager`와 `RbacManager`의 내부 `Vec<AuditEntry>` / `Vec<RbacAuditEntry>`는
`AccessGate` 도입 후 점진적으로 제거한다.
Phase 2에서는 두 시스템을 병렬로 유지하면서 교차 검증하고,
Phase 3에서 메모리 내 로그를 제거한다.

### 3.4 `GatedRegistry` — 도구 레벨이 아닌 Registry 레벨에서 인터셉트

`GatedTool<T>` 래퍼는 각 도구를 개별적으로 래핑해야 한다.
oxi-sdk crate에서 제공하는 도구(`ReadTool`, `WriteTool` 등)는
소스코드에 접근할 수 없어 래핑이 어려울 수 있다.

대신 **Registry 레벨**에서 인터셉트한다:

```rust
// tools/gated_registry.rs (신규)

/// 권한 체크가 내장된 ToolRegistry 프록시.
///
/// 모든 도구 호출이 AccessGate를 통과하도록 인터셉트.
/// 도구 코드는 수정하지 않는다.
pub struct GatedRegistry {
    /// 실제 도구 레지스트리.
    inner: ToolRegistry,
    /// 권한 게이트.
    gate: Arc<AccessGate>,
    /// 에이전트 신원.
    context: AgentContext,
}

impl GatedRegistry {
    pub fn new(
        inner: ToolRegistry,
        gate: Arc<AccessGate>,
        context: AgentContext,
    ) -> Self {
        Self { inner, gate, context }
    }

    /// 권한 체크 후 도구 실행.
    ///
    /// 1. tool_name으로 CSpace/RBAC/Permissions 체크
    /// 2. 파일 도구인 경우 params에서 path 추출하여 경로 체크
    /// 3. 통과하면 실제 도구 실행
    pub async fn execute(
        &self,
        tool_name: &str,
        tool_call_id: &str,
        params: Value,
        signal: Option<oneshot::Receiver<()>>,
        ctx: &ToolContext,
    ) -> Result<AgentToolResult, String> {
        // 사전 권한 체크
        let check = CheckRequest::Tool {
            context: &self.context,
            tool_name,
        };

        if let Err(denied) = self.gate.check_sync(&check) {
            tracing::warn!(
                agent = %denied.agent,
                tool = %tool_name,
                layer = ?denied.layer,
                "도구 접근 거부"
            );
            return Ok(AgentToolResult::error(&format!(
                "🔒 권한 거부: {} — {} {}",
                denied.reason,
                denied.suggestion.unwrap_or_default(),
                match denied.layer {
                    DenyLayer::Capability => "[CSpace]",
                    DenyLayer::Rbac => "[RBAC]",
                    DenyLayer::Permission => "[Permissions]",
                    DenyLayer::ExecPolicy => "[ExecPolicy]",
                }
            )));
        }

        // 파일 도구인 경우 경로 체크
        if let Some(path) = extract_path_from_params(tool_name, &params) {
            let mode = path_mode_for_tool(tool_name);
            let path_check = CheckRequest::Path {
                context: &self.context,
                path: Path::new(&path),
                mode,
            };
            if let Err(denied) = self.gate.check_sync(&path_check) {
                tracing::warn!(
                    agent = %denied.agent,
                    path = %path,
                    tool = %tool_name,
                    "경로 접근 거부"
                );
                return Ok(AgentToolResult::error(&format!(
                    "🔒 경로 접근 거부: {}", denied.reason
                )));
            }
        }

        // 권한 통과 — 실제 도구 실행
        self.inner
            .execute(tool_name, tool_call_id, params, signal, ctx)
            .await
    }

    /// 내부 registry의 tool name 목록 반환.
    pub fn tool_names(&self) -> Vec<String> {
        self.inner.names()
    }
}

/// 도구 이름에서 path 파라미터 추출.
fn extract_path_from_params(tool_name: &str, params: &Value) -> Option<String> {
    match tool_name {
        "read" | "ls" | "find" | "grep" => params.get("path").and_then(|v| v.as_str()).map(String::from),
        "write" | "edit" => params.get("path").and_then(|v| v.as_str()).map(String::from),
        _ => None,
    }
}

/// 도구 이름에서 필요한 경로 접근 모드 결정.
fn path_mode_for_tool(tool_name: &str) -> PathMode {
    match tool_name {
        "write" | "edit" => PathMode::Write,
        _ => PathMode::Read,
    }
}
```

등록부 변경:

```rust
// tools/registration.rs — 변경

/// 에이전트의 권한이 적용된 registry를 생성.
///
/// CSpace는 여전히 도구 등록/미등록을 결정하고,
/// GatedRegistry는 등록된 도구의 실행 시 권한을 체크한다.
pub fn build_gated_registry(
    kernel: &KernelHandle,
    cspace: &CSpace,
    search_cache: Arc<SearchCache>,
    agent_id: AgentId,
    gate: Arc<AccessGate>,
    context: AgentContext,
) -> GatedRegistry {
    let registry = ToolRegistry::new();

    // 1. CSpace 기반 도구 등록 (기존 로직 그대로)
    register_tools_from_cspace(&registry, kernel, cspace, search_cache, agent_id);

    // 2. GatedRegistry로 래핑
    GatedRegistry::new(registry, gate, context)
}
```

**왜 `GatedTool<T>`보다 나은가:**

| | `GatedTool<T>` (기존 제안) | `GatedRegistry` (개선) |
|---|---|---|
| 도구당 코드 | 매 도구마다 래핑 필요 | 도구 코드 수정 없음 |
| crate 외부 도구 | 생성자 접근 필요 | registry만 있으면 됨 |
| 새 도구 추가 시 | 래핑 코드도 추가 | 자동 보호 |
| 책임 | 도구가 권한을 앎 | Registry가 권한을 관리 |

### 3.5 ExecTool에 게이트 적용

ExecTool은 `GatedRegistry`의 인터셉트 대상이 아니다 (이미 자체 권한 체크가 있기 때문).
대신 기존의 `access.lock().can_use_tool()` 호출을 `gate.check()`로 교체:

```rust
// tools/exec_tool.rs — 변경

pub struct ExecTool {
    config: Arc<ExecConfig>,
    gate: Arc<AccessGate>,
    /// 에이전트 신원 — None 불가 (AgentContext newtype).
    context: AgentContext,
}

impl ExecTool {
    pub fn new(
        config: Arc<ExecConfig>,
        gate: Arc<AccessGate>,
        context: AgentContext,
    ) -> Self {
        Self { config, gate, context }
    }

    pub fn from_kernel(kernel: &KernelHandle, gate: Arc<AccessGate>, context: AgentContext) -> Self {
        Self::new(
            Arc::new(kernel.exec.config().clone()),
            gate,
            context,
        )
    }
}

// shell_exec / structured_exec 내부:
// 기존: access.lock().can_use_tool(name, "bash")
// 변경: self.gate.check(CheckRequest::Exec { ... })
```

### 3.6 ExecConfig 허용 목록 의미론 수정

현재 `is_binary_allowed()`에서 **빈 목록 = 모두 허용**이다:

```rust
// 현재 (위험)
pub fn is_binary_allowed(&self, name: &str) -> bool {
    self.allowed_commands.is_empty() || self.allowed_commands.iter().any(|c| c == name)
}
```

명시적 모드 전환을 도입:

```rust
// config.rs — 변경

/// 허용 목록 동작 모드.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AllowlistMode {
    /// 모든 바이너리 허용 (개발 전용).
    Permissive,
    /// 허용 목록에 있는 바이너리만 실행.
    Enforced,
}

impl Default for AllowlistMode {
    fn default() -> Self {
        // 프로덕션 안전: 기본적으로 강제 모드.
        Self::Enforced
    }
}

pub struct ExecConfig {
    // ... 기존 필드 ...
    /// 허용 목록 모드.
    #[serde(default)]
    pub allowlist_mode: AllowlistMode,
}

impl ExecConfig {
    pub fn is_binary_allowed(&self, name: &str) -> bool {
        match self.allowlist_mode {
            AllowlistMode::Permissive => true,
            AllowlistMode::Enforced => self.allowed_commands.iter().any(|c| c == name),
        }
    }
}
```

### 3.7 기본 설정 정리

```toml
# share/default-config.toml — 변경

[gateway]
# 모든 인터페이스 바인딩 제거, localhost만.
host = "127.0.0.1"
port = 4200

[exec]
default_mode = "structured"
allow_shell_mode = false
# 허용 목록 모드 추가 (기본값: enforced).
allowlist_mode = "enforced"
# 최소 안전 바이너리 세트.
# 빈 목록이 "모두 허용"이 되지 않음 (allowlist_mode=enforced).
allowed_commands = [
    "ls", "cat", "head", "tail", "wc",
    "grep", "rg", "find", "fd",
    "git", "cargo", "rustc",
    "python3", "node", "bun",
    "curl", "wget",
    "jq", "yq",
    "echo", "mkdir", "cp", "mv",
    # 제거: "osascript" — macOS AppleScript 임의 실행 위험
    # 제거: "open"     — 임의 앱/URL 열기 위험
    # 제거: "shortcuts" — macOS Shortcuts 임의 실행 위험
    # 제거: "gh"       — GitHub CLI (repo scope 위험, 필요시 명시적 추가)
]
default_timeout_secs = 120
max_timeout_secs = 600
```

### 3.8 동적 권한 상승 (HitL 승인 흐름)

현재 `RbacManager::request_approval()`이 승인 요청을 생성하지만,
이것이 `EventBus`와 연결되어 **사용자에게 전달되는 흐름**이 설계되지 않았다.

`AccessGate`에서 자동 권한 상승을 지원:

```rust
// access_manager/gate.rs — 추가

impl AccessGate {
    /// 권한 체크 + 자동 HitL 상승.
    ///
    /// 일반 check()가 실패하면, Action이 상승 가능한 경우
    /// PendingApproval을 생성하고 EventBus에 이벤트를 발행한다.
    /// 승인 대기 최대 시간은 `escalation_timeout`으로 제어.
    pub async fn check_or_escalate(
        &self,
        req: CheckRequest<'_>,
        event_bus: &EventBus,
        escalation_timeout: Duration,
    ) -> Result<(), AccessDenied> {
        // 먼저 일반 체크 시도
        if self.check(&req).await.is_ok() {
            return Ok(());
        }

        // 상승 가능한지 확인 (모든 거부가 상승 가능한 것은 아님)
        let action = match &req {
            CheckRequest::Tool { context, tool_name } => {
                Action::UseTool(tool_name.to_string())
            }
            CheckRequest::Exec { context, binary, .. } => {
                Action::UseTool(binary.to_string())
            }
            _ => return Err(AccessDenied::not_escalatable(&req)),
        };

        // 고위험 액션만 상승 허용
        if !action.requires_approval() {
            return Err(AccessDenied::not_escalatable(&req));
        }

        // 승인 요청 생성
        let context = req.agent_context();
        let mut access = self.access.lock();
        let subject = Subject::Agent(context.agent_id);
        let approval_id = access.rbac_manager_mut().request_approval(
            subject,
            action,
            req.resource().to_string(),
            format!("에이전트 '{}'의 권한 상승 요청", context.agent_name),
        );

        // EventBus에 알림 (채널이 사용자에게 전달)
        let _ = event_bus.emit(KernelEvent::ApprovalRequested {
            id: approval_id,
            action: format!("{:?}", req),
            resource: req.resource().to_string(),
            reason: format!("자동 권한 상승: {}", context.agent_name),
        }).await;

        // TODO: Phase 4에서 채널(Web/CLI/Telegram)이 이 이벤트를 수신하여
        // 사용자에게 승인 UI를 표시하는 흐름 구현.
        // 현재는 타임아웃으로 만료 처리.
        Err(AccessDenied {
            agent: context.agent_name.clone(),
            resource: req.resource().to_string(),
            layer: DenyLayer::Permission,
            reason: "권한 상승 요청됨 — 사용자 승인 대기 중".into(),
            suggestion: Some(format!("승인 ID: {}", approval_id)),
        })
    }
}
```

---

## 4. 마이그레이션 계획

### Phase 1: Critical 보안 수정 (1일)

> 리그레이션 없이 즉시 적용 가능한 최소 변경.

| 작업 | 파일 | 세부내용 |
|------|------|----------|
| `ExecConfig::allowlist_mode` 추가 | `config.rs` | `AllowlistMode` enum, 기본값 `Enforced` |
| `ExecConfig::is_binary_allowed()` 수정 | `config.rs` | 빈 목록 ≠ 모두 허용 |
| 기본 설정에서 위험 바이너리 제거 | `share/default-config.toml` | `osascript`, `open`, `shortcuts`, `gh` 제거 |
| `gateway.host` 기본값 변경 | `share/default-config.toml` | `0.0.0.0` → `127.0.0.1` |
| `ExecTool::new()`에 `AgentContext` 필수화 | `tools/exec_tool.rs` | `Option<String>` → `AgentContext` |

**Phase 1 후 달성되는 것:**
- G1 해결 (bypass 타입 레벨 차단)
- G5 해결 (위험 바이너리 제거)
- 빈 허용 목록 문제 해결

### Phase 2: 통합 권한 게이트 + AuditSink (2-3일)

> `AccessGate` + `TrailAuditSink` 도입. 기존 로직과 병렬 운영.

| 작업 | 파일 | 세부내용 |
|------|------|----------|
| `AgentContext` newtype | `access_manager/context.rs` (신규) | 에이전트 신원 타입 |
| `AccessGate` 구현 | `access_manager/gate.rs` (신규) | 4계층 통합 체크 |
| `AuditEvent` + `AuditSink` trait | `access_manager/audit_sink.rs` (신규) | 감아 통합 타입 |
| `TrailAuditSink` 구현 | `access_manager/audit_sink.rs` (신규) | Merkle chain + 파일 |
| `GatedRegistry` 구현 | `tools/gated_registry.rs` (신규) | Registry 레벨 인터셉트 |
| ExecTool에 gate 적용 | `tools/exec_tool.rs` | `can_use_tool()` → `gate.check()` |
| `KernelHandle`에 `AccessGate` 추가 | `kernel_handle/mod.rs` | `gate: AccessGate` 필드 |
| `build_gated_registry()` 추가 | `tools/registration.rs` | CSpace + Gate 조합 |

**Phase 2 후 달성되는 것:**
- G2 해결 (RBAC 통합)
- G3 해결 (always-on 도구 권한 체크)
- G4 해결 (감사 로그 영속화)
- G6 해결 (감사 로그 단일화)

### Phase 3: 레거시 정리 (1일)

> `AccessGate`가 안정화된 후 내부 중복 제거.

| 작업 | 파일 | 세부내용 |
|------|------|----------|
| `AccessManager.audit_log: Vec<AuditEntry>` 제거 | `access_manager/mod.rs` | `AuditSink`로 이관 완료 |
| `RbacManager.audit_log: Vec<RbacAuditEntry>` 제거 | `access_manager/rbac.rs` | `AuditSink`로 이관 완료 |
| `can_access_path_in_workspace()` 분해 | `access_manager/mod.rs` | `AccessGate::check_path()`로 대체 |
| `AccessManager::new()`에 `AuditSink` 주입 | `access_manager/mod.rs` | 생성자 파라미터 추가 |

### Phase 4: HitL 승인 흐름 (1일, 선택적)

> 채널(Web/CLI/Telegram)과 연결하여 실시간 권한 상승 UI 제공.

| 작업 | 파일 | 세부내용 |
|------|------|----------|
| `check_or_escalate()` 구현 | `access_manager/gate.rs` | 자동 상승 로직 |
| 채널별 승인 UI | `channels/*/` | 승인 요청/응답 컴포넌트 |
| EventBus에 `ApprovalRequested` 수신 로직 | `channels/*/` | 채널 → 사용자 → 승인/거부 |

---

## 5. 아키텍처 비교

### Before (현재)

```
             ┌─────────────────┐
             │   CSpace        │ ← registration.rs (등록만)
             └────────┬────────┘
                      │ 등록된 도구만
          ┌───────────┴───────────┐
          │                       │
  ┌───────┴───────┐    ┌─────────┴─────────┐
  │ ExecTool      │    │ Always-on tools   │
  │ (Permissions  │    │ (권한 체크 없음)    │
  │  only)        │    │                   │
  └───────┬───────┘    └───────────────────┘
          │
  ┌───────┴───────┐
  │ ExecConfig    │
  └───────────────┘

  감사: 3곳에 분산 (AccessManager.vec + RbacManager.vec + AuditTrail)
  RBAC: can_access_path_in_workspace()에서만 호출
```

### After (개선)

```
             ┌─────────────────────────────────────────┐
             │              AgentContext                │
             │   (agent_id + agent_name + CSpace)      │
             └────────────────┬────────────────────────┘
                              │
             ┌────────────────┴────────────────────┐
             │            AccessGate                │
             │  ┌────────────────────────────────┐ │
             │  │ Layer 0: CSpace capability      │ │
             │  │ Layer 1: RBAC role check        │ │
             │  │ Layer 2: Agent Permissions      │ │
             │  │ Layer 3: ExecConfig policy      │ │
             │  └────────────────────────────────┘ │
             └───────────────┬─────────────────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
    ┌─────────┴─────────┐     ┌────────────┴───────────┐
    │ GatedRegistry     │     │ ExecTool               │
    │ (always-on 도구   │     │ (gate.check_exec)      │
    │  인터셉트)        │     │                        │
    └───────────────────┘     └────────────────────────┘

    감사: TrailAuditSink (Merkle chain + JSONL 파일)
```

---

## 6. 영향 범위

| 컴포넌트 | 변경 | Phase |
|----------|------|-------|
| `config.rs` | `AllowlistMode` enum 추가 | 1 |
| `share/default-config.toml` | 위험 바이너리 제거, host 변경, 모드 추가 | 1 |
| `tools/exec_tool.rs` | `AgentContext` 필수화, gate 적용 | 1-2 |
| `access_manager/context.rs` | `AgentContext` newtype (신규) | 2 |
| `access_manager/gate.rs` | `AccessGate` (신규) | 2 |
| `access_manager/audit_sink.rs` | `AuditEvent` + `AuditSink` trait (신규) | 2 |
| `tools/gated_registry.rs` | `GatedRegistry` (신규) | 2 |
| `tools/registration.rs` | `build_gated_registry()` 추가 | 2 |
| `kernel_handle/mod.rs` | `AccessGate` 필드 추가 | 2 |
| `access_manager/mod.rs` | 내부 audit_log 제거 | 3 |
| `access_manager/rbac.rs` | 내부 audit_log 제거 | 3 |

---

## 7. 위험 및 완화

| 위험 | 심각도 | 완화 |
|------|--------|------|
| 권한 게이트가 너무 엄격해 기존 에이전트 동작 차단 | 🟡 | Phase 1에서 `AllowlistMode::Enforced` + 관대한 기본 목록으로 시작. Phase 2에서 게이트는 점진적 활성화 (feature flag). |
| `GatedRegistry`가 oxi-sdk `ToolRegistry` 인터페이스와 호환되지 않음 | 🟡 | `GatedRegistry`는 `ToolRegistry`의 메서드를 위임. SDK의 `AgentBuilder`가 `ToolRegistry`를 소유하므로, `GatedRegistry`가 `ToolRegistry`를 감싸는 프록시 패턴으로 해결. |
| `AccessGate` 동기 체크가 `AccessManager` Mutex 락 경합 | 🟢 | 체크는 HashMap lookup (O(1)) + Vec linear scan (짧은 목록). 실제 프로파일링 전까지는 미미한 오버헤드. 필요시 `RwLock`으로 읽기 병렬화. |
| `TrailAuditSink` 파일 writer 채널 full 시 감사 이벤트 손실 | 🟢 | bounded channel (1000) + `try_send`. full 시 경고 로그 + 메모리 버퍼에 폴백. AuditTrail(Merkle)은 항상 기록되므로 파일 누락은 변조 탐지로 확인 가능. |
| Phase 3 레거시 정리 시 `can_access_path_in_workspace()` 제거가 다른 호출자에 영향 | 🟡 | grep으로 모든 호출처를 확인 후 `AccessGate::check_path()`로 마이그레이션. |

---

## 8. 성공 기준

### Phase 1 완료 기준

- [ ] `ExecTool`에 `Option<String>` agent_name이 존재하지 않음 (→ `AgentContext`)
- [ ] 빈 `allowed_commands`가 "모두 허용"이 아님
- [ ] 기본 설정에 `osascript`, `open`, `shortcuts` 미포함
- [ ] `gateway.host` 기본값이 `127.0.0.1`
- [ ] `cargo test --workspace` 통과

### Phase 2 완료 기준

- [ ] 모든 도구가 단일 `AccessGate::check()` 경로를 통과
- [ ] Always-on 도구도 CSpace/RBAC/Permissions 체크 가능
- [ ] 감사 이벤트가 `TrailAuditSink` (Merkle + JSONL)에 기록
- [ ] `AccessDenied` 응답에 거부 계층(`DenyLayer`)과 제안 포함
- [ ] `cargo test --workspace` 통과

### Phase 3 완료 기준

- [ ] `AccessManager.audit_log: Vec<AuditEntry>` 필드 제거
- [ ] `RbacManager.audit_log: Vec<RbacAuditEntry>` 필드 제거
- [ ] 감사 이벤트는 `Arc<dyn AuditSink>`로만 흐름
- [ ] 기존 에이전트 동작 회귀 없음

### 최종 기준

- [ ] 4개 보안 계층이 명시적 위계로 정렬 (CSpace → RBAC → Permissions → ExecConfig)
- [ ] 단일 게이트(`AccessGate`)가 모든 권한 결정의 진입점
- [ ] 감사 로그가 단일 싱크(`AuditSink`)로 통합
- [ ] 프로덕션 빌드에서 권한 없는 실행 경로가 타입 레벨에서 차단
