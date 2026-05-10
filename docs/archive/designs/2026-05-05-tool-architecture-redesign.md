# Tool Architecture Redesign

> **날짜:** 2026-05-05
> **상태:** 설계 완료, 구현 대기
> **영향 모듈:** `agent_runtime`, `program`, `host_exec`, `container`, `container_manager`, `config`, DESIGN.md

---

## 1. 문제 진단

### 현재 구조

```
AgentRuntime
└── ToolRegistry::with_builtins()     ← oxi 기본값 그대로 사용
    ├── ReadTool
    ├── WriteTool
    ├── EditTool
    ├── BashTool (sh -c ...)          ← 모든 셸 실행이 여기로
    ├── GrepTool
    ├── FindTool
    └── LsTool
```

### 문제점

1. **BashTool이 모든 셸 실행을 독점한다.** 컨테이너 내부 실행(`jq`, `curl`)과 호스트 실행(`gh`, `osascript`)의 구분이 없다.
2. **Container minimal_tools가 방치되어 있다.** `jq`, `sqlite3` 등은 BashTool의 `sh -c "jq ..."` 문자열 실행에만 의존한다. 구조화된 도구가 아니다.
3. **Program이 ToolRegistry와 연결되어 있지 않다.** Program은 SKILL.md만 System Prompt에 주입하고, `[tools]` 정의는 AgentTool로 등록되지 않는다.
4. **host_exec이 독립 모듈로만 존재한다.** HostExecBridge는 내부에서만 사용되고, 에이전트의 도구로 노출되지 않는다.
5. **ClawGarden 잔재가 남아 있다.** `garden`, `garden_manager` 등 ClawGarden에서 가져온 명칭이 그대로 쓰이고 있다. Oxios는 ClawGarden과 다른 제품이다.

### 참고: OpenClaw이 pi를 활용하는 방식

OpenClaw은 pi를 SDK로 임베딩하여:

- `builtInTools: []` 로 비우고 전부 커스텀 도구로 교체
- BashTool을 `exec` / `process` 두 도구로 분리
- 30+ 커스텀 도구를 채널/샌드박스별로 주입
- Tool + Skill + Extension을 통합 확장으로 관리

Oxios도 oxi(= pi의 Rust 포팅)를 동일한 방식으로 활용해야 한다.

---

## 2. 명명 정리

Oxios는 ClawGarden이 아니다. ClawGarden에서 가져온 코드는 있지만, 명칭과 개념은 Oxios의 정체성에 맞게 교체한다.

| 이전 (ClawGarden 잔재) | 변경 후 (Oxios) | 의미 |
|---|---|---|
| Garden | **Container** | 격리된 실행 환경 |
| GardenManager | **ContainerManager** | 컨테이너 생명주기 관리 |
| GardenStartConfig | **ContainerConfig** | 컨테이너 시작 설정 |
| GardenInfo | **ContainerInfo** | 컨테이너 메타데이터 |
| garden.rs | **container_manager.rs** | 고수준 라이프사이클 |
| container.rs | **container.rs** (유지) | 저수준 백엔드 |
| pi2oxi | **oxi** | oxi의 과거 이름 |
| create_garden / start_garden | **create / start** | 메서드명 정리 |
| exec_in_garden | **exec_in_container** | 메서드명 정리 |

**이 설계 문서 이후부터 모든 명칭은 새 기준을 사용한다.**

---

## 3. 설계 원칙

1. **oxi는 단일 실행 엔진이다.** AgentLoop가 LLM 호출, 도구 선택, 도구 실행을 모두 담당한다. Oxios는 도구의 **구성(composition)**만 담당한다.
2. **Program은 범용 확장 단위다.** oxi(pi)의 Tool, Skill, Extension, MCP 서버가 모두 Program 안에 대응된다.
3. **실행 위치는 자동으로 결정된다.** 도구가 컨테이너에 있으면 container_exec, 호스트에 있으면 host_exec.
4. **설치 단위가 곧 확장 단위다.** `oxios program install` 한 번이 Tool 등록 + Skill 주입 + 의존성 검증을 모두 수행한다.
5. **oxi-agent 코드는 재사용하고 재구현하지 않는다.** BashTool을 내부적으로 위임하여 로직 중복을 피한다.
6. **Oxios는 단일 컨테이너를 기본으로 한다.** Agent OS이지 컨테이너 오케스트레이터가 아니다.

