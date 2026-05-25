# Capability Discovery Architecture

> Oxios AgentOS의 tool, program, OS context를 에이전트에게 전달하는 아키텍처.

## 설계 원칙

| 원칙 | 설명 |
|------|------|
| **항상 인지** | 에이전트는 항상 뭘 가진지 안다 (Capability Index) |
| **필요 시 상세** | 상세 사용법은 필요할 때 `read`로 SKILL.md 조회 |
| **역할 기반 tool** | OS 역할(Profile)에 따라 tool 등록만 차등 |
| **열린 세계** | 새 program/skill 추가 시 시스템 수정 없음 |

## 세 가지 문제, 세 가지 해결

```
문제 A: "어떤 tool이 쓸 수 있는가"     → Tool Profile
문제 B: "어떤 program이 있는지 아는가"  → Capability Index + read
문제 C: "OS 룰/구조를 아는가"           → Kernel Manifest
```

---

## 1. Tool Profile

OS 역할 체계에 따라 tool registry에 등록할 tool을 결정.
에이전트의 한계를 정하는 게 아니라, **OS에서 이 역할에 제공하는 tool set**을 정의.

```rust
/// 에이전트의 OS 역할. tool 등록 범위를 결정.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolProfile {
    /// 기본 작업. 코딩, 검색, 브라우징.
    #[default]
    Worker,
    /// 작업 + 메모리 + 프로그램.
    Standard,
    /// OS 내부 제어. space 관리, 에이전트 조율.
    Operator,
    /// 시스템 관리자. 전체 접근.
    Supervisor,
}
```

### Profile × Tool 매트릭스

| Tool | Worker | Standard | Operator | Supervisor |
|------|:------:|:--------:|:--------:|:----------:|
| **read, write, edit, grep, find, ls** | ✅ | ✅ | ✅ | ✅ |
| **exec** (shell) | ✅ | ✅ | ✅ | ✅ |
| **browser** | ✅ | ✅ | ✅ | ✅ |
| **web_search** | ✅ | ✅ | ✅ | ✅ |
| memory_read, memory_search | | ✅ | ✅ | ✅ |
| memory_write | | | ✅ | ✅ |
| a2a_delegate, a2a_send, a2a_query | | | ✅ | ✅ |
| mcp | | | ✅ | ✅ |
| budget, resource_monitor, audit | | | | ✅ |

### Program Tools

`program:*` tool은 **registry에 자동 등록하지 않는다.**

Capability Index에 명령 경로와 설명만 표시하고, 에이전트가 `exec` tool로
직접 실행. 별도 tool 등록 불필요.

```
Before: program:sqlite-explorer:query → ProgramTool → ExecTool
After:  exec("sqlite-explorer query mydb.sqlite")
```

이유: 프로그램이 50개면 tool schema만 수천 토큰. exec로 직접 실행하면
registry는 항상 깔끔하고, program 추가/삭제가 tool 등록에 영향을 주지 않음.

---

## 2. Capability Index

### System Prompt에 항상 주입

모든 program, memory, A2A 등 활성화된 capability의 **인덱스**를 항상 주입.
에이전트는 항상 뭘 가진지 알 수 있음.

```xml
<available_capabilities>
  <capability>
    <name>sqlite-explorer</name>
    <category>program</category>
    <description>Query SQLite databases, inspect schemas</description>
    <command>sqlite-explorer</command>
    <skill>/path/to/programs/sqlite-explorer/SKILL.md</skill>
  </capability>
  <capability>
    <name>github-manager</name>
    <category>program</category>
    <description>GitHub PR, issue, repository management</description>
    <command>gh</command>
    <skill>/path/to/programs/github-manager/SKILL.md</skill>
  </capability>
  <capability>
    <name>memory</name>
    <category>os-service</category>
    <description>Persistent cross-space memory with semantic search</description>
    <tools>memory_read, memory_search</tools>
  </capability>
  <capability>
    <name>a2a</name>
    <category>os-service</category>
    <description>Inter-agent communication and delegation</description>
    <tools>a2a_delegate, a2a_send, a2a_query</tools>
  </capability>
</available_capabilities>

Use the `read` tool to load a capability's SKILL.md for detailed usage instructions.
```

### 토큰 비용

