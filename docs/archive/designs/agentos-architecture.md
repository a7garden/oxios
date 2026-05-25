# Oxios AgentOS — Complete Architecture Design

> Kernel → Tool → Agent 연결 구조와 에이전트 컨텍스트 주입의 통합 설계.
> 자기비판적 태도로 작성. 약점을 숨기지 않음.

---

## 0. 설계 철학

```
Unix가 프로세스에게 syscall을 제공하듯이,
Oxios는 에이전트에게 tool을 제공한다.

프로세스가 커널 내부를 모르듯이,
에이전트는 커널 내부를 모른다.

프로세스가 open/read/write로 파일을 다루듯이,
에이전트는 space_list/exec/browser로 OS를 다룬다.

차이점: Unix 프로세스는 모두 같은 syscall table을 공유하지만,
Oxios 에이전트는 역할에 따라 다른 tool set을 가진다.

이게 AgentOS와 Unix의 유일한 구조적 차이다.
```

---

## 1. 현재 구조 진단

### 1.1 KernelHandle의 사용 현황

```
KernelHandle (8 domain APIs)
├── state       → CLI에서 사용, Guardian에서 사용, Agent에서 ❌
├── agents      → CLI에서 사용, Guardian에서 사용, Agent에서 ❌
├── security    → CLI에서 사용, Guardian에서 사용, Agent에서 ❌
├── persona     → ❌ 아무도 안 씀
├── extensions  → CLI에서 사용, Guardian에서 ❌, Agent에서 ❌
├── mcp         → ❌ 아무도 안 씀
├── infra       → CLI에서 사용, Guardian에서 사용, Agent에서 ❌
└── spaces      → ❌ 아무도 안 씀
```

### 1.2 Tool의 참조 구조

```
현재 Tool → 내부 모듈 직접 참조 (KernelHandle 우회):

ExecTool        → ExecConfig + AccessManager 직접
BrowserTool     → OxibrowserBackend 직접
MemoryTool      → MemoryManager 직접
A2aTool         → A2AProtocol 직접
McpTool         → McpBridge 직접
ProgramTool     → ExecTool 직접 (이것도 우회)

KernelHandle에 있지만 Tool이 안 쓰는 것:
StateApi        → Tool 없음
AgentApi        → Tool 없음
SecurityApi     → Tool 없음
PersonaApi      → Tool 없음
ExtensionApi    → Tool 없음
InfraApi        → Tool 없음
SpaceApi        → Tool 없음
```

### 1.3 System Prompt 구조

```
현재 build_system_prompt():

1. Seed (goal, constraints, acceptance_criteria)     ✅
2. SKILL.md 전체 주입 (모든 enabled program)         ❌ 토큰 폭발
3. Persona prompt                                    ✅
4. Memory blend                                      ✅

빠진 것:
- OS 구조/룰 안내                                      ❌
- Tool 사용법 안내                                      ❌
- 프로그램 인덱스 (요약)                                ❌
- Profile 기반 차등 주입                                ❌
```

### 1.4 진단 요약

| 문제 | 심각도 |
|------|--------|
| Kernel API의 80%가 Tool로 노출 안 됨 | 🔴 Critical |
| Tool이 커널을 우회하고 내부 모듈 직접 참조 | 🔴 Critical |
| SKILL.md 전체 주입 → 토큰 폭발 | 🟡 High |
| OS 구조/룰 안내 없음 | 🟡 High |
| Profile 기반 차등 없음 | 🟡 High |
| 프로그램 사용량 추적/최적화 없음 | 🟢 Medium |

---

## 2. 목표 구조

### 2.1 아키텍처 다이어그램

```
┌─────────────────────────────────────────────────────────────────┐
│                         Agent (LLM)                             │
│                                                                 │
│  "space_list 써볼까?" → tool_call(space, {action:"list"})       │
│  "DB 스키마 확인"     → read(sqlite-explorer/SKILL.md)         │
│                        → exec("sqlite-explorer schema ...")     │
└──────────────────────────┬──────────────────────────────────────┘
                           │
              ┌────────────▼────────────┐
              │     Tool Registry       │
              │                         │
              │  Profile에 따라         │
              │  다른 tool set 노출     │
              │                         │
              │  Worker:    core tools  │
              │  Standard:  + memory    │
              │  Operator:  + OS tools  │
              │  Supervisor: + admin    │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │    Capability Index     │
              │                         │
              │  항상 system prompt에   │
              │  주입되는 capability    │
              │  요약 목록              │
              │  (tool + program + OS)  │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │   KernelHandle          │
              │   (syscall table)       │
              │                         │
              │   모든 Tool이           │
              │   KernelHandle만 참조   │
              └────────────┬────────────┘
                           │
              ┌────────────▼────────────┐
              │   Kernel Modules        │
              │   Supervisor, Space,    │
              │   Memory, A2A, ...      │
              └─────────────────────────┘
```

### 2.2 핵심 원칙

| 원칙 | 설명 | 비판 |
|------|------|------|
| **Single Path** | 모든 OS 제어는 KernelHandle을 경유 | KernelHandle이 God Object가 될 위험 |
| **Profile-based Visibility** | 역할에 따라 tool set 차등 노출 | 4개 profile로 충분한가? |
| **Index + read** | capability 요약은 항상, 상세는 필요 시 read | read가 느리면 에이전트가 안 읽음 |
| **Token Budget** | system prompt에 토큰 예산 적용 | 예산 책정 기준이 불명확 |
| **Usage Tracking** | 프로그램 사용 빈도 추적 → 자동 최적화 | 개인정보/감시 이슈 가능 |

---

## 3. KernelHandle 리팩토링

### 3.1 현재 8 → 목표 11 도메인