---

## 4. 새 아키텍처

### 4.1 전체 구조

```
┌─────────────────────────────────────────────────────────────────┐
│  Oxios Kernel                                                   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ AgentRuntime                                                │ │
│  │                                                              │ │
│  │  ToolRegistry (빈 레지스트리에서 시작, Oxios가 구성)          │ │
│  │  ┌─────────────────────────────────────────────────────────┐ │ │
│  │  │ Tier 1: oxi 네이티브 도구 (항상 로드)                    │ │ │
│  │  │   read, write, edit, grep, find, ls                     │ │ │
│  │  ├─────────────────────────────────────────────────────────┤ │ │
│  │  │ Tier 2: Oxios 실행 도구 (BashTool 교체)                 │ │ │
│  │  │   container_exec  — 워크스페이스 명령 실행               │ │ │
│  │  │   host_exec       — 호스트 명령 실행 (보안 릴레이)       │ │ │
│  │  ├─────────────────────────────────────────────────────────┤ │ │
│  │  │ Tier 3: Program 도구 (동적, 프로그램 설치 시 등록)       │ │ │
│  │  │   program:github:create_pr                               │ │ │
│  │  │   program:jq:parse                                       │ │ │
│  │  └─────────────────────────────────────────────────────────┘ │ │
│  │                                                              │ │
│  │  oxi-agent AgentLoop ← ToolRegistry를 그대로 사용            │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ System Prompt Builder                                       │ │
│  │   ├── 기본 시스템 프롬프트                                   │ │
│  │   ├── 활성 Program들의 SKILL.md                             │ │
│  │   └── 컨텍스트 정보 (워크스페이스, 컨테이너 상태 등)         │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ Program Manager                                             │ │
│  │   install → tools를 Tier 3에 등록 + SKILL.md를 프롬프트에 주입│ │
│  │   uninstall → 도구 제거 + 프롬프트에서 제거                  │ │
│  │   validate → host_requirements 검사                         │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 ToolRegistry 구성 코드

`agent_runtime.rs`의 변경점:

```rust
// Before:
let tools = Arc::new(ToolRegistry::with_builtins());

// After:
fn build_tool_registry(
    container: Option<&ContainerManager>,
    programs: &ProgramManager,
    host_bridge: &HostExecBridge,
    config: &OxiosConfig,
) -> ToolRegistry {
    let registry = ToolRegistry::new();

    // Tier 1: oxi 네이티브 도구 (파일 조작)
    // BashTool은 제외 — Tier 2에서 목적별 도구로 대체
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GrepTool::new());
    registry.register(FindTool::new());
    registry.register(LsTool::new());

    // Tier 2: Oxios 실행 도구
    let container_exec = Arc::new(ContainerExecTool::new(container));
    let host_exec = Arc::new(HostExecTool::new(host_bridge));
    registry.register_arc(container_exec.clone());
    registry.register_arc(host_exec.clone());

    // Tier 3: Program 도구 (동적 등록)
    for program in programs.list_enabled() {
        for tool_def in &program.meta.tools {
            let tool = ProgramTool::from_definition(
                &program.meta.name,
                tool_def,
                &program.meta.host_requirements,
                &config.container,
                container_exec.clone(),
                host_exec.clone(),
            );
            registry.register(tool);
        }
    }

    registry
}
```

### 4.3 BashTool 교체: container_exec + host_exec

#### container_exec

BashTool을 **재구현하지 않는다.** ContainerExecTool은 oxi의 BashTool을 내부 필드로 들고, 로컬 폴백 시 BashTool::execute()에 직접 위임한다.

```rust
/// 컨테이너 내부에서 명령을 실행하는 도구.
///
/// 컨테이너가 활성화된 경우 ContainerBackend::exec_in_container()로 실행하고,
/// 비활성화된 경우 내장 BashTool::execute()에 위임하여 sh -c로 실행한다.
pub struct ContainerExecTool {
    /// oxi BashTool (로컬 폴백용)
    bash: BashTool,
    /// 컨테이너 매니저. None이면 항상 로컬 실행.
    container: Option<Arc<ContainerManager>>,
}