| 항목 | 당 토큰 | 수량 | 총 토큰 |
|------|---------|------|---------|
| capability 블록 1개 | ~20 토큰 | 50개 | ~1,000 토큰 |
| 안내 텍스트 | ~50 토큰 | 1 | ~50 토큰 |
| **총 상시 오버헤드** | | | **~1,050 토큰** |

SKILL.md 전체(평균 2KB × 50 = 100KB)를 주입하는 것과 비교하면
약 1% 수준.

### read = discover

별도 discover tool 없이, 에이전트는 이미 가진 `read` tool로 SKILL.md를 읽음.

```
에이전트: Index에서 "sqlite-explorer" 발견
→ "DB 스키마 확인해야지. 어떻게 쓰더라?"
→ read({ path: "/path/to/programs/sqlite-explorer/SKILL.md" })
← SKILL.md 내용 반환
→ exec({ command: "sqlite-explorer schema mydb.sqlite" })
```

OpenClaw가 증명한 방식. discover tool은 불필요.

---

## 3. Kernel Manifest

Oxios만의 고유한 문제: 에이전트가 **OS를 제어**해야 함.
Space, Agent lifecycle, Memory scope, RBAC, Ouroboros 같은 OS 개념을
이해해야 하는 에이전트에게만 주입.

### Profile × Manifest 매트릭스

| Profile | Kernel Manifest |
|---------|----------------|
| Worker | ❌ 없음 |
| Standard | ❌ 없음 |
| Operator | ✅ OS 구조/룰 |
| Supervisor | ✅ OS 구조/룰 + 시스템 정책 |

### Operator Manifest 내용

```markdown
## Oxios Kernel

You are running inside Oxios Agent OS.

### Spaces
Work is organized into isolated Spaces. Each space has:
- workspace directory (file isolation)
- memory store (scoped knowledge)
- state store (persistent state)
Use `space_list`, `space_create`, `space_archive` to manage spaces.

### Agents
Other agents exist in this OS. Use `a2a_delegate` to assign tasks,
`a2a_send` to send messages, `a2a_query` to request information.
Agent lifecycle: fork → exec → wait → kill.

### Memory
Memory is scoped to spaces. `memory_search` searches current space.
For cross-space access, specify explicit `space_id`.

### Security
All exec operations go through RBAC via AccessManager.
Path sandboxing restricts file access to workspace boundaries.

### Ouroboros
All work follows the Ouroboros protocol: Interview → Seed → Execute → Evaluate → Evolve.
Never execute without a spec.
```

### Supervisor Manifest 추가 내용

```markdown
### System Policy
- Budget enforcement: token and cost limits per agent
- Resource monitor: system overload detection
- Audit trail: immutable logging of all operations
- Circuit breaker: fault tolerance for external calls
```

---

## 4. 전체 System Prompt 조립

```rust
fn build_system_prompt(
    seed: &Seed,
    persona_prompt: Option<&str>,
    profile: &ToolProfile,
    capability_index: &str,
    kernel_manifest: Option<&str>,
) -> String {
    let mut prompt = String::new();

    // 1. Core identity
    prompt.push_str("You are an autonomous agent.\n\n");

    // 2. Task definition (Seed)
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
            prompt.push_str(&format!("- **{}** ({}): {}\n", e.name, e.entity_type, e.description));
        }
    }

    // 3. Persona
    if let Some(pp) = persona_prompt {
        prompt.push_str(&format!("\n## Persona\n{}\n", pp));
    }

    // 4. Capability Index (항상)
    prompt.push_str("\n## Available Capabilities\n");
    prompt.push_str(capability_index);
    prompt.push_str("\nUse the `read` tool to load a capability's SKILL.md for detailed instructions.\n");

    // 5. Kernel Manifest (Operator+)
    if let Some(manifest) = kernel_manifest {
        prompt.push_str("\n");
        prompt.push_str(manifest);
    }

    prompt
}
```

### Profile별 System Prompt 크기

| Profile | 포함 섹션 | 예상 토큰 |
|---------|----------|----------|
| Worker | Seed + Persona + Index | ~2,000 |
| Standard | + Memory 안내 | ~2,200 |
| Operator | + Kernel Manifest | ~3,000 |
| Supervisor | + System Policy | ~3,500 |

---

## 5. AgentRuntime 변경점

### Before