```rust
pub struct KernelHandle {
    // ── 기존 (8) ──
    pub state: StateApi,       // 데이터 영속화, 세션
    pub agents: AgentApi,      // 에이전트 생명주기, 예산, 메모리
    pub security: SecurityApi, // 인증, 감사, RBAC
    pub persona: PersonaApi,   // 페르소나 관리
    pub extensions: ExtensionApi, // 프로그램, 스킬, 호스트 도구
    pub mcp: McpApi,           // MCP 서버 브릿지
    pub infra: InfraApi,       // Git, 스케줄러, 크론, 리소스, 이벤트
    pub spaces: SpaceApi,      // Space 관리, 지식 흐름

    // ── 추가 (3) ──
    pub exec: ExecApi,         // 실행 (shell + structured + RBAC)
    pub browser: BrowserApi,   // 브라우저 (navigate, click, type, evaluate, text, html)
    pub a2a: A2aApi,           // 에이전트 간 통신 (delegate, send, query)
}
```

### 3.2 추가 API 정의

**ExecApi** — ExecConfig + AccessManager 통합:

```rust
pub struct ExecApi {
    config: Arc<ExecConfig>,
    access_manager: Arc<Mutex<AccessManager>>,
}

impl ExecApi {
    /// Shell 명령 실행. RBAC 검사 포함.
    pub async fn shell(&self, command: &str, agent_id: &str) -> Result<ExecResult> {
        let am = self.access_manager.lock();
        am.validate_access(agent_id, command)?;
        // timeout, sandbox 적용
        execute_shell(command, self.config.max_timeout_secs).await
    }

    /// 구조화된 명령 실행. binary allowlist + 메타문자 차단.
    pub async fn structured(&self, binary: &str, args: &[String]) -> Result<ExecResult> {
        validate_binary(binary, &self.config.allowed_commands)?;
        validate_args(args)?; // 메타문자 차단
        execute_structured(binary, args).await
    }

    /// 경로 접근 권한 확인.
    pub fn validate_path(&self, agent_id: &str, path: &Path) -> Result<()> {
        self.access_manager.lock().validate_path(agent_id, path)
    }
}
```

**BrowserApi** — OxibrowserBackend 통합:

```rust
pub struct BrowserApi {
    backend: Arc<Mutex<Option<Arc<dyn BrowserBackend>>>>,
}

impl BrowserApi {
    /// URL로 이동.
    pub async fn navigate(&self, url: &str) -> Result<PageInfo> { ... }
    /// CSS 셀렉터로 클릭.
    pub async fn click(&self, selector: &str) -> Result<()> { ... }
    /// 텍스트 입력.
    pub async fn r#type(&self, selector: &str, text: &str) -> Result<()> { ... }
    /// JavaScript 평가.
    pub async fn evaluate(&self, js: &str) -> Result<Value> { ... }
    /// 페이지 텍스트 추출.
    pub async fn text(&self) -> Result<String> { ... }
    /// 페이지 HTML 추출.
    pub async fn html(&self) -> Result<String> { ... }
    /// CSS 셀렉터로 요소 텍스트 조회.
    pub async fn query_all(&self, selector: &str) -> Result<Vec<String>> { ... }
    /// 페이지 제목.
    pub async fn title(&self) -> Result<String> { ... }
    /// 브라우저 종료.
    pub async fn close(&self) -> Result<()> { ... }
}
```

**A2aApi** — A2AProtocol 통합:

```rust
pub struct A2aApi {
    protocol: Arc<A2AProtocol>,
}

impl A2aApi {
    /// 작업 위임.
    pub async fn delegate(&self, task: TaskSpec, from: &AgentId) -> Result<DelegationResult> { ... }
    /// 메시지 전송.
    pub async fn send(&self, message: A2AMessage, from: &AgentId) -> Result<()> { ... }
    /// 역량 조회.
    pub async fn query(&self, capability: &str) -> Result<Vec<AgentCard>> { ... }
}
```

### 3.3 비판: KernelHandle이 God Object가 되는가?

**우려:** 11개 도메인 API를 가진 KernelHandle이 너무 커진다.

**반론:** KernelHandle은 **파사드(facade)** 이지 God Object가 아니다.
각 API는 자신의 내부 모듈만 참조하고, KernelHandle은 단지 그것들을
한 곳에 모아놓은 컨테이너 역할이다. 유닉스의 시스템 콜 테이블이
수백 개의 시스템 콜을 가지고 있어도 God Object가 아닌 것과 같다.

**진짜 위험:** KernelHandle에 cross-domain 비즈니스 로직이 들어가면
그때 God Object가 된다. `save_and_commit()` 같은 편의 메서드는
경계선이다. 최소한으로 유지해야 한다.

---

## 4. Tool 재구성

### 4.1 디렉토리 구조

```
crates/oxios-kernel/src/tools/
├── mod.rs                    (공개 인터페이스)
├── profile.rs                (ToolProfile + 등록 로직)
├── capability_index.rs       (Index 자동 생성)
├── kernel_manifest.rs        (Manifest 자동 생성)
├── usage_tracker.rs          (프로그램 사용량 추적)
│
├── builtin/                  (KernelHandle 독립, 항상 활성)
│   └── (file ops, web_search은 oxi-agent에서 제공)
│
└── kernel/                   (KernelHandle wrapper)
    ├── exec_tool.rs          → KernelHandle.exec
    ├── browser_tool.rs       → KernelHandle.browser
    ├── memory_tools.rs       → KernelHandle.agents (memory)
    ├── space_tool.rs         → KernelHandle.spaces
    ├── agent_tool.rs         → KernelHandle.agents (lifecycle)
    ├── persona_tool.rs       → KernelHandle.persona
    ├── program_tool.rs       → KernelHandle.extensions
    ├── cron_tool.rs          → KernelHandle.infra (cron)
    ├── git_tool.rs           → KernelHandle.infra (git)
    ├── resource_tool.rs      → KernelHandle.infra (resource)
    ├── budget_tool.rs        → KernelHandle.agents (budget)
    ├── security_tool.rs      → KernelHandle.security
    ├── a2a_tool.rs           → KernelHandle.a2a
    ├── mcp_tool.rs           → KernelHandle.mcp
    └── event_tool.rs         → KernelHandle.infra (events)
```

### 4.2 Tool의 통일된 패턴

모든 KernelTool이 동일한 생성 패턴을 따름:

```rust
/// Space 관리 tool. KernelHandle.spaces의 AgentTool wrapper.
pub struct SpaceTool {
    spaces: Arc<SpaceApi>,
}

impl SpaceTool {
    /// KernelHandle에서 SpaceApi를 추출하여 생성.
    pub fn from_kernel(kernel: &KernelHandle) -> Self {
        Self { spaces: Arc::new(kernel.spaces.clone()) }
    }
}

#[async_trait]
impl AgentTool for SpaceTool {
    fn name(&self) -> &str { "space" }

    fn description(&self) -> &'static str {
        "Manage Oxios work spaces. \
         Actions: list, get, create, archive, merge, restore"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "create", "archive", "merge", "restore"],
                    "description": "Space action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Space ID (for get, archive, merge, restore)"
                },
                "name": {
                    "type": "string",
                    "description": "Space name (for create)"
                },
                "absorbed_id": {
                    "type": "string",
                    "description": "Space ID to merge into current (for merge)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: Value,
        _signal: Option<oneshot::Receiver<()>>,
    ) -> Result<AgentToolResult, String> {
        let action = params["action"].as_str().ok_or("missing action")?;
        match action {
            "list" => {
                let spaces = self.spaces.list_spaces();
                let output = spaces.iter()
                    .map(|s| format!("- {} ({}) {}",
                        s.name, s.id,
                        if s.active { "← active" } else { "" }))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(AgentToolResult::success(output))
            }
            "archive" => {
                let id = params["id"].as_str().ok_or("missing id")?;
                self.spaces.archive(id).await.map_err(|e| e.to_string())?;
                Ok(AgentToolResult::success(format!("Space {} archived", id)))
            }
            // ... get, create, merge, restore
            other => Err(format!("Unknown action: {}", other)),
        }
    }
}
```

### 4.3 기존 Tool 마이그레이션 계획

| 기존 Tool | 현재 참조 | 마이그레이션 후 |
|-----------|----------|----------------|
| ExecTool | ExecConfig + AccessManager 직접 | KernelHandle.exec |
| BrowserTool | OxibrowserBackend 직접 | KernelHandle.browser |
| MemoryReadTool | MemoryManager 직접 | KernelHandle.agents (memory methods) |
| MemoryWriteTool | MemoryManager 직접 | KernelHandle.agents |
| MemorySearchTool | MemoryManager 직접 | KernelHandle.agents |
| A2aDelegateTool | A2AProtocol 직접 | KernelHandle.a2a |
| A2aSendTool | A2AProtocol 직접 | KernelHandle.a2a |
| A2aQueryTool | A2AProtocol 직접 | KernelHandle.a2a |
| McpToolWrapper | McpBridge 직접 | KernelHandle.mcp |
| ProgramTool | ExecTool 직접 | KernelHandle.exec (직접 실행) |

**ProgramTool의 변경:** 더 이상 별도 AgentTool로 등록하지 않음.
Capability Index에 명령 경로를 보여주고, 에이전트가 exec으로 직접 실행.
이유: 프로그램 50개면 tool schema만 수천 토큰. exec 하나로 충분.

### 4.4 비판: Tool이 너무 많아지지 않는가?

**우려:** 15개 이상의 OS tool + file ops + web search = LLM에게 너무 많은 tool.

**현실:** LLM은 tool 이름 + 설명만 보고 적절한 tool을 선택한다.
GPT-4, Claude 등은 수십 개의 tool에서도 잘 선택한다.
OpenClaw는 이미 30+ tool을 등록해서 사용 중이다.

**진짜 한계:** tool 수가 아니라, **tool schema의 총 토큰**이 문제다.
각 tool의 schema가 평균 100 토큰이면 20개 tool = 2,000 토큰.
이건 감당 가능하다. 100개 tool = 10,000 토큰이면 문제.

**해결:** Profile로 tool 수를 제한. Worker는 10개 이내.

---

## 5. ToolProfile

### 5.1 정의

```rust
/// 에이전트의 OS 역할. KernelHandle의 어떤 API에 접근할 수 있는지 결정.
///
/// 이것은 "제한"이 아니라 "역할에 맞는 tool 제공"이다.
/// Unix에서 root와 user가 다른 syscall을 쓰는 것과 같다.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolProfile {
    /// 기본 작업자. 코딩, 검색, 브라우징.
    /// OS를 제어할 필요 없는 순수 작업 에이전트.
    #[default]
    Worker,
    /// 기억하는 작업자. 메모리 접근 포함.
    /// 이전 대화, 관련 지식 참조가 필요한 작업.
    Standard,
    /// OS 제어자. space, agent, program, a2a 등 OS 수준 tool 접근.
    /// 다른 에이전트를 조율하거나 OS를 관리.
    Operator,
    /// 시스템 관리자. 전체 OS 제어.
    /// 보안, 예산, 리소스, 감사.
    Supervisor,
}
```

### 5.2 Profile × Tool 매트릭스

```
                                  Worker  Standard  Operator  Supervisor
──────────────────────────────────────────────────────────────────────────
exec                              ✅      ✅        ✅        ✅
browser                           ✅      ✅        ✅        ✅
web_search                        ✅      ✅        ✅        ✅
memory_read, memory_search                ✅        ✅        ✅
memory_write                                       ✅        ✅
space (list, get, create, archive...)              ✅        ✅
agent (list, kill)                                 ✅        ✅
a2a (delegate, send, query)                        ✅        ✅
mcp                                                ✅        ✅
persona (list, set_active)                         ✅        ✅
program (install, uninstall, enable, disable)      ✅        ✅
cron (add, remove, list, trigger)                            ✅
git (log, tag, restore)                                     ✅
security (audit, verify_chain)                              ✅
budget (check, set, reserve)                                ✅
resource (snapshot, history)                                ✅
```

### 5.3 Profile 결정