impl ContainerExecTool {
    pub fn new(container: Option<&ContainerManager>) -> Self {
        Self {
            bash: BashTool::new(),
            container: container.cloned(),
        }
    }
}
```

**기본 컨테이너 이름:** 단일 컨테이너 전제이므로, ContainerManager는
활성 컨테이너 이름을 제공해야 한다:

```rust
impl ContainerManager {
    /// 현재 활성 컨테이너 이름. 없으면 None.
    pub fn active_container_name(&self) -> Option<&str>;
}
```

ContainerExecTool은 `container.active_container_name()`으로 이름을 얻어
`backend.exec_in_container(name, cmd)`를 호출한다.

실행 흐름:

```
container_exec("cargo test")
    │
    ├── 컨테이너 활성?
    │   ├── YES → ContainerBackend::exec_in_container("cargo test")
    │   │         → ExecResult → format_exec_result() → AgentToolResult
    │   └── NO  → BashTool::execute("cargo test")
    │             → AgentToolResult (oxi가 처리)
    │
    └── format_exec_result()는 BashTool의 출력 포맷과 동일:
        트렁케이션 (truncate_head) + 타이밍 + exit code
```

에이전트에게 노출되는 스키마 (BashTool과 동일한 인터페이스):
```json
{
  "name": "container_exec",
  "description": "Execute a command in the workspace. Runs inside the container if active, otherwise locally. Use for compilation, tests, package management, and any workspace command.",
  "parameters": {
    "command": { "type": "string", "description": "The shell command to execute" },
    "timeout": { "type": "integer", "description": "Timeout in seconds", "default": 120 },
    "cwd": { "type": "string", "description": "Working directory" },
    "env": { "type": "object", "description": "Environment variables" }
  }
}
```

**핵심:** LLM에게는 `bash`가 보이지 않고 `container_exec`가 보인다. 컨테이너가 있든 없든 같은 도구를 쓴다.

#### host_exec

호스트 명령 실행은 **보안 모델이 container_exec와 다르다.** ContainerExecTool은 워크스페이스에서 자유롭게 실행하지만, host_exec는 macOS 시스템 자원에 접근하므로 더 엄격한 제한이 필요하다.

**보안 위협 모델:**

| 위협 | 방어 |
|------|------|
| LLM이 임의 명령 실행 | 바이너리 화이트리스트 (gh, git, open 등만 허용) |
| 셸 인젝션 (`; rm -rf`) | `sh -c` 대신 직접 실행 + 인자 분리 |
| 경로 순회 | `../` 차단 |

따라서 host_exec의 API는 container_exec와 다르다:

```rust
/// 호스트(macOS)에서 명령을 실행하는 도구.
/// 보안 정책: 바이너리 화이트리스트 + 인자 분리 실행.
/// 화이트리스트는 HostExecBridge가 소유하며,
/// 초기화 시 config의 allowed_host_commands + required/optional_host_tools를 모두 합친다.
pub struct HostExecTool {
    bridge: Arc<HostExecBridge>,
}
```

에이전트에게 노출되는 스키마:
```json
{
  "name": "host_exec",
  "description": "Execute a command on the host (macOS). Use for git, gh, osascript, open, and other host-only tools. The binary must be in the allowlist.",
  "parameters": {
    "binary": { "type": "string", "description": "Command binary name (e.g. 'gh', 'git', 'open')" },
    "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments" },
    "timeout": { "type": "integer", "description": "Timeout in seconds", "default": 30 }
  }
}
```

**왜 container_exec와 API가 다른가?**

- container_exec: 워크스페이스 안에서 자유로운 셸 스크립트 필요 (`cargo test && echo ok`)
- host_exec: 시스템 자원 접근 → 바이너리 단위 제한이 필요

LLM에게는 두 도구의 용도가 명확히 다르기 때문에, API 차이가 혼란보다 안전성을 우선한다. description에 사용법을 명시한다.

#### 보안 정책 상세

host_exec의 실행 흐름:

```
host_exec(binary="gh", args=["pr", "create", "--title", "Fix"])
    │
    ├── 1. binary가 화이트리스트에 있는지 확인
    │   ├── 화이트리스트 = HostExecBridge.allowed_commands
    │   │   (초기화 시 config.container.allowed_host_commands
    │   │    + config.container.required_host_tools
    │   │    + config.container.optional_host_tools 를 모두 합침)
    │   └── 없으면 → 에러 반환
    │
    ├── 2. args에 셸 메타문자/경로순회 있는지 확인
    │   └── HostExecBridge::validate_args() 재사용
    │
    └── 3. HostExecBridge::exec(binary, args, timeout)
        → HostExecResult → AgentToolResult
