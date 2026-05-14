# Oxios AgentOS — 아키텍처 리팩토링 설계

> KernelHandle → AgentTool 연결 구조 정리 및 진정한 AgentOS를 위한 아키텍처.

## 1. 유닉스 비유

```
Unix:
  User Process → syscall (open, read, fork, kill, ...) → Kernel → Hardware
  프로세스는 커널 내부를 모름. syscall만 앎.

AgentOS:
  Agent → tool_call (space_list, exec, memory_search, ...) → Kernel → Host
  에이전트는 커널 내부를 모름. tool만 앎.
```

KernelHandle = **시스템 콜 테이블**.
모든 OS 제어는 KernelHandle을 거쳐야 함.
AgentTool은 KernelHandle의 얇은 wrapper.

---

## 2. 현재 구조의 문제

```
                    KernelHandle (syscall table)
                    ┌─────────────────────────┐
                    │ state   agents  security │
                    │ persona extensions mcp    │
                    │ infra   spaces            │
                    └──────────────────────────┘
                         ↑          ↑
                    CLI만 씀   Guardian만 씀
                    에이전트는 접근 불가 ❌


에이전트가 쓰는 Tool들:
                    MemoryManager ←──── 직접 참조 (KernelHandle 우회)
                    A2AProtocol   ←──── 직접 참조 (KernelHandle에 없음)
                    ExecConfig    ←──── 직접 참조 (KernelHandle에 없음)
                    Browser       ←──── 직접 참조 (KernelHandle에 없음)
```

**문제점:**
1. Tool이 커널을 우회하고 내부 모듈을 직접 참조
2. Kernel API의 대부분이 Tool로 노출 안 됨 (space, persona, infra, security, extension)
3. 에이전트가 OS를 제어할 방법이 없음

---

## 3. 목표 구조

```
┌──────────────────────────────────────────────────────┐
│                    Agent (LLM)                        │
│                                                      │
│  tool_call: exec, browser_navigate, space_list,      │
│             agent_list, memory_search, cron_add, ...  │
└──────────────────────┬───────────────────────────────┘
                       │
                       │  AgentTool trait (uniform interface)
                       │  name() + description() + schema() + execute()
                       │
┌──────────────────────▼───────────────────────────────┐
│                  Tool Registry                        │
│                                                      │
│  Profile 기반 가시성:                                 │
│  Worker:    file ops, exec, browser, web_search       │
│  Standard:  + memory                                 │
│  Operator:  + space, agent, persona, program, a2a, mcp│
│  Supervisor: + security, budget, resource, cron       │
└──────────────────────┬───────────────────────────────┘
                       │
                       │  모든 tool이 통과하는 단일 경로
                       │
┌──────────────────────▼───────────────────────────────┐
│              KernelHandle (syscall table)              │
│                                                      │
│  .state      save, load, session                     │
│  .agents     list, kill, budget, memory              │
│  .security   audit, verify, rbac, auth               │
│  .persona    list, create, set_active                │
│  .extensions install, uninstall, enable, skills      │
│  .mcp        connect, list_tools, call               │
│  .infra      git, cron, resource, event, config      │
│  .spaces     list, create, archive, merge, restore   │
│  .exec       shell, structured execution             │  ← 추가 필요
│  .browser    navigate, click, type, evaluate         │  ← 추가 필요
│  .a2a        delegate, send, query                   │  ← 추가 필요
└──────────────────────┬───────────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────────┐
│              Kernel 내부 모듈들                        │
│  Supervisor, SpaceManager, MemoryManager, A2A,       │
│  AuditTrail, BudgetManager, CronScheduler, ...       │
└──────────────────────────────────────────────────────┘
```

---

## 4. 리팩토링: 3단계

### Phase 1: KernelHandle 확장

현재 8개 도메인 → 11개 도메인으로 확장.
누락된 exec, browser, a2a를 추가.

```rust
pub struct KernelHandle {
    // 기존 (8)
    pub state: StateApi,
    pub agents: AgentApi,
    pub security: SecurityApi,
    pub persona: PersonaApi,
    pub extensions: ExtensionApi,
    pub mcp: McpApi,
    pub infra: InfraApi,
    pub spaces: SpaceApi,

    // 추가 (3)
    pub exec: ExecApi,       // shell/structured 실행 + RBAC
    pub browser: BrowserApi, // navigate, click, type, evaluate, text, html
    pub a2a: A2aApi,         // delegate, send, query, capabilities
}
```

