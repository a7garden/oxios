# RFC-015: 보안 모델 통합

> **상태:** 📝 설계
> **날짜:** 2026-05-26
> **우선순위:** P0-P1
> **범위:** `access_manager/`, `tools/exec_tool.rs`, `tools/kernel_bridge.rs`
> **선행:** 없음
> **후행:** 없음

---

## 1. 동기

현재 3개의 독립적인 보안 계층이 서로 다른 시점에 체크되어 간격(gap)이 존재:

```
Layer 1: RBAC (RbacManager)        — Subject + Action + Resource
Layer 2: Agent Permissions          — 도구/경로/네트워크/fork/시간/메모리
Layer 3: ExecConfig                 — 바이너리 허용목록, 셸 모드, 타임아웃
```

**발견된 간격:**

| # | 간격 | 심각도 |
|---|------|--------|
| G1 | `ExecTool::new()` (agent_name=None)가 권한 체크 완전 우회 | 🔴 |
| G2 | ExecTool은 Agent Permissions만 체크, RBAC 무시 | 🟡 |
| G3 | Always-on 도구(read/write/edit/grep/find/ls)가 AccessManager 통과 안함 | 🟡 |
| G4 | RBAC audit log 파일 미영속 | 🟢 |
| G5 | 기본 `allowed_commands`에 `osascript` 포함 | 🟡 |

---

## 2. 설계

### 2.1 ExecTool bypass 제거

```rust
// 변경 전
impl ExecTool {
    pub fn new(config: ExecConfig) -> Self {
        Self { config, agent_name: None }
    }
}

// 변경 후
impl ExecTool {
    /// 프로덕션용: 항상 에이전트 이름 필요
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        let agent_name = kernel.agent_name()
            .expect("ExecTool requires an agent context")
            .to_string();
        Self {
            config: kernel.exec_config().clone(),
            agent_name: Some(agent_name),
        }
    }

    /// 테스트 전용: 명시적 bypass 승인 필요
    #[cfg(test)]
    pub fn new_unrestricted(config: ExecConfig) -> Self {
        Self { config, agent_name: None }
    }
}

// cfg(test)가 아니면 new_unrestricted()가 존재하지 않음
// 일반 new()는 제거
```

### 2.2 통합 권한 체크 게이트

ExecTool, 그리고 모든 도구의 권한 체크를 단일 경로로 통합:

```rust
/// 통합 권한 검사 — 모든 도구가 호출
pub struct UnifiedAccessGate {
    rbac: Arc<RbacManager>,
    agent_perms: Arc<Mutex<AccessManager>>,
}

impl UnifiedAccessGate {
    /// 도구 실행 전 통합 권한 검사
    pub async fn check_tool_access(
        &self,
        agent_name: &str,
        tool_name: &str,
        action: &str, // "execute", "read", "write"
    ) -> Result<(), AccessDenied> {
        // 1. Agent Permissions 체크
        let perms = self.agent_perms.lock().await;
        if !perms.can_use_tool(agent_name, tool_name) {
            return Err(AccessDenied {
                agent: agent_name.into(),
                tool: tool_name.into(),
                reason: DenyReason::NotInAllowedTools,
                suggestion: Some(format!(
                    "관리자에게 '{}' 에이전트의 '{}' 도구 권한을 요청하세요.",
                    agent_name, tool_name
                )),
            });
        }

        // 2. RBAC 체크
        let subject = Subject::Agent(agent_name.into());
        let rbac_action = Action::UseTool(tool_name.into());
        if !self.rbac.check(&subject, &rbac_action) {
            return Err(AccessDenied {
                agent: agent_name.into(),
                tool: tool_name.into(),
                reason: DenyReason::RbacDenied,
                suggestion: Some("RBAC 정책을 확인하세요.".into()),
            });
        }

        Ok(())
    }

    /// 경로 접근 권한 검사 (read/write/edit 도구용)
    pub async fn check_path_access(
        &self,
        agent_name: &str,
        path: &Path,
        mode: PathMode, // Read, Write, Execute
    ) -> Result<(), AccessDenied> {
        // 1. Workspace sandbox
        let perms = self.agent_perms.lock().await;
        if !perms.can_access_path(agent_name, path)? {
            return Err(AccessDenied::path_denied(agent_name, path));
        }

        // 2. RBAC 경로 액션
        let subject = Subject::Agent(agent_name.into());
        let action = Action::AccessPath(path.to_string_lossy().into());
        if !self.rbac.check(&subject, &action) {
            return Err(AccessDenied::rbac_path_denied(agent_name, path));
        }

        Ok(())
    }
}
```

### 2.3 Always-on 도구에도 권한 체크 적용

현재 oxi-sdk의 파일 도구들은 AccessManager를 우회한다. KernelBridge 레벨에서 권한 게이트를 추가:

```rust
// tools/kernel_bridge.rs — 권한 래퍼

/// 권한 체크가 내장된 도구 래퍼
pub struct GatedTool<T: AgentTool> {
    inner: T,
    gate: Arc<UnifiedAccessGate>,
    agent_name: String,
}

#[async_trait]
impl<T: AgentTool> AgentTool for GatedTool<T> {
    fn name(&self) -> &str { self.inner.name() }
    fn description(&self) -> &'static str { self.inner.description() }
    fn parameters_schema(&self) -> Value { self.inner.parameters_schema() }

    async fn execute(&self, id: &str, params: Value, cx: ToolContext) -> Result<AgentToolResult, String> {
        // 사전 권한 체크
        if let Err(denied) = self.gate.check_tool_access(&self.agent_name, self.inner.name(), "execute").await {
            return Ok(AgentToolResult::error(&format!(
                "권한 거부: {} — {}", denied.reason, denied.suggestion.unwrap_or_default()
            )));
        }

        // 파일 도구인 경우 경로 체크
        if let Some(path) = extract_path_from_params(&params) {
            if let Err(denied) = self.gate.check_path_access(&self.agent_name, &path, PathMode::Write).await {
                return Ok(AgentToolResult::error(&format!(
                    "경로 접근 거부: {}", denied.reason
                )));
            }
        }

        self.inner.execute(id, params, cx).await
    }
}
```

