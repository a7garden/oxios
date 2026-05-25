# Oxios AgentOS — 최종 아키텍처 설계

> 연구 결과 종합: Tool Discovery, OS Capability Models, Semantic Retrieval
> 이전 설계의 모든 약점을 해결.

---

## 0. 이전 설계의 약점 — 재확인

| 약점 | 근본 원인 | 해결 전략 |
|------|----------|----------|
| 닫힌 Profile (4개 enum) | 권한을 enum으로 하드코딩 | seL4 스타일 Capability Token 도입 |
| 에이전트가 언제 OS tool을 써야 할지 모름 | Manifest가 정적 텍스트 | 의미 기반 Tool Retrieval |
| 50+ 프로그램 토큰 폭발 | 전체 Index를 항상 주입 | RAG-MCP 패턴: 검색 후 선택적 주입 |
| Program Tool 등록 오버헤드 | 50개 tool schema를 registry에 | exec 단일 tool + 의미 검색 |
| KernelHandle God Object 우려 | 11개 도메인 파사드 | Thin facade + Capability 위임 |

---

## 1. 핵심 통찰: 연구에서 배운 것

### 1.1 LLM Tool 선택의 현실 (Tool Discovery Research)

```
Anthropic 공식 권장: tool ≤ 20개 per request
OpenAI 한계: 128개 hard limit, 50개부터 정확도 저하
연구 합의: semantic retrieval로 top-K만 주입하는 게 정답
```

**ToolLLM (ICLR 2024):** 16,000+ API에서 two-stage retrieve-then-use로 90%+ 달성.
**RAG-MCP (2025):** MCP 서버 수백 개에서 embedding 검색으로 관련 서버만 로드.
**Dynamic ReAct (2025):** 매 스텝마다 필요한 tool만 동적 로드/언로드.

### 1.2 OS Capability 모델의 현실 (OS Research)

```
seL4: 권한 enum이 없다. Capability Token이 곧 권한.
Genode: 재귀적 샌드박스. 부모가 자식에게 subset 위임.
Android: Intent 시스템. 선언적 역량 광고 + 런타임 해석.
E Language: "Only connectivity begets connectivity."
```

**핵심:** 닫힌 enum(CAP_READ, CAP_WRITE)이 아니라,
**객체에 대한 능력 토큰(Capability Token)**이 권한이다.
토큰은 위임 가능, 제한 가능(attenuate), 회수 가능(revoke).

### 1.3 최적의 조합

```
에이전트 권한 = seL4 Capability Token (enum 없이도 확장 가능)
Tool 선택 = Semantic Retrieval (프로그램 100개도 OK)
OS 제어 = KernelHandle (단일 경로)
컨텍스트 주입 = RAG-MCP (검색 후 필요한 것만)
```

---

## 2. 핵심 개념: Capability Token

### 2.1 정의

```rust
/// 에이전트의 권한을 나타내는 불가위조 토큰.
/// 특정 리소스에 대한 특정 권한을 인코딩.
/// 소유 = 권한. 위임 가능. 회수 가능.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// 능력 고유 ID (불가위조, 커널 발행)
    id: CapabilityId,
    /// 대상 리소스
    resource: ResourceRef,
    /// 허용된 권한
    rights: Rights,
    /// 발행자 (어떤 에이전트/커널이 만들었는지)
    issuer: Issuer,
    /// 파생 트리 (부모 능력)
    parent: Option<CapabilityId>,
}

/// 리소스 참조 — 능력이 가리키는 대상
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceRef {
    /// 커널 도메인 API
    KernelDomain { domain: String },         // "space", "agent", "memory", ...
    /// 특정 프로그램
    Program { name: String },                // "sqlite-explorer"
    /// 특정 Space
    Space { id: SpaceId },
    /// 특정 에이전트
    Agent { id: AgentId },
    /// 실행 환경
    Exec { mode: ExecMode },
    /// 브라우저
    Browser,
    /// A2A 통신
    A2a,
    /// MCP 서버
    Mcp { server: String },
}

/// 권한 집합 — 비트 플래그
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rights(u32);

impl Rights {
    pub const NONE: Rights = Rights(0);
    pub const READ: Rights = Rights(1 << 0);
    pub const WRITE: Rights = Rights(1 << 1);
    pub const EXECUTE: Rights = Rights(1 << 2);
    pub const DELEGATE: Rights = Rights(1 << 3);  // 하위 에이전트에게 위임 가능
    pub const ALL: Rights = Rights(0xFFFF);
}

/// 능력 공간 — 에이전트가 보유한 능력 토큰의 집합
#[derive(Debug, Clone, Default)]
pub struct CSpace {
    capabilities: HashMap<CapabilityId, Capability>,
}
```