**ExecApi** — exec_config + access_manager 통합:

```rust
pub struct ExecApi {
    config: Arc<ExecConfig>,
    access_manager: Arc<Mutex<AccessManager>>,
}

impl ExecApi {
    pub async fn shell(&self, command: &str, agent_id: &str) -> Result<ExecResult> { ... }
    pub async fn structured(&self, binary: &str, args: &[String]) -> Result<ExecResult> { ... }
    pub fn validate_access(&self, agent_id: &str, path: &Path) -> Result<()> { ... }
}
```

**BrowserApi** — OxibrowserBackend 통합:

```rust
pub struct BrowserApi {
    backend: Arc<dyn BrowserBackend>,
}

impl BrowserApi {
    pub async fn navigate(&self, url: &str) -> Result<PageInfo> { ... }
    pub async fn click(&self, selector: &str) -> Result<()> { ... }
    pub async fn r#type(&self, selector: &str, text: &str) -> Result<()> { ... }
    pub async fn evaluate(&self, js: &str) -> Result<Value> { ... }
    pub async fn text(&self) -> Result<String> { ... }
    pub async fn html(&self) -> Result<String> { ... }
    pub async fn query_all(&self, selector: &str) -> Result<Vec<String>> { ... }
    pub async fn title(&self) -> Result<String> { ... }
}
```

**A2aApi** — A2AProtocol 통합:

```rust
pub struct A2aApi {
    protocol: Arc<A2AProtocol>,
}

impl A2aApi {
    pub async fn delegate(&self, task: TaskSpec, from: &AgentId) -> Result<DelegationResult> { ... }
    pub async fn send(&self, message: A2AMessage, from: &AgentId) -> Result<()> { ... }
    pub async fn query(&self, capability: &str) -> Result<Vec<AgentCard>> { ... }
}
```

### Phase 2: Tool 재구성

모든 tool이 KernelHandle만 참조. 내부 모듈 직접 참조 제거.

```
tools/
├── builtin/                  (always-on, kernel 독립)
│   └── (file ops, web_search은 oxi-agent에서 제공)
│
├── kernel/                   (KernelHandle wrapper)
│   ├── exec_tool.rs          → KernelHandle.exec
│   ├── browser_tool.rs       → KernelHandle.browser
│   ├── memory_tool.rs        → KernelHandle.agents (memory methods)
│   ├── space_tool.rs         → KernelHandle.spaces
│   ├── agent_tool.rs         → KernelHandle.agents (lifecycle)
│   ├── persona_tool.rs       → KernelHandle.persona
│   ├── program_tool.rs       → KernelHandle.extensions
│   ├── cron_tool.rs          → KernelHandle.infra (cron)
│   ├── git_tool.rs           → KernelHandle.infra (git)
│   ├── resource_tool.rs      → KernelHandle.infra (resource)
│   ├── budget_tool.rs        → KernelHandle.agents (budget)
│   ├── security_tool.rs      → KernelHandle.security
│   ├── a2a_tool.rs           → KernelHandle.a2a
│   ├── mcp_tool.rs           → KernelHandle.mcp
│   └── event_tool.rs         → KernelHandle.infra (events)
│
├── profile.rs                (Profile → Tool 등록 로직)
├── capability_index.rs       (Index 자동 생성)
├── kernel_manifest.rs        (Manifest 자동 생성)
└── mod.rs
```

각 KernelTool의 구조 (동일한 패턴):