```

HostExecBridge의 기존 보안 모델(바이너리 화이트리스트, 메타문자 차단, 경로 순회 차단)을 그대로 재사용한다. 재구현하지 않는다.

### 4.4 실행 위치 자동 결정

Program 도구의 실행 위치는 **Program의 host_requirements**와 **글로벌 config 설정**을 모두 고려하여 자동 결정된다.

```rust
pub struct ProgramTool {
    /// "program:{program_name}:{tool_name}" 형태의 전체 이름.
    /// from_definition()에서 한 번 생성하여 name()에서 &str로 반환.
    full_name: String,
    /// 실행할 바이너리 (예: "gh", "jq")
    binary: String,
    /// 기본 인자 (예: ["pr", "create"])
    default_args: Vec<String>,
    /// true면 host_exec로, false면 container_exec로
    runs_on_host: bool,
    /// 실행 위임 대상 (직접 실행하지 않음)
    container_exec: Arc<ContainerExecTool>,
    host_exec: Arc<HostExecTool>,
}
```

**왜 직접 실행하지 않나?** ProgramTool이 ContainerManager나 HostExecBridge를 직접
들면 컨테이너 비활성 시 로컬 폴백(BashTool 위임) 로직을 중복 구현해야 한다.
Tier 2 도구에 위임하면 이 로직을 재사용한다.

라우팅 판단 로직:

```rust
impl ProgramTool {
    pub fn from_definition(
        program_name: &str,
        tool_def: &ToolDef,
        host_requirements: &ProgramHostRequirements,
        container_config: &ContainerConfig,
        container_exec: Arc<ContainerExecTool>,
        host_exec: Arc<HostExecTool>,
    ) -> Self {
        // command에서 바이너리와 기본 인자 추출
        let parts: Vec<&str> = tool_def.command.split_whitespace().collect();
        let binary = parts.first().unwrap_or(&"").to_string();
        let default_args = parts.iter().skip(1).map(|s| s.to_string()).collect();

        // 실행 위치 결정:
        // 1) Program의 host_requirements에 있으면 host_exec
        // 2) 글로벌 config의 required_host_tools / optional_host_tools에 있으면 host_exec
        // 3) 그 외는 container_exec
        let is_host_tool = |name: &str| -> bool {
            host_requirements.required.iter()
                .chain(host_requirements.optional.iter())
                .chain(container_config.required_host_tools.iter())
                .chain(container_config.optional_host_tools.iter())
                .any(|t| t == name)
        };

        let runs_on_host = is_host_tool(&binary);

        Self {
            full_name: format!("program:{}:{}", program_name, tool_def.name),
            binary,
            default_args,
            runs_on_host,
            container_exec,
            host_exec,
        }
    }
}
```

라우팅 결정:

```
Program 도구 실행 요청 ("gh pr create")
    │
    ├── runs_on_host?
    │   │
    │   ├── YES → HostExecTool.execute()에 위임
    │   │   host_bridge.exec("gh", ["pr", "create"] + user_args)
    │   │   → 화이트리스트 검증 → 메타문자 검증 → 실행
    │   │
    │   └── NO  → ContainerExecTool.execute()에 위임
    │       container_active?
    │       ├── YES → backend.exec_in_container(cmd)
    │       └── NO  → BashTool.execute(sh -c cmd)  (자동 폴백)