### 2.2 왜 enum이 아닌가?

```
Before (닫힌 Profile):
  enum ToolProfile { Worker, Standard, Operator, Supervisor }
  → 새 역할 추가 시 enum 수정
  → "Worker + cron만" 같은 조합 불가

After (열린 Capability):
  let agent_caps = CSpace::from([
      Capability::new(ResourceRef::KernelDomain { domain: "exec" }, Rights::ALL),
      Capability::new(ResourceRef::Browser, Rights::ALL),
      Capability::new(ResourceRef::Program { name: "sqlite-explorer" }, Rights::EXECUTE),
      // 필요한 것만. 더도 말도.
  ]);
  → 어떤 조합이든 가능
  → 새 리소스 추가 시 Capability만 발행
  → 커널 수정 불필요
```

### 2.3 Capability 생명주기

```
1. 발행 (Issue):
   커널 또는 상위 에이전트가 Capability를 생성.
   예: kernel.issue(ResourceRef::Space { id: "default" }, Rights::READ | Rights::WRITE)

2. 위임 (Delegate):
   DELEGATE 권한이 있는 에이전트가 하위 에이전트에게 subset을 전달.
   예: supervisor.delegate(agent_id, cap_id, Rights::READ)  // READ만 위임

3. 제한 (Attenuate):
   원본 권한의 subset으로 새 Capability 생성.
   예: cap.attenuate(Rights::READ)  // 읽기 전용으로 제한

4. 회수 (Revoke):
   부모 Capability 삭제 → 모든 파생 Capability 자동 회수.
   seL4의 Capability Derivation Tree와 동일.
   예: kernel.revoke(parent_cap_id)  // 자식 에이전트의 모든 권한 회수
```

---

## 3. 의미 기반 Tool Retrieval

### 3.1 문제 재정의

```
이전: "Profile로 어떤 tool을 보여줄지 결정" (닫힌 세계)
이후: "Goal과 의미적으로 관련된 tool을 검색" (열린 세계)
```

### 3.2 ToolRetriever