```rust
/// Space management tool. Wraps KernelHandle.spaces.
pub struct SpaceTool {
    spaces: Arc<SpaceApi>,
}

impl SpaceTool {
    pub fn new(handle: &KernelHandle) -> Self {
        Self { spaces: Arc::new(handle.spaces.clone()) }
        // 또는 handle을 통째로 들고 있는다
    }
}

#[async_trait]
impl AgentTool for SpaceTool {
    fn name(&self) -> &str { "space" }

    fn description(&self) -> &'static str {
        "Manage Oxios work spaces. Actions: list, get, create, archive, merge, restore"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "get", "create", "archive", "merge", "restore"]
                },
                "id": { "type": "string", "description": "Space ID" },
                "name": { "type": "string", "description": "Space name (for create)" },
                "absorbed_id": { "type": "string", "description": "Space to merge (for merge)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, _: &str, params: Value, _: Option<oneshot::Receiver<()>>)
        -> Result<AgentToolResult, String>
    {
        let action = params["action"].as_str().ok_or("missing action")?;
        match action {
            "list" => {
                let spaces = self.spaces.list_spaces();
                Ok(AgentToolResult::success(
                    serde_json::to_string_pretty(&spaces).unwrap()
                ))
            }
            "archive" => {
                let id = params["id"].as_str().ok_or("missing id")?;
                self.spaces.archive(id).await
                    .map_err(|e| e.to_string())?;
                Ok(AgentToolResult::success(format!("Space {} archived", id)))
            }
            // ... get, create, merge, restore
        }
    }
}
```

### Phase 3: Profile 기반 등록

```rust
/// Profile에 따라 tool을 선택적으로 등록.
pub fn register_tools(
    registry: &ToolRegistry,
    kernel: &KernelHandle,
    profile: &ToolProfile,
) {
    // ── Always-on (KernelHandle 독립) ──
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GrepTool::new());
    registry.register(FindTool::new());
    registry.register(LsTool::new());
    registry.register(WebSearchTool::new(search_cache));

    // ── Always-on (KernelHandle 경유) ──
    registry.register(ExecTool::from_kernel(kernel));
    registry.register(BrowserTool::from_kernel(kernel));

    // ── Standard+ ──
    if *profile >= ToolProfile::Standard {
        registry.register(MemoryReadTool::from_kernel(kernel));
        registry.register(MemorySearchTool::from_kernel(kernel));
    }

    // ── Operator+ ──
    if *profile >= ToolProfile::Operator {
        registry.register(MemoryWriteTool::from_kernel(kernel));
        registry.register(SpaceTool::from_kernel(kernel));
        registry.register(AgentControlTool::from_kernel(kernel));
        registry.register(PersonaTool::from_kernel(kernel));
        registry.register(ProgramTool::from_kernel(kernel));
        registry.register(CronTool::from_kernel(kernel));
        registry.register(A2aTool::from_kernel(kernel));
        registry.register(McpTool::from_kernel(kernel));
    }

    // ── Supervisor ──
    if *profile >= ToolProfile::Supervisor {
        registry.register(SecurityTool::from_kernel(kernel));
        registry.register(BudgetTool::from_kernel(kernel));
        registry.register(ResourceTool::from_kernel(kernel));
        registry.register(GitTool::from_kernel(kernel));
        registry.register(AuditTool::from_kernel(kernel));
    }
}
```

---

## 5. Capability Index 자동 생성

등록된 tool + 설치된 프로그램에서 Index를 자동 생성.

```rust
pub fn build_capability_index(
    registry: &ToolRegistry,
    kernel: &KernelHandle,
    profile: &ToolProfile,
) -> String {
    let mut entries = Vec::new();

    // 1. Registry에 등록된 tool에서 자동 추출
    for tool_name in registry.tool_names() {
        if let Some(tool) = registry.get(&tool_name) {
            entries.push(format!(
                "  <capability>\n    \
                 <name>{name}</name>\n    \
                 <category>os-tool</category>\n    \
                 <description>{desc}</description>\n  \
                 </capability>",
                name = tool.name(),
                desc = tool.description(),
            ));
        }
    }

    // 2. Programs (exec으로 직접 실행 가능)
    for program in kernel.extensions.list_programs().await {
        entries.push(format!(
            "  <capability>\n    \
             <name>{name}</name>\n    \
             <category>program</category>\n    \
             <description>{desc}</description>\n    \
             <command>{cmd}</command>\n    \
             <skill>{skill_path}</skill>\n  \
             </capability>",
            name = program.name,
            desc = program.description,
            cmd = program.name,
            skill_path = program.skill_path.display(),
        ));
    }

    format!("\n<available_capabilities>\n{}\n</available_capabilities>\n\n\
             Use the `read` tool to load a capability's SKILL.md for detailed instructions.",
            entries.join("\n"))
}
```

---

## 6. Kernel Manifest 자동 생성