```rust
fn resolve_profile(seed: &Seed, config: &OxiosConfig) -> ToolProfile {
    // 1순위: Seed에 명시적 지정
    if let Some(ref profile) = seed.tool_profile {
        return profile.clone();
    }

    // 2순위: 활성 Persona의 기본 profile
    if let Some(ref persona_id) = config.active_persona_id {
        if let Some(persona) = config.personas.iter().find(|p| &p.id == persona_id) {
            // Persona의 role로 추론
            return match persona.role.as_str() {
                "supervisor" | "admin" => ToolProfile::Supervisor,
                "coordinator" | "operator" | "orchestrator" => ToolProfile::Operator,
                "researcher" | "analyst" => ToolProfile::Standard,
                _ => ToolProfile::Worker,
            };
        }
    }

    // 3순위: 기본값
    ToolProfile::Worker
}
```

### 5.4 Seed 확장

```rust
pub struct Seed {
    pub id: SeedId,
    pub goal: String,
    pub constraints: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub ontology: Vec<Entity>,
    pub created_at: DateTime<Utc>,
    pub generation: u32,
    pub parent_seed_id: Option<SeedId>,

    /// Tool profile for this seed's execution.
    /// Overrides persona default if set.
    #[serde(default)]
    pub tool_profile: Option<ToolProfile>,
}
```

### 5.5 Persona에 Profile 연결

```toml
# oxios.toml

[[persona]]
id = "dev"
name = "Dev"
role = "developer"
# role에서 자동 추론: Worker
# 코딩, 검색, 브라우징만 필요

[[persona]]
id = "research"
name = "Research"
role = "researcher"
# role에서 자동 추론: Standard
# 메모리 검색으로 이전 지식 참조

[[persona]]
id = "orchestrator"
name = "Orchestrator"
role = "orchestrator"
# role에서 자동 추론: Operator
# space, agent, a2a 필요

[[persona]]
id = "admin"
name = "Admin"
role = "admin"
# role에서 자동 추론: Supervisor
# 전체 OS 제어
```

### 5.6 비판: 4개 profile로 충분한가?

**우려:** 미래에 "Worker + cron만" 같은 조합이 필요하면?

**분석:** Profile은 **미리 정의된 닫힌 집합**이다. 확장하려면 enum을 수정해야 한다.
OpenClaw도 4개 profile (minimal, coding, messaging, full)로 하고 있고,
실제로 이 정도면 충분하다. 너무 세밀한 profile은 오히려 관리 부담.

**대안 고려:** Profile을 enum 대신 Set<ToolId>로 하면 무한 확장 가능.
하지만 이건 "모든 에이전트마다 다른 tool set"을 의미하고,
관리가 불가능해진다. 4개 enum이 더 실용적.

**결론:** 4개 profile로 시작. 필요하면 추가. enum 수정은 가벼운 변경.

---

## 6. Capability Index

### 6.1 원칙

```
OpenClaw가 증명한 방식:
  - SKILL.md 전체를 주입하지 않는다
  - name + description + location만 주입한다
  - 에이전트가 필요하면 read tool로 직접 읽는다

Oxios의 확장:
  - program뿐만 아니라 모든 OS capability를 Index에 포함
  - 등록된 tool + 설치된 program + OS 서비스를 통합
```

### 6.2 Index 형식

```xml
<available_capabilities>
  ── OS Tools (registry에 등록된 것) ──
  <capability>
    <name>exec</name>
    <category>os-tool</category>
    <description>Execute shell commands or structured binaries</description>
  </capability>
  <capability>
    <name>browser</name>
    <category>os-tool</category>
    <description>Headless web browser: navigate, click, type, evaluate, text, html</description>
  </capability>

  ── Programs (exec으로 직접 실행 가능) ──
  <capability>
    <name>sqlite-explorer</name>
    <category>program</category>
    <description>Query SQLite databases, inspect schemas</description>
    <command>sqlite-explorer</command>
    <skill>/path/to/sqlite-explorer/SKILL.md</skill>
  </capability>
  <capability>
    <name>github-manager</name>
    <category>program</category>
    <description>GitHub PR, issue, repository management</description>
    <command>gh</command>
    <skill>/path/to/github-manager/SKILL.md</skill>
  </capability>

  ── OS Services (Profile에 따라 노출) ──
  <capability>
    <name>memory</name>
    <category>os-service</category>
    <description>Persistent cross-space memory with semantic search</description>
    <tools>memory_read, memory_search</tools>
  </capability>
  <capability>
    <name>spaces</name>
    <category>os-service</category>
    <description>Work space management: list, create, archive, merge</description>
    <tools>space</tools>
  </capability>
</available_capabilities>

Use the `read` tool to load a program's SKILL.md for detailed usage instructions.
```

### 6.3 자동 생성

```rust
pub fn build_capability_index(
    registry: &ToolRegistry,
    kernel: &KernelHandle,
    profile: &ToolProfile,
) -> String {
    let mut entries = Vec::new();

    // 1. Registry에 등록된 tool
    for name in registry.tool_names() {
        if let Some(tool) = registry.get(&name) {
            entries.push(xml_capability(
                tool.name(),
                "os-tool",
                tool.description(),
                None, None,
            ));
        }
    }

    // 2. Programs (exec으로 실행 가능)
    let rt = tokio::runtime::Handle::current();
    let programs = rt.block_on(async { kernel.extensions.list_programs().await });
    for program in &programs {
        entries.push(xml_capability(
            &program.meta.name,
            "program",
            &program.meta.description,
            Some(&program.meta.name), // command
            Some(&program.skill_path().display().to_string()), // skill path
        ));
    }

    // 3. OS Services (profile에 따라)
    if *profile >= ToolProfile::Standard {
        entries.push(xml_capability(
            "memory", "os-service",
            "Persistent cross-space memory with semantic search",
            None, None,
        ));
    }
    if *profile >= ToolProfile::Operator {
        for (name, desc) in &[
            ("spaces", "Work space management: list, create, archive, merge"),
            ("agents", "Agent lifecycle: list, kill, budget"),
            ("a2a", "Inter-agent communication: delegate, send, query"),
            ("persona", "Persona management: list, set active"),
            ("programs", "Program management: install, uninstall, enable, disable"),
        ] {
            entries.push(xml_capability(name, "os-service", desc, None, None));
        }
    }

    format_index(entries)
}
```

### 6.4 토큰 비용 추정