```rust
/// 프로그램/스킬/OS 서비스의 의미 검색 엔진.
/// embedding 모듈(TF-IDF 또는 dense)을 사용하여 goal과 관련된 capability를 검색.
pub struct ToolRetriever {
    /// 검색 인덱스: capability 설명의 embedding
    index: Vec<ToolEmbedding>,
    /// embedding 제공자
    embedder: Arc<dyn EmbeddingProvider>,
    /// 커널 핸들 (실제 tool/프로그램 접근용)
    kernel: Arc<KernelHandle>,
}

struct ToolEmbedding {
    /// 능력 식별자
    name: String,
    /// 분류: "os-tool", "program", "os-service"
    category: String,
    /// 한 줄 설명 (검색 대상)
    description: String,
    /// 설명의 embedding 벡터
    vector: Vec<f32>,
    /// SKILL.md 경로 (program인 경우)
    skill_path: Option<PathBuf>,
    /// 명령 경로 (program인 경우)
    command: Option<String>,
}

impl ToolRetriever {
    /// 커널에서 설치된 모든 program과 OS tool을 인덱싱.
    pub async fn build_index(&mut self) {
        self.index.clear();

        // 1. 항상 활성 OS tool
        let os_tools = vec![
            ("exec", "os-tool", "Execute shell commands or structured binaries"),
            ("browser", "os-tool", "Headless web browser: navigate, click, type, evaluate, read pages"),
            ("web_search", "os-tool", "Search the web for information"),
            ("memory_read", "os-tool", "Read persistent memory entries"),
            ("memory_search", "os-tool", "Semantic search across memory store"),
            ("memory_write", "os-tool", "Store new memory entries"),
            ("space", "os-tool", "Manage work spaces: list, create, archive, merge"),
            ("agent", "os-tool", "Manage agents: list, kill, check budgets"),
            ("a2a", "os-tool", "Inter-agent communication: delegate, send, query"),
            ("persona", "os-tool", "Persona management: list, set active"),
            ("program", "os-tool", "Program management: install, uninstall, list"),
            ("cron", "os-tool", "Schedule recurring tasks"),
            ("security", "os-tool", "Audit trail, verify integrity, RBAC"),
            ("budget", "os-tool", "Token budget management per agent"),
            ("resource", "os-tool", "System resource monitoring: CPU, memory, disk"),
        ];
        for (name, cat, desc) in os_tools {
            let vector = self.embedder.embed(desc).await;
            self.index.push(ToolEmbedding {
                name: name.to_string(),
                category: cat.to_string(),
                description: desc.to_string(),
                vector,
                skill_path: None,
                command: None,
            });
        }

        // 2. 설치된 프로그램
        for program in self.kernel.extensions.list_programs().await {
            let desc = &program.meta.description;
            let vector = self.embedder.embed(desc).await;
            self.index.push(ToolEmbedding {
                name: program.meta.name.clone(),
                category: "program".to_string(),
                description: desc.clone(),
                vector,
                skill_path: Some(program.skill_path()),
                command: Some(program.meta.name.clone()),
            });
        }

        // 3. MCP 서버
        for server in self.kernel.mcp.list_servers() {
            let desc = &server.description;
            let vector = self.embedder.embed(desc).await;
            self.index.push(ToolEmbedding {
                name: format!("mcp:{}", server.name),
                category: "mcp".to_string(),
                description: desc.clone(),
                vector,
                skill_path: None,
                command: None,
            });
        }
    }

    /// goal과 관련된 상위 K개 capability 검색.
    pub async fn retrieve(&self, goal: &str, top_k: usize) -> Vec<RetrievedTool> {
        let query_vec = self.embedder.embed(goal).await;

        let mut scored: Vec<_> = self.index.iter()
            .map(|te| {
                let score = cosine_similarity(&query_vec, &te.vector);
                RetrievedTool {
                    name: te.name.clone(),
                    category: te.category.clone(),
                    description: te.description.clone(),
                    score,
                    skill_path: te.skill_path.clone(),
                    command: te.command.clone(),
                }
            })
            .collect();

        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        scored.truncate(top_k);
        scored
    }
}
```

### 3.3 검색 결과 → System Prompt 주입

```
Goal: "DB 스키마 확인하고 마이그레이션 스크립트 작성해"

ToolRetriever.retrieve(goal, top_k=8):
  1. sqlite-explorer  (0.91) — Query SQLite databases
  2. exec             (0.85) — Execute shell commands
  3. browser          (0.42) — Headless web browser
  4. memory_search    (0.38) — Semantic memory search
  5. github-manager   (0.31) — GitHub PR management
  ...

System Prompt에 주입:
<relevant_capabilities>
  <capability score="0.91">
    <name>sqlite-explorer</name>
    <category>program</category>
    <description>Query SQLite databases, inspect schemas</description>
    <command>sqlite-explorer</command>
    <skill>/path/to/SKILL.md</skill>
  </capability>
  <capability score="0.85">
    <name>exec</name>
    <category>os-tool</category>
    <description>Execute shell commands or structured binaries</description>
  </capability>
  ... (top 8개만)
</relevant_capabilities>
```

### 3.4 비판: embedding이 정말 필요한가?

**우려:** TF-IDF로 "DB 스키마" → "sqlite-explorer"를 매칭할 수 있을까?

**현실:**
- TF-IDF는 키워드 매칭에 강함. "database" → "sqlite-explorer" OK
- 하지만 "데이터베이스 구조 확인" (한국어) → 매칭 실패 가능
- Dense embedding (multilingual)이 한국어 goal도 처리 가능
- **Oxios는 이미 `embedding` 모듈을 가지고 있음** — 전환 비용 낮음

**해결:** Phase 1은 TF-IDF로 시작. 프로그램 50개면 TF-IDF도 충분.
프로그램 200+ 또는 한국어 goal이 많아지면 dense embedding으로 전환.

### 3.5 이전의 "Capability Index"와의 차이

```
Before: 전체 프로그램 목록을 항상 주입 (50개 = ~1,000 토큰)
After:  goal과 관련된 것만 주입 (top 8개 = ~200 토큰)

Before: 코딩 에이전트에게 sqlite-explorer가 보임 (필요 없는데)
After:  "리팩토링해줘"라는 goal에는 sqlite-explorer가 안 보임
        "DB 스키마 확인해"라는 goal에는 sqlite-explorer가 1등으로 보임
```