```rust
fn execute(&self, seed: &Seed) {
    let registry = ToolRegistry::new();
    // Tier 1-7 전부 항상 등록
    register_file_ops(&registry);
    register_exec(&registry);
    register_browser(&registry);
    register_all_programs(&registry);  // ← 전부
    register_memory(&registry);        // ← 항상
    register_a2a(&registry);           // ← 항상

    // SKILL.md 전체 주입
    let skills = pm.list_enabled();
    for skill in skills { prompt.push(skill.content); }
}
```

### After

```rust
fn execute(&self, seed: &Seed) {
    let profile = resolve_profile(seed, &self.config);

    // 1. Always-on tools
    let registry = ToolRegistry::new();
    register_file_ops(&registry);
    registry.register(ExecTool::new());
    registry.register(BrowserTool::new(browser_backend));
    registry.register(WebSearchTool::new());

    // 2. Profile-based tools
    match profile {
        Standard | Operator | Supervisor => {
            registry.register(MemoryReadTool::new(mm.clone()));
            registry.register(MemorySearchTool::new(mm.clone()));
        }
        _ => {}
    }
    match profile {
        Operator | Supervisor => {
            registry.register(MemoryWriteTool::new(mm.clone()));
            registry.register(A2aDelegateTool::new(a2a.clone()));
            registry.register(A2aSendTool::new(a2a.clone()));
            registry.register(A2aQueryTool::new(a2a.clone()));
            registry.register(McpToolWrapper::new(mcp.clone()));
        }
        _ => {}
    }
    match profile {
        Supervisor => {
            // budget, resource_monitor, audit tools
        }
        _ => {}
    }

    // 3. Capability Index (항상 주입)
    let index = build_capability_index(pm, mm, a2a, mcp, &profile);

    // 4. Kernel Manifest (Operator+)
    let manifest = match profile {
        Operator => Some(kernel_manifest_operator()),
        Supervisor => Some(kernel_manifest_supervisor()),
        _ => None,
    };

    // 5. System prompt
    let prompt = build_system_prompt(seed, persona_prompt, &profile, &index, manifest.as_deref());

    // program:* tools는 등록 안 함. exec으로 직접 실행.
}
```

---

## 6. Profile 결정 로직

```rust
fn resolve_profile(seed: &Seed, config: &OxiosConfig) -> ToolProfile {
    // 1. Seed에 명시적 profile이 있으면 사용
    if let Some(ref profile) = seed.tool_profile {
        return profile.clone();
    }

    // 2. Persona에 기본 profile이 있으면 사용
    if let Some(ref persona_id) = config.active_persona {
        if let Some(profile) = config.persona_profiles.get(persona_id) {
            return profile.clone();
        }
    }

    // 3. 기본값: Worker
    ToolProfile::Worker
}
```

### Persona에 Profile 연결

```toml
# oxios.toml
[persona.dev]
name = "Dev"
role = "developer"
profile = "worker"         # 코딩만, OS context 없음

[persona.orchestrator]
name = "Orchestrator"
role = "coordinator"
profile = "operator"       # 에이전트 조율, space 관리

[persona.admin]
name = "Admin"
role = "system"
profile = "supervisor"     # 전체 OS 제어
```

---

## 7. Seed 확장

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

---

## 8. Capability Index 빌드

```rust
fn build_capability_index(
    pm: &Option<Arc<ProgramManager>>,
    mm: &Option<Arc<MemoryManager>>,
    a2a: &Option<Arc<A2AProtocol>>,
    mcp: &Option<Arc<McpBridge>>,
    profile: &ToolProfile,
) -> String {
    let mut entries = Vec::new();

    // Programs
    if let Some(ref pm) = pm {
        for program in pm.list_enabled() {
            entries.push(format!(
                "  <capability>\n    \
                 <name>{name}</name>\n    \
                 <category>program</category>\n    \
                 <description>{desc}</description>\n    \
                 <command>{cmd}</command>\n    \
                 <skill>{skill_path}</skill>\n  \
                 </capability>",
                name = program.meta.name,
                desc = program.meta.description,
                cmd = program.meta.name, // command name
                skill_path = program.skill_path().display(),
            ));
        }
    }

    // OS Services (profile에 따라 노출)
    if mm.is_some() && *profile >= ToolProfile::Standard {
        entries.push(
            "  <capability>\n    \
             <name>memory</name>\n    \
             <category>os-service</category>\n    \
             <description>Persistent cross-space memory with semantic search</description>\n    \
             <tools>memory_read, memory_search</tools>\n  \
             </capability>".to_string()
        );
    }

    if a2a.is_some() && *profile >= ToolProfile::Operator {
        entries.push(
            "  <capability>\n    \
             <name>a2a</name>\n    \
             <category>os-service</category>\n    \
             <description>Inter-agent communication and delegation</description>\n    \
             <tools>a2a_delegate, a2a_send, a2a_query</tools>\n  \
             </capability>".to_string()
        );
    }

    if mcp.is_some() && *profile >= ToolProfile::Operator {
        entries.push(
            "  <capability>\n    \
             <name>mcp</name>\n    \
             <category>os-service</category>\n    \
             <description>Model Context Protocol bridge to external tool servers</description>\n  \
             <tools>mcp</tools>\n  \
             </capability>".to_string()
        );
    }

    if entries.is_empty() {
        String::new()
    } else {
        format!("\n<available_capabilities>\n{}\n</available_capabilities>", entries.join("\n"))
    }
}
```