| 항목 | 단위 토큰 | Worker | Operator |
|------|----------|--------|----------|
| OS tools (5~10개) | ~15 | ~75 | ~150 |
| Programs (50개) | ~20 | ~1,000 | ~1,000 |
| OS services (0~5개) | ~15 | 0 | ~75 |
| 래퍼 텍스트 | ~50 | ~50 | ~50 |
| **총 Index 토큰** | | **~1,125** | **~1,275** |

SKILL.md 전체 주입(50개 × 평균 500토큰 = 25,000토큰) 대비 **~5%**.

### 6.5 비판: 에이전트가 read를 안 부르면?

**우려:** Index에 요약만 있고, 에이전트가 SKILL.md를 read하지 않으면
프로그램을 제대로 못 씀.

**현실:**
1. LLM은 tool의 name + description으로도 상당히 잘 추론한다.
   "sqlite-explorer — Query SQLite databases"면 대충 `exec("sqlite-explorer ...")`를 시도.
2. 실패하면 그때 SKILL.md를 read. 자연스러운 피드백 루프.
3. OpenClaw가 이 방식으로 이미 수천 명의 사용자에게 작동 중.

**진짜 위험:** 프로그램의 CLI 인터페이스가 직관적이지 않으면
SKILL.md를 읽어도 못 씀. 이건 프로그램 설계 문제지 아키텍처 문제가 아님.

---

## 7. Kernel Manifest

### 7.1 원칙

```
에이전트가 OS를 제어하려면 OS의 구조와 룰을 알아야 한다.
하지만 모든 에이전트가 OS를 제어하는 건 아니다.

Worker에게 OS 룰을 가르칠 필요 없다.
Operator에게만 가르친다.
```

### 7.2 Manifest 내용

```markdown
## Oxios Agent OS

You are running inside Oxios, an Agent Operating System.

### Spaces
Work is organized into isolated Spaces. Each space has:
- Workspace directory (file isolation)
- Memory store (scoped knowledge)
- State store (persistent state)
Use `space` tool: list, get, create, archive, merge, restore.
Memory is scoped to the current space unless specified otherwise.

### Agents
Other agents exist in this OS.
Use `agent` tool: list, kill.
Use `a2a` tool: delegate tasks, send messages, query capabilities.
Agent lifecycle: fork → exec → wait → kill.

### Programs
Programs are OS-level installable capabilities.
Read the Capability Index for available programs.
Use `read` to load SKILL.md for usage details.
Use `exec` to run program commands.

### Security (Supervisor only)
All operations are audit-logged.
Use `security` tool: verify audit chain, query log.
Use `budget` tool: check/set token budgets per agent.
Use `resource` tool: monitor CPU, memory, disk.

### Ouroboros Protocol
All work follows: Interview → Seed → Execute → Evaluate → Evolve.
```

### 7.3 자동 생성

```rust
pub fn build_kernel_manifest(profile: &ToolProfile) -> Option<String> {
    if *profile < ToolProfile::Operator {
        return None;
    }

    let mut m = String::from(
        "## Oxios Agent OS\n\n\
         You are running inside Oxios, an Agent Operating System.\n\n"
    );

    m.push_str(
        "### Spaces\n\
         Work is organized into isolated Spaces. Each space has:\n\
         - Workspace directory (file isolation)\n\
         - Memory store (scoped knowledge)\n\
         - State store (persistent state)\n\
         Use `space` tool: list, get, create, archive, merge, restore.\n\
         Memory is scoped to the current space unless specified otherwise.\n\n"
    );

    m.push_str(
        "### Agents\n\
         Other agents exist in this OS.\n\
         Use `agent` tool: list, kill.\n\
         Use `a2a` tool: delegate tasks, send messages, query capabilities.\n\
         Agent lifecycle: fork → exec → wait → kill.\n\n"
    );

    m.push_str(
        "### Programs\n\
         Programs are OS-level installable capabilities.\n\
         Read the Capability Index for available programs.\n\
         Use `read` to load SKILL.md for usage details.\n\
         Use `exec` to run program commands.\n\n"
    );

    if *profile >= ToolProfile::Supervisor {
        m.push_str(
            "### Security\n\
             All operations are audit-logged.\n\
             Use `security` tool: verify audit chain, query log.\n\
             Use `budget` tool: check/set token budgets per agent.\n\
             Use `resource` tool: monitor CPU, memory, disk.\n\n"
        );
    }

    m.push_str(
        "### Ouroboros Protocol\n\
         All work follows: Interview → Seed → Execute → Evaluate → Evolve.\n"
    );

    Some(m)
}
```

---

## 8. Usage Tracker — 프로그램 사용량 추적

### 8.1 문제

프로그램이 100개 설치되어 있으면:
- Index에 100개 capability → ~2,000 토큰 (여전히 OK)
- 하지만 100개 중 5개만 쓰이고 95개는 안 쓰임
- 안 쓰이는 건 Index에서 숨기면 토큰 절약
- 근데 "숨기면" 에이전트가 그 존재를 모름

### 8.2 설계

```rust
/// 프로그램 사용 빈도 추적.
/// exec tool이 실행될 때마다 기록.
pub struct UsageTracker {
    /// program_name → 호출 횟수
    counts: Arc<Mutex<HashMap<String, u64>>>,
    /// 추적 시작 시간
    since: Instant,
}

impl UsageTracker {
    /// exec tool에서 호출. 프로그램 사용 기록.
    pub fn record(&self, program_name: &str) {
        let mut counts = self.counts.lock();
        *counts.entry(program_name.to_string()).or_insert(0) += 1;
    }

    /// 사용량 보고서.
    pub fn report(&self) -> UsageReport {
        let counts = self.counts.lock();
        let total: u64 = counts.values().sum();
        let mut entries: Vec<_> = counts.iter()
            .map(|(name, count)| UsageEntry {
                name: name.clone(),
                count: *count,
                ratio: if total > 0 { *count as f64 / total as f64 } else { 0.0 },
            })
            .collect();
        entries.sort_by(|a, b| b.count.cmp(&a.count));

        UsageReport {
            entries,
            total_calls: total,
            since: self.since,
        }
    }

    /// Index에 포함할 프로그램 목록 선별.
    /// 항상 포함: 최근 N회 이상 사용.
    /// 조건부: 최근 M회 미만 → "거의 안 씀" 표시.
    pub fn classify(&self, threshold: u64) -> (Vec<String>, Vec<String>) {
        let counts = self.counts.lock();
        let mut active = Vec::new();
        let mut dormant = Vec::new();
        for (name, count) in counts.iter() {
            if *count >= threshold {
                active.push(name.clone());
            } else {
                dormant.push(name.clone());
            }
        }
        (active, dormant)
    }
}
```