---

## 4. Capability 기반 Tool 등록

### 4.1 CSpace → Tool Registry 매핑

```rust
/// 에이전트의 CSpace(Capability 공간)에서 Tool Registry를 구성.
/// 닫힌 Profile enum 대신, CSpace에 있는 Capability만큼 tool이 등록됨.
pub fn register_tools_from_cspace(
    registry: &ToolRegistry,
    kernel: &KernelHandle,
    cspace: &CSpace,
) {
    // 항상 활성: 파일 작업 (oxi-agent 제공)
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GrepTool::new());
    registry.register(FindTool::new());
    registry.register(LsTool::new());
    registry.register(WebSearchTool::new(search_cache));

    // CSpace 기반: 가진 Capability에 해당하는 tool만 등록
    for (_, cap) in &cspace.capabilities {
        match &cap.resource {
            ResourceRef::Exec { .. } if cap.rights.has(Rights::EXECUTE) => {
                registry.register(ExecTool::from_kernel(kernel));
            }
            ResourceRef::Browser if cap.rights.has(Rights::EXECUTE) => {
                registry.register(BrowserTool::from_kernel(kernel));
            }
            ResourceRef::KernelDomain { domain } if cap.rights.has(Rights::READ) => {
                match domain.as_str() {
                    "memory" => {
                        registry.register(MemoryReadTool::from_kernel(kernel));
                        registry.register(MemorySearchTool::from_kernel(kernel));
                    }
                    "space" => registry.register(SpaceTool::from_kernel(kernel)),
                    "agent" => registry.register(AgentTool::from_kernel(kernel)),
                    "a2a" => registry.register(A2aTool::from_kernel(kernel)),
                    "persona" => registry.register(PersonaTool::from_kernel(kernel)),
                    "program" => registry.register(ProgramTool::from_kernel(kernel)),
                    "cron" => registry.register(CronTool::from_kernel(kernel)),
                    "security" => registry.register(SecurityTool::from_kernel(kernel)),
                    "budget" => registry.register(BudgetTool::from_kernel(kernel)),
                    "resource" => registry.register(ResourceTool::from_kernel(kernel)),
                    "mcp" => registry.register(McpTool::from_kernel(kernel)),
                    _ => {}
                }
            }
            ResourceRef::KernelDomain { domain } if cap.rights.has(Rights::WRITE) => {
                match domain.as_str() {
                    "memory" => registry.register(MemoryWriteTool::from_kernel(kernel)),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
```

### 4.2 비판: CSpace가 Profile보다 복잡하지 않은가?

**우려:** Profile enum은 4개 값이면 끝인데, CSpace는 Capability를 직접 관리해야 함.

**현실:**
- Profile은 "모든 Worker가 같은 tool을 가진다"는 가정. 현실적으로 틀림.
  코딩 Worker와 소설 작성 Worker는 같은 tool을 쓰지만,
  OS 제어 Worker는 다른 tool이 필요.
- CSpace는 **에이전트마다 다른 권한**을 자연스럽게 표현.
- 복잡도는 `CapabilityTemplate`으로 숨김:

```rust
/// 자주 쓰는 Capability 조합을 템플릿으로 제공.
/// 직접 CSpace를 조립할 필요 없이 템플릿 선택으로 끝.
pub struct CapabilityTemplate;

impl CapabilityTemplate {
    /// 기본 작업자: 파일 + exec + browser
    pub fn worker() -> CSpace {
        CSpace::from([
            Capability::new(ResourceRef::Exec { mode: ExecMode::All }, Rights::ALL),
            Capability::new(ResourceRef::Browser, Rights::EXECUTE),
        ])
    }

    /// 기억하는 작업자: worker + memory
    pub fn standard() -> CSpace {
        let mut cspace = Self::worker();
        cspace.insert(Capability::new(
            ResourceRef::KernelDomain { domain: "memory".into() },
            Rights::READ,
        ));
        cspace
    }

    /// OS 제어자: standard + space + agent + a2a + program
    pub fn operator() -> CSpace {
        let mut cspace = Self::standard();
        for domain in &["space", "agent", "a2a", "persona", "program", "mcp"] {
            cspace.insert(Capability::new(
                ResourceRef::KernelDomain { domain: domain.to_string() },
                Rights::ALL,
            ));
        }
        cspace.insert(Capability::new(
            ResourceRef::KernelDomain { domain: "memory".into() },
            Rights::READ | Rights::WRITE,
        ));
        cspace
    }

    /// 시스템 관리자: 전체
    pub fn supervisor() -> CSpace {
        let mut cspace = Self::operator();
        for domain in &["security", "budget", "resource", "cron"] {
            cspace.insert(Capability::new(
                ResourceRef::KernelDomain { domain: domain.to_string() },
                Rights::ALL,
            ));
        }
        cspace
    }

    /// 커스텀: 특정 프로그램만 + 기본 도구
    pub fn with_programs(programs: &[&str]) -> CSpace {
        let mut cspace = Self::worker();
        for name in programs {
            cspace.insert(Capability::new(
                ResourceRef::Program { name: name.to_string() },
                Rights::EXECUTE,
            ));
        }
        cspace
    }
}
```