```

ProgramTool은 실행 로직을 직접 갖지 않고 Tier 2 도구에 위임하므로,
로컬 폴백(BashTool) 로직이 자동으로 재사용된다.

**`git` 중복 문제 해결:** config에서 `git`이 `required_host_tools`에 있으면, `git` 명령은 항상 호스트에서 실행된다. 컨테이너에도 git이 있지만, Oxios 철학상 git은 호스트 마운트로 사용하는 게 의도이므로 이게 맞다.

### 4.5 ProgramTool의 name() 해결

oxi의 `AgentTool` 트레이트:

```rust
fn name(&self) -> &str;
```

`&str`을 반환해야 하므로 동적 문자열 생성이 불가. 해결:

```rust
pub struct ProgramTool {
    full_name: String,  // "program:github:create_pr"
    // ...
}

impl AgentTool for ProgramTool {
    fn name(&self) -> &str {
        &self.full_name
    }
}
```

`from_definition()`에서 `format!("program:{}:{}", ...)`으로 한 번 생성하고 필드에 저장. `name()`은 그 참조를 반환. 수명 문제 없음.

---

## 5. Program = 범용 확장 단위

### 5.1 외부 세계 매핑

| 외부 개념 | Program에서의 대응 | 구현 방식 |
|-----------|-------------------|-----------|
| **oxi(pi) Tool** | `program.toml [[tools]]` | `AgentTool` → ToolRegistry 등록 |
| **oxi(pi) Skill** | `SKILL.md` | System Prompt에 주입 |
| **oxi(pi) Extension** | `program.toml [hooks]` (향후) | 에이전트 라이프사이클 훅 |
| **MCP 서버** | `program.toml [mcp]` (향후) | McpBridge 연결 |

### 5.2 Program의 세 가지 성격

#### 명령형 Program (Tool)

```toml
[program]
name = "jq"
version = "1.0.0"
description = "JSON processing"
author = "oxios"

[[tools]]
name = "parse"
description = "Parse and query JSON data"
command = "jq"
```

- `program:jq:parse` AgentTool이 ToolRegistry에 등록됨
- SKILL.md 없어도 됨

#### 지시형 Program (Skill)

```toml
[program]
name = "code-review"
version = "1.0.0"
description = "Deep code review"
author = "oxios"

# [[tools]] 없음 → ToolRegistry에 등록 안 함

[requires_tools]
names = ["read", "container_exec", "grep"]

[host_requirements]
required = ["git"]
```

- ToolRegistry에 도구 등록 안 함
- System Prompt에 `code-review/SKILL.md` 주입
- 에이전트는 Tier 1-2 도구를 조합해서 SKILL.md의 지시를 수행

#### 복합형 Program (Tool + Skill)

```toml
[program]
name = "github"
version = "1.0.0"
description = "GitHub integration"
author = "oxios"

[[tools]]
name = "create_pr"
description = "Create a pull request"
command = "gh pr create"

[[tools]]
name = "list_issues"
description = "List repository issues"
command = "gh issue list"

[host_requirements]
required = ["gh"]

# SKILL.md도 있음 → PR 생성 가이드라인 등
```

- `program:github:create_pr`, `program:github:list_issues` 등록
- SKILL.md도 System Prompt에 주입

### 5.3 `[[tools]]` vs `[requires_tools]` 분리

| 필드 | 의미 | 동작 |
|------|------|------|
| `[[tools]]` | 이 Program이 **제공**하는 도구 | ToolRegistry에 AgentTool로 등록 |
| `[requires_tools]` | 이 Program이 **필요로** 하는 도구 | 설치 시 정적 화이트리스트로 검증만 |

`requires_tools`의 검증은 ToolRegistry가 아직 구성되지 않은 시점(Program init)에 일어나므로, **정적 화이트리스트**로 검증한다:

```rust
/// Tier 1-2의 기본 도구 이름 (항상 존재 보장)
const BASE_TOOLS: &[&str] = &[
    "read", "write", "edit", "grep", "find", "ls",
    "container_exec", "host_exec",
];