### 8.3 Index에서 활용

```rust
fn build_capability_index(/* ... */) -> String {
    // ...

    // Programs: 사용량 기반 분류
    let (active, dormant) = usage_tracker.classify(5);

    for program in &programs {
        let is_active = active.contains(&program.meta.name);
        let is_dormant = dormant.contains(&program.meta.name);

        if is_dormant {
            // Index에 포함하되 "거의 안 씀" 표시
            entries.push(xml_capability(
                &program.meta.name,
                "program",
                &format!("{} (rarely used)", program.meta.description),
                // ...
            ));
        } else {
            entries.push(xml_capability(
                &program.meta.name,
                "program",
                &program.meta.description,
                // ...
            ));
        }
    }
}
```

### 8.4 자동 최적화 제안

```rust
/// Supervisor 에이전트나 CLI에서 호출.
/// 사용량이 낮은 프로그램의 disable을 제안.
pub fn suggest_optimizations(&self, programs: &[Program]) -> Vec<OptimizationSuggestion> {
    let (_, dormant) = self.classify(3);
    dormant.iter()
        .filter_map(|name| {
            programs.iter().find(|p| p.meta.name == *name)
        })
        .map(|p| OptimizationSuggestion {
            program: p.meta.name.clone(),
            suggestion: format!(
                "'{}' has not been used recently. \
                 Consider disabling to reduce context overhead.",
                p.meta.name
            ),
        })
        .collect()
}
```

### 8.5 비판: 추적이 오버엔지니어링인가?

**우려:** 프로그램 100개의 사용량을 추적하는 건 오버엔지니어링.
Index에 100개 capability가 ~2,000 토큰이면 그냥 다 넣으면 되지 않나?

**현실:**
- 100개: ~2,000 토큰 → OK
- 500개: ~10,000 토큰 → 위험
- 1000개: ~20,000 토큰 → 불가능

**결론:** 당장은 필요 없음. 프로그램이 100개 이하면 Index에 전부 포함.
UsageTracker는 인프라만 준비해두고, 프로그램이 200개를 넘을 때 활성화.

---

## 9. System Prompt 조립

### 9.1 전체 구조

```
System Prompt:
  ┌──────────────────────────────────────────┐
  │ 1. Core Identity (1줄)                   │  항상
  ├──────────────────────────────────────────┤
  │ 2. Seed (goal, constraints, criteria)    │  항상
  ├──────────────────────────────────────────┤
  │ 3. Persona (활성 persona의 system_prompt)│  항상 (있으면)
  ├──────────────────────────────────────────┤
  │ 4. Capability Index                      │  항상
  │    (tool + program + os-service 요약)    │
  ├──────────────────────────────────────────┤
  │ 5. Kernel Manifest                       │  Operator+
  │    (OS 구조, Space, Agent, A2A, ...)     │
  └──────────────────────────────────────────┘
```

### 9.2 구현

```rust
fn build_system_prompt(
    seed: &Seed,
    persona_prompt: Option<&str>,
    capability_index: &str,
    kernel_manifest: Option<&str>,
) -> String {
    let mut prompt = String::new();

    // 1. Core Identity
    prompt.push_str("You are an autonomous agent.\n\n");

    // 2. Seed
    prompt.push_str(&format!("## Goal\n{}\n", seed.goal));
    if !seed.constraints.is_empty() {
        prompt.push_str("\n## Constraints\n");
        for (i, c) in seed.constraints.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, c));
        }
    }
    if !seed.acceptance_criteria.is_empty() {
        prompt.push_str("\n## Acceptance Criteria\n");
        for (i, c) in seed.acceptance_criteria.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, c));
        }
    }
    if !seed.ontology.is_empty() {
        prompt.push_str("\n## Domain Entities\n");
        for e in &seed.ontology {
            prompt.push_str(&format!(
                "- **{}** ({}): {}\n",
                e.name, e.entity_type, e.description
            ));
        }
    }

    // 3. Persona
    if let Some(pp) = persona_prompt {
        prompt.push_str(&format!("\n## Persona\n{}\n", pp));
    }

    // 4. Capability Index (항상)
    prompt.push_str("\n## Available Capabilities\n");
    prompt.push_str(capability_index);
    prompt.push_str(
        "\nUse the `read` tool to load a program's SKILL.md \
         for detailed usage instructions.\n"
    );

    // 5. Kernel Manifest (Operator+)
    if let Some(manifest) = kernel_manifest {
        prompt.push_str("\n");
        prompt.push_str(manifest);
    }

    prompt
}
```

### 9.3 Profile별 System Prompt 크기 추정

| Profile | 포함 섹션 | 추정 토큰 |
|---------|----------|----------|
| Worker | Seed + Persona + Index | ~2,000 |
| Standard | + Memory 안내 | ~2,200 |
| Operator | + Kernel Manifest | ~3,000 |
| Supervisor | + Security 안내 | ~3,500 |

---

## 10. Tool 등록 통합

### 10.1 register_tools()