→ **템플릿 4개로 기존 Profile과 동일한 사용성.**
하지만 필요하면 언제든 커스텀 CSpace를 직접 조립 가능. 열린 세계.

---

## 5. 전체 아키텍처

### 5.1 다이어그램

```
┌──────────────────────────────────────────────────────────────────┐
│                        Agent (LLM)                               │
│                                                                  │
│  System Prompt:                                                  │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │ 1. Seed (goal, constraints)                              │     │
│  │ 2. Persona                                               │     │
│  │ 3. Relevant Capabilities ← ToolRetriever 검색 결과       │     │
│  │ 4. Kernel Manifest ← CSpace 기반 자동 생성               │     │
│  └─────────────────────────────────────────────────────────┘     │
│                                                                  │
│  Tool Registry:                                                  │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │ 파일 ops + exec + browser (항상)                          │     │
│  │ + CSpace에 있는 Capability에 해당하는 tool만               │     │
│  │ + 검색된 상위 K개 program은 Index에 포함                   │     │
│  └─────────────────────────────────────────────────────────┘     │
└───────────────────────────┬──────────────────────────────────────┘
                            │
               ┌────────────▼────────────┐
               │    Tool Registry        │
               │                         │
               │  항상: file ops, exec,   │
               │        browser, search   │
               │  CSpace: 해당 tool만     │
               └────────────┬────────────┘
                            │
               ┌────────────▼────────────┐
               │    KernelHandle         │
               │    (syscall table)      │
               │                         │
               │  모든 tool이 경유        │
               └────────────┬────────────┘
                            │
               ┌────────────▼────────────┐
               │    Kernel Modules       │
               └─────────────────────────┘

에이전트 생성 흐름:
  ┌─────────┐    ┌──────────┐    ┌───────────────┐    ┌────────────┐
  │   Seed   │ →  │ Template │ →  │    CSpace     │ →  │ Tool       │
  │ + Goal   │    │ 선택     │    │ (Capability   │    │ Registry   │
  │          │    │          │    │  Token 집합)  │    │ 구성       │
  └─────────┘    └──────────┘    └───────────────┘    └─────┬──────┘
                                                            │
                      ┌─────────────────────────────────────▼──┐
                      │          ToolRetriever                  │
                      │  goal embedding → top-K 검색            │
                      │  → Relevant Capabilities 생성            │
                      │  → System Prompt에 주입                  │
                      │  → Kernel Manifest 자동 생성             │
                      └────────────────────────────────────────┘
```

### 5.2 AgentRuntime.execute() 최종