fn validate_requires_tools(requires: &[String]) -> Result<()> {
    for name in requires {
        if !BASE_TOOLS.contains(&name.as_str()) {
            // Program 도구일 수도 있으므로 경고만 (에러 아님)
            tracing::warn!("Unknown tool '{}' in requires_tools", name);
        }
    }
    Ok(())
}
```

### 5.4 Program 수명주기

```
oxios program install ./my-program
    │
    ├── 1. program.toml 파싱 + 스키마 검증
    ├── 2. [requires_tools] 검증 — 정적 화이트리스트로 확인
    ├── 3. host_requirements 검사 — 호스트에 필요한 명령어 있는지
    │   └── missing required → 경고 (설치는 진행, 사용 시 실패)
    ├── 4. .programs/ 디렉토리에 복사
    ├── 5. [[tools]] → ToolRegistry에 AgentTool로 등록 (Tier 3)
    └── 6. SKILL.md → System Prompt Builder에 등록

oxios program uninstall my-program
    │
    ├── 1. ToolRegistry에서 Program 도구 제거
    ├── 2. System Prompt Builder에서 SKILL.md 제거
    └── 3. .programs/ 디렉토리 삭제
```

---

## 6. program.toml 스키마 (개정)

```toml
[program]
name = "my-program"          # 필수, 고유 식별자 (kebab-case)
version = "1.0.0"            # 필수, SemVer
description = "..."          # 필수
author = "oxios"             # 필수

# ── 이 Program이 에이전트에게 제공하는 도구 ──
# 각 도구는 AgentTool로 ToolRegistry에 등록됨.
# command의 첫 단어가 바이너리 이름. host 도구면 host_exec, 아니면 container_exec.

[[tools]]
name = "parse"               # 도구 식별자
description = "Parse JSON"   # LLM이 보는 설명
command = "jq"               # 실행할 명령어 (첫 단어 = 바이너리, 나머지 = 기본 인자)

[[tools]]
name = "query"
description = "Query with expression"
command = "jq -r"            # 기본 인자 포함 가능

# ── 이 Program이 동작에 필요로 하는 도구 ──
# 설치 시 존재 여부 검증만. ToolRegistry에 등록하지 않음.
[requires_tools]
names = ["read", "container_exec", "grep"]

# ── 호스트(macOS) 의존성 ──
# command의 바이너리가 여기 있으면 host_exec로 라우팅.
[host_requirements]
required = []                # 없으면 설치 경고
optional = ["gh"]            # 있으면 기능 확장