Profile에 따라 OS 구조/룰 안내를 자동 생성.

```rust
pub fn build_kernel_manifest(profile: &ToolProfile) -> Option<String> {
    if *profile < ToolProfile::Operator {
        return None;
    }

    let mut manifest = String::from(
        "## Oxios Agent OS\n\n\
         You are running inside Oxios Agent OS.\n\n"
    );

    // Spaces
    manifest.push_str(
        "### Spaces\n\
         Work is organized into isolated Spaces.\n\
         - Each space has: workspace directory, memory store, state store\n\
         - Use `space` tool to list, create, archive, merge spaces\n\
         - Memory is scoped to the current space unless specified otherwise\n\n"
    );

    // Agents
    if *profile >= ToolProfile::Operator {
        manifest.push_str(
            "### Agents\n\
             Other agents exist in this OS.\n\
             - Use `agent` tool to list, create, kill agents\n\
             - Use `a2a` tool to delegate tasks, send messages, query capabilities\n\
             - Agent lifecycle: fork → exec → wait → kill\n\n"
        );
    }

    // Programs
    manifest.push_str(
        "### Programs\n\
         Programs are OS-level installable capabilities.\n\
         - Read the Capability Index above for available programs\n\
         - Use `read` to load a program's SKILL.md for usage\n\
         - Use `exec` to run program commands\n\n"
    );

    // Security
    if *profile >= ToolProfile::Supervisor {
        manifest.push_str(
            "### Security & Auditing\n\
             - All operations are audit-logged\n\
             - `security` tool: verify audit chain, query audit log\n\
             - `budget` tool: check/set token budgets per agent\n\
             - `resource` tool: monitor CPU, memory, disk\n\n"
        );
    }

    // Ouroboros
    manifest.push_str(
        "### Ouroboros Protocol\n\
         All work follows: Interview → Seed → Execute → Evaluate → Evolve.\n"
    );

    Some(manifest)
}
```

---

## 7. 최종 System Prompt 조립

```rust
fn build_system_prompt(
    seed: &Seed,
    persona_prompt: Option<&str>,
    capability_index: &str,       // auto-generated from registry + programs
    kernel_manifest: Option<&str>, // auto-generated from profile
) -> String {
    let mut prompt = String::new();

    // 1. Identity
    prompt.push_str("You are an autonomous agent.\n\n");

    // 2. Task (Seed)
    prompt.push_str(&format!("## Goal\n{}\n", seed.goal));
    // constraints, acceptance_criteria, ontology ...

    // 3. Persona
    if let Some(pp) = persona_prompt {
        prompt.push_str(&format!("\n## Persona\n{}\n", pp));
    }

    // 4. Capability Index (항상)
    prompt.push_str("\n## Available Capabilities\n");
    prompt.push_str(capability_index);

    // 5. Kernel Manifest (Operator+)
    if let Some(manifest) = kernel_manifest {
        prompt.push_str(manifest);
    }

    prompt
}
```

---

## 8. 전체 흐름도

```
┌─────────────────────────────────────────────────────────────┐
│                         seed + profile                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
          ┌────────────────▼────────────────┐
          │       register_tools()           │
          │  profile → 선택적 tool 등록      │
          └────────────────┬────────────────┘
                           │
     ┌─────────────────────▼─────────────────────┐
     │              ToolRegistry                  │
     │                                           │
     │  Worker:     file ops, exec, browser, ws   │
     │  Standard:   + memory                      │
     │  Operator:   + space, agent, a2a, mcp, ... │
     │  Supervisor: + security, budget, audit     │
     └─────────────────────┬─────────────────────┘
                           │
     ┌─────────────────────▼─────────────────────┐
     │         build_capability_index()           │
     │  registry tools + programs → XML index     │
     └─────────────────────┬─────────────────────┘
                           │
     ┌─────────────────────▼─────────────────────┐
     │         build_kernel_manifest()            │
     │  profile → OS 구조/룰 안내                 │
     └─────────────────────┬─────────────────────┘
                           │
     ┌─────────────────────▼─────────────────────┐
     │         build_system_prompt()              │
     │  Seed + Persona + Index + Manifest         │
     └─────────────────────┬─────────────────────┘
                           │
     ┌─────────────────────▼─────────────────────┐
     │           AgentLoop::run()                 │
     │  LLM이 tool_call → registry에서 실행       │
     │  모든 tool → KernelHandle 경유             │
     │  KernelHandle → 내부 모듈 실행             │
     └───────────────────────────────────────────┘
```