```rust
fn execute(&self, seed: &Seed) -> Result<ExecuteResult> {
    let kernel = self.kernel_handle.as_ref()
        .context("KernelHandle not available")?;

    // 1. CSpace 결정
    //    - Seed에 명시적 CSpace가 있으면 사용
    //    - 없으면 Persona의 role → Template → CSpace
    //    - 없으면 기본: worker template
    let cspace = resolve_cspace(seed, &self.config);

    // 2. Tool Registry 구성 (CSpace 기반)
    let registry = ToolRegistry::new();
    register_tools_from_cspace(&registry, kernel, &cspace);

    // 3. Tool Retrieval: goal과 관련된 capability 검색
    let retrieved = self.tool_retriever.retrieve(&seed.goal, 8).await;

    // 4. Relevant Capabilities 주입용 텍스트 생성
    let capabilities_prompt = format_retrieved_capabilities(&retrieved);

    // 5. Kernel Manifest 자동 생성 (CSpace 기반)
    let manifest = build_kernel_manifest(&cspace);

    // 6. System Prompt 조립
    let persona_prompt = self.persona_manager
        .as_ref()
        .and_then(|pm| pm.active_system_prompt());
    let system_prompt = build_system_prompt(
        seed,
        persona_prompt.as_deref(),
        &capabilities_prompt,
        manifest.as_deref(),
    );

    // 7. Memory blend
    let system_prompt = if let Some(ref mm) = self.memory_manager {
        mm.blend_into_prompt(&mm.recall(&seed.goal).await?, &system_prompt)
    } else {
        system_prompt
    };

    // 8. AgentLoop 실행
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

## 6. Kernel Manifest 자동 생성 (CSpace 기반)

Profile enum 대신 CSpace에서 자동 생성:

```rust
pub fn build_kernel_manifest(cspace: &CSpace) -> Option<String> {
    // OS 제어 Capability가 하나도 없으면 Manifest 불필요
    let has_os_caps = cspace.capabilities.values().any(|cap| {
        matches!(cap.resource, ResourceRef::KernelDomain { .. })
            || matches!(cap.resource, ResourceRef::Space { .. })
            || matches!(cap.resource, ResourceRef::Agent { .. })
            || matches!(cap.resource, ResourceRef::A2a)
    });
    if !has_os_caps {
        return None;
    }

    let mut m = String::from(
        "## Oxios Agent OS\n\n\
         You are running inside Oxios, an Agent Operating System.\n\n"
    );

    // CSpace에 있는 Capability에 대해서만 설명
    if cspace.has_domain("space") {
        m.push_str(
            "### Spaces\n\
             Work is organized into isolated Spaces.\n\
             Use `space` tool: list, get, create, archive, merge, restore.\n\n"
        );
    }

    if cspace.has_domain("agent") || cspace.has_domain("a2a") {
        m.push_str(
            "### Agents\n\
             Other agents exist in this OS.\n\
             Use `agent` tool: list, kill.\n\
             Use `a2a` tool: delegate tasks, send messages.\n\n"
        );
    }

    if cspace.has_domain("memory") {
        m.push_str(
            "### Memory\n\
             Persistent memory scoped to spaces.\n\
             Use `memory_search` to find relevant context.\n\n"
        );
    }

    if cspace.has_domain("security") {
        m.push_str(
            "### Security\n\
             All operations are audit-logged.\n\
             Use `security` tool: verify chain, query log.\n\n"
        );
    }

    m.push_str(
        "### Programs\n\
         Read the capabilities above for available programs.\n\
         Use `read` to load SKILL.md for usage.\n\
         Use `exec` to run program commands.\n\n"
    );

    m.push_str(
        "### Ouroboros Protocol\n\
         All work follows: Interview → Seed → Execute → Evaluate → Evolve.\n"
    );

    Some(m)
}
```

---

## 7. Capability 위임 (seL4 + Genode 패턴)

### 7.1 에이전트 스폰 시 Capability 위임

```rust
impl KernelHandle {
    /// 새 에이전트를 스폰하고 Capability를 위임.
    /// 부모의 Capability subset만 전달 (Genode 패턴).
    pub async fn spawn_agent(
        &self,
        parent_cspace: &CSpace,
        task: &str,
        delegate_caps: &[ResourceRef],
        attenuate_to: Rights,
    ) -> Result<(AgentId, CSpace)> {
        // 1. 부모에서 위임 가능한지 확인
        let mut child_cspace = CSpace::default();
        for resource in delegate_caps {
            if let Some(parent_cap) = parent_cspace.find_by_resource(resource) {
                if !parent_cap.rights.has(Rights::DELEGATE) {
                    anyhow::bail!("No DELEGATE right for {:?}", resource);
                }
                // 2. 제한된 Capability 생성 (attenuation)
                let child_cap = parent_cap.derive(attenuate_to);
                child_cspace.insert(child_cap);
            }
        }

        // 3. 에이전트 생성
        let agent_id = self.agents.fork(task).await?;

        // 4. CSpace 등록 (Capability Derivation Tree에 기록)
        self.capability_tree.register(&agent_id, &child_cspace);

        Ok((agent_id, child_cspace))
    }