등록 시 래핑:

```rust
// kernel_bridge.rs
fn register_always_on(registry: &mut ToolRegistry, gate: Arc<UnifiedAccessGate>, agent_name: &str) {
    for tool in oxi_sdk::default_tools() {
        registry.register(GatedTool::new(tool, gate.clone(), agent_name));
    }
}
```

### 2.4 RBAC Audit Log 영속화

```rust
// access_manager/rbac.rs 변경

pub struct RbacManager {
    // 기존
    policies: RwLock<Vec<RbacPolicy>>,
    audit_log: RwLock<Vec<RbacAuditEntry>>,

    // 추가: 파일 영속화
    audit_sink: Option<AuditSink>, // bounded channel → 백그라운드 파일 writer
}

struct AuditSink {
    tx: mpsc::Sender<RbacAuditEntry>,
}

impl RbacManager {
    pub fn new(max_entries: usize, audit_path: Option<PathBuf>) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        if let Some(path) = &audit_path {
            // 백그라운드 파일 writer 태스크
            let path = path.clone();
            tokio::spawn(async move {
                let mut file = tokio::fs::OpenOptions::new()
                    .create(true).append(true)
                    .open(&path).await
                    .expect("RBAC audit file");

                while let Some(entry) = rx.recv().await {
                    let line = serde_json::to_string(&entry).unwrap();
                    let _ = file.write_all(line.as_bytes()).await;
                    let _ = file.write_all(b"\n").await;
                }
            });
        }

        Self {
            policies: RwLock::new(Vec::new()),
            audit_log: RwLock::new(Vec::new()),
            audit_sink: Some(AuditSink { tx }),
        }
    }
}
```

### 2.5 기본 설정 정리

```toml
# share/default-config.toml — 변경

[exec]
default_mode = "structured"
allowed_commands = [
    "ls", "cat", "head", "tail", "wc",
    "grep", "rg", "find", "fd",
    "git", "cargo", "rustc",
    "python3", "node", "bun",
    "curl", "wget",
    "jq", "yq",
    "echo", "mkdir", "cp", "mv",
    # 제거: "osascript" — macOS arbitrary AppleScript 실행 위험
    # 제거: "open" — arbitrary 앱/URL 열기 위험
]
```

---

## 3. 마이그레이션 계획

### Phase 1: Critical 보안 수정 (0.5일)

| 작업 | 파일 |
|------|------|
| `ExecTool::new()` → `#[cfg(test)]` | `tools/exec_tool.rs` |
| `osascript`/`open` 기본 허용목록 제거 | `share/default-config.toml` |
| `gateway.host` 기본값 `127.0.0.1`로 통일 | `config.rs` 또는 TOML |

### Phase 2: 통합 권한 게이트 (1-2일)

| 작업 | 파일 |
|------|------|
| `UnifiedAccessGate` 구현 | `access_manager/gate.rs` (신규) |
| ExecTool에 게이트 적용 | `tools/exec_tool.rs` |
| `GatedTool<T>` 래퍼 구현 | `tools/kernel_bridge.rs` |
| always-on 도구 래핑 등록 | `tools/kernel_bridge.rs` |

### Phase 3: Audit 영속화 (0.5일)

| 작업 | 파일 |
|------|------|
| RBAC audit 파일 영속화 | `access_manager/rbac.rs` |
| `max_agents` 기본값 통일 (10으로) | `config.rs` |

---

## 4. 영향 범위

| 컴포넌트 | 변경 |
|----------|------|
| `tools/exec_tool.rs` | bypass 제거, 게이트 통합 |
| `tools/kernel_bridge.rs` | GatedTool 래핑 |
| `access_manager/` | `gate.rs` 신규, `rbac.rs` 영속화 |
| `config.rs` | 기본값 통일 |
| `default-config.toml` | osascript 제거, host 변경 |

---

## 5. 위험 및 완화

| 위험 | 완화 |
|------|------|
| 권한 게이트가 너무 엄격해 기존 에이전트 동작 차단 | 기본 정책은 기존 동작 유지, 게이트는 점진적 활성화 |
| `GatedTool` 래퍼가 파일 도구 성능 저하 | 체크는 HashMap lookup (O(1)), 미미한 오버헤드 |
| 테스트 코드가 `ExecTool::new()` 사용 | `#[cfg(test)]`로 분리, 테스트는 영향 없음 |

---

## 6. 성공 기준

- [ ] `ExecTool::new()` (무제한)가 프로덕션 빌드에 존재하지 않음
- [ ] 모든 도구가 단일 권한 체크 경로 통과
- [ ] Always-on 도구도 권한 체크 가능 (선택적 활성화)
- [ ] RBAC audit log가 파일에 영속화
- [ ] 기본 설정에 `osascript`/`open` 미포함
- [ ] 기존 에이전트 동작 회귀 없음