```rust
/// Profile에 따라 tool을 선택적으로 등록.
/// 모든 tool이 KernelHandle만 참조.
pub fn register_tools(
    registry: &ToolRegistry,
    kernel: &KernelHandle,
    profile: &ToolProfile,
    usage_tracker: Option<Arc<UsageTracker>>,
) {
    // ════════════════════════════════════════════════
    //  Always-on: 파일 작업 (oxi-agent 제공)
    // ════════════════════════════════════════════════
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GrepTool::new());
    registry.register(FindTool::new());
    registry.register(LsTool::new());

    // ════════════════════════════════════════════════
    //  Always-on: KernelHandle 경유
    // ════════════════════════════════════════════════
    registry.register(ExecTool::from_kernel(kernel));
    registry.register(BrowserTool::from_kernel(kernel));
    registry.register(WebSearchTool::new(search_cache));

    // ════════════════════════════════════════════════
    //  Standard+: 메모리
    // ════════════════════════════════════════════════
    if *profile >= ToolProfile::Standard {
        registry.register(MemoryReadTool::from_kernel(kernel));
        registry.register(MemorySearchTool::from_kernel(kernel));
    }

    // ════════════════════════════════════════════════
    //  Operator+: OS 제어
    // ════════════════════════════════════════════════
    if *profile >= ToolProfile::Operator {
        registry.register(MemoryWriteTool::from_kernel(kernel));
        registry.register(SpaceTool::from_kernel(kernel));
        registry.register(AgentTool::from_kernel(kernel));
        registry.register(A2aTool::from_kernel(kernel));
        registry.register(PersonaTool::from_kernel(kernel));
        registry.register(ProgramTool::from_kernel(kernel));
        registry.register(McpTool::from_kernel(kernel));
    }

    // ════════════════════════════════════════════════
    //  Supervisor: 시스템 관리
    // ════════════════════════════════════════════════
    if *profile >= ToolProfile::Supervisor {
        registry.register(SecurityTool::from_kernel(kernel));
        registry.register(BudgetTool::from_kernel(kernel));
        registry.register(ResourceTool::from_kernel(kernel));
        registry.register(CronTool::from_kernel(kernel));
        registry.register(GitTool::from_kernel(kernel));
        registry.register(AuditTool::from_kernel(kernel));
        registry.register(EventTool::from_kernel(kernel));
    }
}
```

### 10.2 AgentRuntime.execute() 변경

```rust
fn execute(&self, seed: &Seed) -> Result<ExecuteResult> {
    let kernel = self.kernel_handle.as_ref()
        .context("KernelHandle not available")?;

    // 1. Profile 결정
    let profile = resolve_profile(seed, &self.config);

    // 2. Tool 등록 (Profile 기반)
    let registry = ToolRegistry::new();
    register_tools(&registry, kernel, &profile, self.usage_tracker.clone());

    // 3. Capability Index 생성
    let index = build_capability_index(&registry, kernel, &profile);

    // 4. Kernel Manifest 생성
    let manifest = build_kernel_manifest(&profile);

    // 5. System Prompt 조립
    let persona_prompt = self.persona_manager
        .as_ref()
        .and_then(|pm| pm.active_system_prompt());
    let system_prompt = build_system_prompt(
        seed, persona_prompt.as_deref(),
        &index, manifest.as_deref(),
    );

    // 6. Memory blend
    let system_prompt = if let Some(ref mm) = self.memory_manager {
        mm.blend_into_prompt(&mm.recall(&seed.goal).await?, &system_prompt)
    } else {
        system_prompt
    };

    // 7. AgentLoop 실행
    let agent_loop = AgentLoop::new(AgentLoopConfig {
        provider: self.provider.clone(),
        model_id: self.config.model_id.clone(),
        system_prompt,
        registry,
        max_iterations: self.config.max_iterations,
        ..
    });

    agent_loop.run(seed.goal.clone()).await
}
```

---

## 11. 에이전트 시나리오

### 11.1 코딩 에이전트 (Worker)

```
Seed: "이 함수 리팩토링해줘"
Profile: Worker
Persona: Dev

Tool Registry (10개):
  read, write, edit, grep, find, ls,
  exec, browser, web_search, get_search_results

System Prompt:
  Seed(goal, constraints)
  Persona("pragmatic developer")
  Capability Index (tool 10개 + program 50개 요약)
  Kernel Manifest: ❌

에이전트 행동:
  grep으로 함수 찾기 → read로 읽기 → edit으로 수정
  필요하면 browser로 문서 확인

토큰 오버헤드: ~2,000 (prompt만)
```

### 11.2 소설 작성 에이전트 (Worker)

```
Seed: "판타지 소설 한 챕터 써줘"
Profile: Worker
Persona: Novelist (custom)

Tool Registry (10개):
  read, write, edit, grep, find, ls,
  exec, browser, web_search, get_search_results

System Prompt:
  Seed(goal: "판타지 소설 작성", constraints: "2000자 이상")
  Persona("creative novelist")
  Capability Index (tool 10개 + program 50개 요약)
  Kernel Manifest: ❌

에이전트 행동:
  write로 소설 작성 → edit으로 다듬기
  필요하면 browser로 참고 자료 검색
  OS 제어 불필요. OS의 존재조차 모름.

토큰 오버헤드: ~2,000
```

### 11.3 이전 작업 기반 코딩 (Standard)

```
Seed: "저번에 논의한 아키텍처 적용해"
Profile: Standard
Persona: Dev

Tool Registry (12개):
  Worker 10개 + memory_read, memory_search

System Prompt:
  Seed + Persona + Index + Memory 안내
  Kernel Manifest: ❌

에이전트 행동:
  memory_search("아키텍처 논의") → 관련 기억 발견
  memory_read로 상세 확인
  코딩 작업
```

### 11.4 OS 제어 에이전트 (Operator)

```
Seed: "팀에게 작업 분배하고 결과 취합해"
Profile: Operator
Persona: Orchestrator

Tool Registry (~20개):
  Worker 10개 + memory + space, agent, a2a, persona,
  program, mcp

System Prompt:
  Seed + Persona + Index + Kernel Manifest
  (Space, Agent, A2A, Program 설명 포함)

에이전트 행동:
  Kernel Manifest에서 OS 구조 인지
  space list → 현재 space 확인
  a2a delegate → dev 에이전트에게 작업 위임
  a2a delegate → review 에이전트에게 리뷰 위임
  a2a query → 결과 수집
  memory_write → 결과 저장

토큰 오버헤드: ~3,000
```

### 11.5 시스템 관리자 (Supervisor)