    /// 에이전트 종료 시 모든 Capability 회수 (seL4 패턴).
    pub async fn kill_agent(&self, agent_id: &AgentId) -> Result<()> {
        // CDT에서 해당 에이전트의 subtree 전체 회수
        self.capability_tree.revoke_subtree(agent_id);
        // 에이전트 종료
        self.agents.kill(&agent_id.to_string()).await?;
        Ok(())
    }
}
```

### 7.2 Android Intent 패턴 (동적 발견)

```rust
/// 에이전트가 "무엇을 하고 싶은지"를 선언하면
/// 커널이 적절한 프로그램을 찾아 연결.
pub struct ToolIntent {
    /// 하고 싶은 동작
    pub action: String,          // "query", "commit", "deploy", ...
    /// 데이터 타입
    pub data_type: String,       // "database", "git", "web", ...
    /// 추가 컨텍스트
    pub description: String,
}

impl ToolRetriever {
    /// Intent 기반 tool 검색 (Android Intent Resolution 패턴).
    pub async fn resolve_intent(&self, intent: &ToolIntent) -> Vec<RetrievedTool> {
        let query = format!("{} {} {}", intent.action, intent.data_type, intent.description);
        self.retrieve(&query, 5).await
    }
}
```

---

## 8. 실행 시나리오

### 8.1 코딩 에이전트

```
Seed: "이 함수 리팩토링해줘"
CSpace: CapabilityTemplate::worker()
  → Exec, Browser만 가진 CSpace

ToolRetriever.retrieve("이 함수 리팩토링해줘", 8):
  → exec (0.82), browser (0.45), memory_search (0.31), ...

Registry: file ops, exec, browser, web_search
System Prompt: Seed + Persona + Top 8 capabilities + Manifest ❌
Token: ~2,000
```

### 8.2 소설 작성 에이전트

```
Seed: "판타지 소설 한 챕터 써줘"
CSpace: CapabilityTemplate::worker()

ToolRetriever.retrieve("판타지 소설 한 챕터 써줘", 8):
  → browser (0.52), web_search (0.48), exec (0.22), ...

Registry: file ops, exec, browser, web_search
System Prompt: Seed + Persona + Top 8 + Manifest ❌
Token: ~2,000

→ OS의 존재를 모름. 글만 씀.
```

### 8.3 DB 작업 + 코딩

```
Seed: "DB 스키마 확인하고 마이그레이션 스크립트 작성해"
CSpace: CapabilityTemplate::worker()

ToolRetriever.retrieve("DB 스키마 확인하고 마이그레이션", 8):
  → sqlite-explorer (0.91), exec (0.85), browser (0.42), ...

Registry: file ops, exec, browser, web_search
Relevant Capabilities:
  sqlite-explorer (0.91) — Query SQLite databases
  exec (0.85) — Execute shell commands
  ...

에이전트: "sqlite-explorer가 있네. SKILL.md 읽어보자"
→ read(sqlite-explorer/SKILL.md)
→ exec("sqlite-explorer schema mydb.sqlite")
→ write로 마이그레이션 스크립트 작성
```

### 8.4 OS 제어 에이전트

```
Seed: "팀에게 작업 분배하고 결과 취합해"
CSpace: CapabilityTemplate::operator()
  → Exec, Browser, Memory(RW), Space, Agent, A2A, Persona, Program, MCP

ToolRetriever.retrieve("팀에게 작업 분배하고 결과 취합", 8):
  → a2a (0.88), agent (0.82), space (0.71), memory (0.55), ...

Registry: file ops, exec, browser, web_search,
          memory(RW), space, agent, a2a, persona, program, mcp
System Prompt: Seed + Persona + Top 8 + Manifest ✅ (Space, Agent, A2A 설명)
Token: ~3,000
```

### 8.5 커스텀: Worker + cron만

```
Seed: "매일 오전 9시에 테스트 실행해"
CSpace:
  CapabilityTemplate::worker() +
  Capability::new(KernelDomain("cron"), Rights::ALL)