---

## 9. 실행 시나리오

### A. 단순 코딩 (Worker)

```
Seed: "이 함수 리팩토링해줘"
Profile: Worker

Tool Registry: file ops, exec, browser, web_search
System Prompt: Seed + Persona + Capability Index
Kernel Manifest: ❌

에이전트: read, edit, grep 사용. 끝.
```

### B. 코딩 + 웹 검색 (Worker)

```
Seed: "API 문서 확인하고 엔드포인트 구현해"
Profile: Worker

에이전트: browser navigate → text → read → edit.
discover/discover tool 필요 없음. browser는 항상 활성.
```

### C. 코딩 + DB 작업 (Worker)

```
Seed: "DB 스키마 확인하고 마이그레이션 스크립트 작성해"
Profile: Worker

Step 1: Index에서 "sqlite-explorer" 발견
Step 2: read SKILL.md → 사용법 파악
Step 3: exec("sqlite-explorer schema mydb.sqlite")
Step 4: write로 마이그레이션 스크립트 작성
```

### D. 이전 작업 기반 코딩 (Standard)

```
Seed: "저번에 논의한 아키텍처 적용해"
Profile: Standard

Tool Registry: Worker + memory_read, memory_search
System Prompt: Seed + Persona + Index

Step 1: memory_search("아키텍처 논의")
Step 2: memory_read 결과 확인
Step 3: 코딩 작업
```

### E. 멀티 에이전트 조율 (Operator)

```
Seed: "팀에게 작업 분배하고 결과 취합해"
Profile: Operator

Tool Registry: 전체 + A2A
System Prompt: Seed + Persona + Index + Kernel Manifest

Step 1: Kernel Manifest에서 space 구조, A2A 사용법 인지
Step 2: a2a_delegate로 작업 분배
Step 3: a2a_query로 결과 수집
Step 4: 취합
```

---

## 10. 요약

```
┌──────────────────────────────────────────────────────┐
│                   System Prompt                      │
│                                                      │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐ │
│  │   Seed   │  │  Persona  │  │  Capability Index  │ │
│  │  (항상)  │  │  (항상)   │  │     (항상)         │ │
│  └──────────┘  └──────────┘  └────────────────────┘ │
│                                                      │
│  ┌──────────────────────────────────────────────────┐│
│  │          Kernel Manifest (Operator+)             ││
│  │          OS 룰, Space, A2A, Security             ││
│  └──────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────┐
│                   Tool Registry                      │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  Always-on: file ops, exec, browser, web_search│  │
│  └────────────────────────────────────────────────┘  │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  Profile-based: memory, a2a, mcp, budget       │  │
│  │  Worker ❌  Standard ✅  Operator ✅✅           │  │
│  └────────────────────────────────────────────────┘  │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  Programs: 등록 안 함. exec으로 직접 실행       │  │
│  │  Index에서 경로 확인 → read SKILL.md → exec    │  │
│  └────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

| 메트릭 | Before | After |
|--------|--------|-------|
| 프로그램 50개 시 tool schema | 50+ tools 등록 | 0 (exec으로 직접) |
| 프로그램 50개 시 prompt | ~50,000 토큰 (전체 SKILL.md) | ~1,000 토큰 (Index만) |
| 코딩 에이전트 오버헤드 | 전체 context | Seed + Persona + Index |
| 새 capability 추가 | runtime 코드 수정 | Index에 자동 등록 |
| OS 룰 전달 | 없음 | Profile 기반 선택적 |
| 프로그램 인지 | SKILL.md 바다에서 놓침 | Index에서 항상 노출 |