```
Seed: "시스템 상태 점검하고 이상 감지해"
Profile: Supervisor
Persona: Admin

Tool Registry (전체 ~25개):
  전부

System Prompt:
  Seed + Persona + Index + Kernel Manifest (Security 포함)

에이전트 행동:
  resource snapshot → CPU/메모리/디스크 확인
  security verify_chain → 감사 체인 무결성 확인
  budget check → 에이전트별 토큰 사용량 확인
  agent list → 에이전트 상태 확인
  audit query → 최근 감사 로그 확인

토큰 오버헤드: ~3,500
```

---

## 12. Before vs After

### 12.1 아키텍처 비교

```
Before:
  Agent → MemoryTool → MemoryManager (직접)         ❌ 커널 우회
  Agent → ExecTool → ExecConfig (직접)              ❌ 커널 우회
  Agent → BrowserTool → OxibrowserBackend (직접)    ❌ 커널 우회
  Agent → space_list ???                             ❌ 없음
  Agent → agent_kill ???                             ❌ 없음
  Agent → cron_add ???                               ❌ 없음
  SKILL.md 50개 전체 주입                            ❌ 토큰 폭발
  OS 구조 안내 없음                                  ❌

After:
  Agent → 모든 Tool → KernelHandle → Module         ✅ 단일 경로
  Agent → space → KernelHandle.spaces → SpaceManager ✅
  Agent → agent → KernelHandle.agents → Supervisor   ✅
  Agent → cron → KernelHandle.infra → CronScheduler  ✅
  SKILL.md 요약만 주입 (Index)                       ✅ ~1,000 토큰
  OS 구조 안내 (Manifest) Profile 기반               ✅
```

### 12.2 메트릭 비교

| 메트릭 | Before | After |
|--------|--------|-------|
| Kernel API → Tool 노출 | 20% | 100% |
| Tool → KernelHandle 경유 | 0% | 100% |
| 프로그램 50개 prompt | ~25,000 토큰 | ~1,000 토큰 |
| 코딩 에이전트 prompt | ~25,000 토큰 | ~2,000 토큰 |
| OS 제어 에이전트 prompt | ~25,000 토큰 | ~3,000 토큰 |
| 새 capability 추가 | runtime 코드 수정 | Index에 자동 등록 |
| OS 구조 전달 | 없음 | Profile 기반 선택적 |
| 프로그램 사용량 추적 | 없음 | UsageTracker |

---

## 13. 구현 로드맵

```
Phase 1: KernelHandle 확장                     (기반 인프라)
  ├── ExecApi 추가
  ├── BrowserApi 추가
  ├── A2aApi 추가
  └── kernel.rs에서 11개 Facade 조립

Phase 2: 기존 Tool 마이그레이션                 (KernelHandle 경유)
  ├── ExecTool → KernelHandle.exec
  ├── BrowserTool → KernelHandle.browser
  ├── MemoryTools → KernelHandle.agents
  ├── A2aTools → KernelHandle.a2a
  └── McpTool → KernelHandle.mcp

Phase 3: 신규 OS Tool 추가                      (에이전트 OS 제어)
  ├── SpaceTool → KernelHandle.spaces
  ├── AgentTool → KernelHandle.agents
  ├── PersonaTool → KernelHandle.persona
  ├── ProgramTool → KernelHandle.extensions
  ├── CronTool → KernelHandle.infra
  ├── GitTool → KernelHandle.infra
  ├── SecurityTool → KernelHandle.security
  ├── BudgetTool → KernelHandle.agents
  └── ResourceTool → KernelHandle.infra

Phase 4: Profile + Index + Manifest             (컨텍스트 최적화)
  ├── ToolProfile enum + resolve_profile()
  ├── register_tools() (Profile 기반)
  ├── build_capability_index() (자동 생성)
  ├── build_kernel_manifest() (자동 생성)
  ├── build_system_prompt() (통합)
  └── Seed.tool_profile 필드 추가

Phase 5: Usage Tracker                          (향후 최적화)
  └── UsageTracker 인프라 준비 (당장은 비활성)
```

---

## 14. 자기비판

### 이 설계의 약점

| 약점 | 심각도 | 대응 |
|------|--------|------|
| KernelHandle이 11개 도메인 → 파사드 비대 | 🟡 | 각 API는 독립적. 파사드는 thin. |
| 4개 Profile이 닫힌 집합 | 🟡 | 실용적 충분. 필요시 enum 확장. |
| ProgramTool 제거 → exec로만 실행 | 🟡 | 직관적 CLI면 OK. 복잡하면 SKILL.md read. |
| read로 SKILL.md 읽는 게 느릴 수 있음 | 🟢 | 1턴 추가. 성능보다 정확성. |
| UsageTracker 당장 불필요 | 🟢 | 인프라만 준비. |
| 에이전트가 SKILL.md를 안 읽을 수 있음 | 🟢 | LLM이 실패하면 자연스럽게 read. |
| Kernel Manifest 내용이 하드코딩 | 🟡 | OS 룰은 근본적으로 정적. |

### 이 설계가 풀지 못하는 문제

1. **에이전트가 "언제 OS tool을 써야 하는지"를 모름**
   → Kernel Manifest가 안내하지만, LLM의 판단에 의존.
   → 이건 아키텍처 문제가 아니라 LLM 능력의 한계.

2. **Profile 결정이 자동이 아님**
   → Persona의 role로 추론하지만, 100% 정확할 수 없음.
   → 사용자가 명시적으로 지정해야 할 수도 있음.

3. **KernelHandle이 Rust trait이 아니라 concrete struct**
   → 테스트 시 mock이 어려울 수 있음.
   → 각 API를 trait으로 만들면 되지만, 복잡도 증가.

### 개선 가능한 부분 (미래)

1. **Goal 기반 Profile 자동 추론** — Interview 단계에서 goal 분석하여 profile 제안
2. **KernelHandle trait化** — 테스트 용이성을 위해 각 API를 trait으로
3. **Capability Index 벡터 검색** — 프로그램이 200+개면 Index에서도 검색 필요
4. **Dynamic tool activation** — AgentLoop 실행 중에 tool을 동적 추가/제거