→ 닫힌 Profile로는 불가능한 조합
→ CSpace로는 자연스럽게 가능
```

---

## 9. Before vs After (최종)

| 문제 | Before (Profile) | After (Capability) |
|------|-----------------|-------------------|
| 닫힌 권한 | 4개 enum 하드코딩 | 열린 Capability Token |
| 커스텀 조합 | 불가능 | 자유로운 조합 |
| Tool 선택 | 정적 Index (전체 주입) | Semantic Retrieval (top-K) |
| 50+ 프로그램 | ~1,000 토큰 (전체) | ~200 토큰 (관련 것만) |
| OS Manifest | Profile 기반 정적 | CSpace 기반 자동 |
| 에이전트 위임 | 없음 | seL4 Capability Derivation |
| 하위 에이전트 권한 | 없음 | Attenuation + Revocation |
| 프로그램 발견 | Index 나열 | 의미 검색 |
| 코딩 에이전트 오버헤드 | OS tool 전부 노출 | 관련 tool만 노출 |

---

## 10. 템플릿 = 기본값, CSpace = 자유

```
CapabilityTemplate::worker()       → 기본 코딩
CapabilityTemplate::standard()     → 기억하는 코딩
CapabilityTemplate::operator()     → OS 제어
CapabilityTemplate::supervisor()   → 전체 관리
CapabilityTemplate::with_programs(["sqlite", "github"])  → 커스텀

// 또는 직접 조립:
let cspace = CSpace::from([
    Capability::new(ResourceRef::Exec { mode: ExecMode::All }, Rights::ALL),
    Capability::new(ResourceRef::Browser, Rights::EXECUTE),
    Capability::new(ResourceRef::Program { name: "sqlite-explorer" }, Rights::EXECUTE),
    // 이것만. 더도 말도.
]);
```

기존 Profile의 단순함은 템플릿으로 보존하면서,
열린 세계의 유연성은 CSpace로 확보.

---

## 11. 구현 로드맵

```
Phase 1: KernelHandle 확장 (기반)
  ├── ExecApi, BrowserApi, A2aApi 추가
  └── 11개 도메인 파사드 조립

Phase 2: Capability 시스템 (핵심)
  ├── Capability, CSpace, Rights 정의
  ├── CapabilityTemplate (worker/standard/operator/supervisor)
  ├── Capability Derivation Tree (위임/회수)
  └── resolve_cspace() (Seed/Persona → CSpace)

Phase 3: Tool Retrieval (검색)
  ├── ToolRetriever (embedding 기반 의미 검색)
  ├── build_index() (kernel tools + programs + MCP)
  ├── retrieve() (goal → top-K capabilities)
  └── format_retrieved_capabilities() (XML)

Phase 4: Tool 재구성 (KernelHandle 경유)
  ├── 기존 tool 마이그레이션 (직접 참조 → KernelHandle)
  ├── 신규 OS tool 추가 (space, agent, persona, ...)
  └── register_tools_from_cspace()

Phase 5: System Prompt 통합
  ├── build_system_prompt() (통합)
  ├── build_kernel_manifest() (CSpace 기반)
  ├── AgentRuntime.execute() 재작성
  └── Seed.cspace 필드 추가
```

---

## 12. 자기비판 — 여전히 남는 약점

| 약점 | 인정 | 대응 |
|------|------|------|
| TF-IDF로 한국어 goal 매칭 불안정 | ✅ | Phase 1은 키워드 보완, Phase 2에서 dense embedding |
| Capability Token 관리 오버헤드 | ✅ | Template으로 숨김. 직접 조립은 고급 사용자만 |
| ToolRetriever 인덱스 빌드 시간 | ✅ | 프로그램 100개면 < 1초. 스타트업에 1회만 |
| embedding 품질이 tool 선택 품질 결정 | ✅ | description 작성 품질이 핵심. 가이드라인 필요 |
| seL4 Capability Derivation Tree 구현 복잡 | ✅ | Phase 1은 flat CSpace. Tree는 Phase 3 |
| Manifest가 여전히 하드코딩된 텍스트 | ✅ | CSpace 기반이라 자동이긴 하지만, 내용은 정적 |

### 이 설계가 풀지 못하는 것

1. **LLM이 SKILL.md를 안 읽으면 프로그램을 제대로 못 씀**
   → Manifest가 "read로 읽어라"고 안내하지만, 강제는 불가.

2. **embedding이 이상한 tool을 추천하면**
   → LLM이 무시하면 OK. 잘못 쓰면 문제. top-K를 너무 크게 하지 않으면 완화.

3. **CSpace를 직접 조립하는 사용자의 실수**
   → Template를 기본으로 제공. 직접 조립은 opt-in.