# ── 컨테이너 의존성 ──
[container]
minimal_tools = []
```

디렉토리 구조:

```
my-program/
├── program.toml     ← 메타데이터 + 도구 정의
├── SKILL.md         ← 에이전트 지시사항 (선택)
└── bin/             ← 실행 스크립트 (선택)
```

---

## 7. System Prompt 구성

```rust
fn build_system_prompt(
    seed: &Seed,
    programs: &[Program],
    container_active: bool,
    config: &OxiosConfig,
) -> String {
    let mut prompt = build_base_prompt(seed);

    // 실행 환경 정보
    if container_active {
        prompt.push_str(&format!(
            "\n## Execution Environment\n\
             Running inside an Apple Container.\n\
             Available container tools: {}\n\
             Use `container_exec` to run commands in the container.\n\
             Use `host_exec` to run commands on macOS (host).\n",
            config.container.minimal_tools.join(", "),
        ));
    } else {
        prompt.push_str(
            "\n## Execution Environment\n\
             Running locally (no container).\n\
             Use `container_exec` for all shell commands.\n",
        );
    }

    // Program SKILL.md 주입 (활성 Program만)
    for program in programs {
        if program.enabled && !program.skill_content.is_empty() {
            prompt.push_str(&format!(
                "\n## Program: {}\n\n{}\n",
                program.meta.name, program.skill_content
            ));
        }
    }

    prompt
}
```

---

## 8. ClawGarden 잔재 제거

### 파일 리네이밍

| 이전 | 변경 후 | 비고 |
|------|---------|------|
| `container.rs` | `container.rs` (유지) | 저수준 ContainerBackend + AppleBackend (~890줄) |
| `garden.rs` | **`container_manager.rs`** | 고수준 ContainerManager (~450줄) |

**분리 유지 이유:** container.rs는 저수준 백엔드(트레이트 + Apple Container CLI 래핑), container_manager.rs는 고수준 라이프사이클(백엔드 + host_exec + state_store 오케스트레이션). 추상화 레벨이 다르므로 합치지 않는다.

### 전체 교체 항목

| 파일 | 변경 |
|------|------|
| `container.rs` | 메서드명 정리 (create_garden → create 등) |
| `garden.rs` → `container_manager.rs` | GardenManager → ContainerManager, GardenInfo → ContainerInfo |
| `host_exec.rs` | 주석/변수명 정리 |
| `config.rs` | `garden_path` → `container_path` |
| `lib.rs` | `garden` 모듈 → `container_manager` 모듈 |
| `DESIGN.md` | garden → container 전면 교체 |
| `AGENTS.md` | garden 참조 교체 |

---

## 9. 변경 파일 목록

### Phase 0: ClawGarden 잔재 제거 (선행 권장)

| 파일 | 변경 내용 |
|------|-----------|
| `crates/oxios-kernel/src/container.rs` | 메서드명 정리 |
| `crates/oxios-kernel/src/garden.rs` → `container_manager.rs` | GardenManager → ContainerManager |
| `crates/oxios-kernel/src/host_exec.rs` | 주석/변수명 정리 |
| `crates/oxios-kernel/src/config.rs` | `garden_path` → `container_path` |
| `crates/oxios-kernel/src/lib.rs` | 모듈 리네이밍 |
| `DESIGN.md` | garden → container 전면 교체 |
| `AGENTS.md` | garden 참조 교체 |

### Phase 1: ToolRegistry 재구성

| 파일 | 변경 내용 |
|------|-----------|
| `crates/oxios-kernel/src/agent_runtime.rs` | `with_builtins()` → `build_tool_registry()` |
| **신규** `crates/oxios-kernel/src/tools/mod.rs` | 도구 모듈 |
| **신규** `crates/oxios-kernel/src/tools/container_exec.rs` | ContainerExecTool (BashTool 내부 위임) |
| **신규** `crates/oxios-kernel/src/tools/host_exec_tool.rs` | HostExecTool (HostExecBridge 래핑) |

### Phase 2: Program-Tool 연동

| 파일 | 변경 내용 |
|------|-----------|
| **신규** `crates/oxios-kernel/src/tools/program_tool.rs` | ProgramTool (full_name 필드, 이중 라우팅) |
| `crates/oxios-kernel/src/program.rs` | ToolDef에 command 필드, requires_tools 파싱, list_enabled() |
| `.programs/*/program.toml` | 새 스키마로 재작성 |

### Phase 3: System Prompt 통합

| 파일 | 변경 내용 |
|------|-----------|
| `crates/oxios-kernel/src/agent_runtime.rs` | System Prompt Builder에 Program SKILL.md 주입 |

---

## 10. 제약사항 및 향후 고려사항

1. **oxi-agent의 AgentTool 트레이트는 변경하지 않는다.** oxi는 경계 밖의 코드다.
2. **BashTool 내부 위임.** ContainerExecTool은 oxi의 BashTool을 필드로 들고 로컬 폴백에 위임한다. BashTool 코드를 복사하지 않는다.
3. **ToolRegistry에 `unregister()`가 없다.** Program uninstall 시 레지스트리를 재구성하거나, oxi-agent에 `unregister()`를 추가해야 한다.
4. **MCP 통합** — `[mcp]` 섹션은 선언만 하고, 실제 McpBridge 연동은 후속 작업.
5. **Extension hooks** — `[hooks]` (oxi Extension 대응)은 후속 작업. 현재는 Tool + Skill에 집중.
6. **도구 이름 충돌** — Program 도구는 `program:{program_name}:{tool_name}` 네임스페이스 사용.
7. **단일 컨테이너 전제.** 에이전트마다 별도 컨테이너를 띄우는 건 향후 과제.
8. **container_exec과 host_exec의 API가 다르다.** container_exec은 셸 문자열(command), host_exec은 구조화된(binary + args) 입력. 이는 보안 요구사항의 차이를 반영한 의도적 설계다.