---

## 9. Before vs After

### Before (현재)

```
Agent → MemoryTool → MemoryManager (직접)         ❌ 커널 우회
Agent → A2aTool → A2AProtocol (직접)              ❌ 커널 우회
Agent → ExecTool → ExecConfig (직접)              ❌ 커널 우회
Agent → BrowserTool → OxibrowserBackend (직접)    ❌ 커널 우회
Agent → space_list ???                             ❌ 없음
Agent → agent_list ???                             ❌ 없음
Agent → persona_set ???                            ❌ 없음
Agent → cron_add ???                               ❌ 없음
Agent → security_audit ???                         ❌ 없음

KernelHandle → CLI만 사용                          ❌ 에이전트 접근 불가
```

### After (목표)

```
Agent → exec        → KernelHandle.exec         ✅
Agent → browser     → KernelHandle.browser      ✅
Agent → memory      → KernelHandle.agents       ✅
Agent → space       → KernelHandle.spaces       ✅
Agent → agent       → KernelHandle.agents       ✅
Agent → persona     → KernelHandle.persona      ✅
Agent → program     → KernelHandle.extensions   ✅
Agent → cron        → KernelHandle.infra        ✅
Agent → git         → KernelHandle.infra        ✅
Agent → a2a         → KernelHandle.a2a          ✅
Agent → mcp         → KernelHandle.mcp          ✅
Agent → security    → KernelHandle.security     ✅
Agent → budget      → KernelHandle.agents       ✅
Agent → resource    → KernelHandle.infra        ✅

모든 OS 제어 → KernelHandle 경유 → 단일 경로
```

---

## 10. 구현 순서

```
Phase 1: KernelHandle 확장
  ├── ExecApi 추가 (exec_config + access_manager)
  ├── BrowserApi 추가 (OxibrowserBackend)
  ├── A2aApi 추가 (A2AProtocol)
  └── kernel.rs에서 11개 Facade 조립

Phase 2: Tool 재구성
  ├── tools/kernel/ 디렉토리 생성
  ├── 기존 tool들을 KernelHandle wrapper로 마이그레이션
  │   ├── exec_tool → KernelHandle.exec
  │   ├── browser_tool → KernelHandle.browser
  │   ├── memory_tools → KernelHandle.agents (memory methods)
  │   ├── a2a_tools → KernelHandle.a2a
  │   └── mcp_tool → KernelHandle.mcp
  └── 신규 tool 추가
      ├── space_tool → KernelHandle.spaces
      ├── agent_tool → KernelHandle.agents
      ├── persona_tool → KernelHandle.persona
      ├── cron_tool → KernelHandle.infra
      ├── git_tool → KernelHandle.infra
      ├── security_tool → KernelHandle.security
      ├── budget_tool → KernelHandle.agents
      └── resource_tool → KernelHandle.infra

Phase 3: Profile + Index + Manifest
  ├── ToolProfile enum + profile 결정 로직
  ├── capability_index.rs (자동 생성)
  ├── kernel_manifest.rs (자동 생성)
  ├── register_tools() (profile 기반 등록)
  └── build_system_prompt() 통합

Phase 4: Seed 확장
  └── tool_profile 필드 추가
```

---

## 11. 아름다움의 기준

```
1. 단일 경로 (Single Path)
   에이전트 → Tool → KernelHandle → Module
   모든 OS 제어가 하나의 경로를 통과.

2. 대칭성 (Symmetry)
   CLI → KernelHandle → Module
   Agent → Tool → KernelHandle → Module
   Guardian → KernelHandle → Module
   같은 KernelHandle. 같은 API. 다른 interface.

3. 자동성 (Automation)
   Program 추가 → Index에 자동 등록
   Tool 추가 → Profile에 의해 자동 노출
   Kernel API 추가 → Tool로 자동 노출 가능

4. 단순성 (Simplicity)
   discover tool 불필요. read가 곧 discover.
   program tool 등록 불필요. exec로 직접 실행.
   scope enum 불필요. profile은 OS 역할 체계.
```
