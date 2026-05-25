# 남은 작업 설계

> `cargo check` 통과. 다음 단계는 AgentRuntime을 새 아키텍처로 전환.

---

## 현재 상태

### 완료된 것
- ✅ KernelHandle 11개 도메인 (exec, browser, a2a 추가)
- ✅ Capability 시스템 (types, template, resolve)
- ✅ ToolRetriever (semantic search engine)
- ✅ Kernel Tools (space, agent, persona, cron, security, budget, resource)
- ✅ Seed.cspace_hint 필드
- ✅ workspace 빌드 통과

### 아직 안 된 것
- ❌ AgentRuntime이 새 아키텍처를 사용하지 않음
- ❌ 기존 Tool들이 KernelHandle을 우회 (직접 참조)
- ❌ CSpace 기반 tool 등록
- ❌ ToolRetriever가 system prompt에 연결
- ❌ ProgramTool을 별도 등록하지 않고 exec로만 해결

---

## 작업 1: AgentRuntime 교체

### 현재 (구 구조)
```rust
// agent_runtime.rs — run_agent_loop()

// Tier 1: oxi native (항상)
registry.register(ReadTool::new());
registry.register(WriteTool::new());
// ...

// Tier 2: exec (직접 참조)
let tool = ExecTool::for_agent(cfg, access, agent_name);

// Tier 3: program tools (전부 등록)
for program in &programs { registry.register(ProgramTool::new(...)); }

// Tier 5: memory (직접 참조)
registry.register(MemoryWriteTool::new(mm.clone()));

// Tier 6: A2A (직접 참조)
registry.register(A2aDelegateTool::new(a2a.clone()));

// Tier 7: browser (직접 참조)
registry.register(BrowserTool::new(backend.clone()));
```

### 목표 (새 구조)
```rust
// agent_runtime.rs — run_agent_loop()

// 1. CSpace 결정
let cspace = resolve_cspace(
    seed.cspace_hint.as_deref(),
    persona_role,
    "worker",
    agent_id,
);

// 2. CSpace 기반 tool 등록 (KernelHandle 경유)
register_tools_from_cspace(&registry, kernel, &cspace);

// 3. ToolRetriever로 관련 capability 검색
let query_vec = retriever.embedder().embed(&seed.goal).await;
let retrieved = retriever.retrieve(&query_vec, 8);

// 4. System Prompt 조립
let capabilities_xml = format_capability_index(&retrieved);
let active_domains = cspace.active_domains();
let manifest = build_kernel_manifest(&active_domains);
let system_prompt = build_system_prompt(
    seed, persona_prompt, &capabilities_xml, manifest.as_deref(),
);
```

### 변경 파일
- `crates/oxios-kernel/src/agent_runtime.rs` — execute() + run_agent_loop() 재작성

### 핵심 변경점

**AgentRuntime 필드 교체:**
```
Before:
  program_manager: Option<Arc<ProgramManager>>
  mcp_bridge: Option<Arc<McpBridge>>
  memory_manager: Option<Arc<MemoryManager>>
  exec_config: Option<Arc<ExecConfig>>
  exec_access: Option<Arc<Mutex<AccessManager>>>
  a2a: Option<Arc<A2AProtocol>>
  browser_backend: Option<Arc<OxibrowserBackend>>

After:
  kernel_handle: Arc<KernelHandle>          ← 단일 참조
  tool_retriever: Arc<ToolRetriever>         ← 의미 검색
  persona_manager: Option<Arc<PersonaManager>> ← 유지
```

→ 7개 필드가 3개로 축소. 모든 것이 KernelHandle을 통해 간다.

**AgentLoopContext 필드 교체:**
```
Before: 12개 필드 (각 모듈 Arc를 개별 전달)
After: 4개 필드
  kernel_handle: Arc<KernelHandle>
  tool_retriever: Arc<ToolRetriever>
  cspace: CSpace
  persona_prompt: Option<String>
```

**kernel.rs에서 AgentRuntime 생성 변경:**
```
Before:
  AgentRuntime::new(provider, model_id)
    .with_program_manager(pm)
    .with_oxios_config(config)
    .with_persona_manager(persona)
    .with_mcp_bridge(mcp)
    .with_memory_manager(mm)
    .with_exec_config(exec_cfg, access)
    .with_a2a(a2a)
    .with_browser(backend)

After:
  AgentRuntime::new(provider, model_id, kernel_handle, tool_retriever)
    .with_persona_manager(persona)
```

---

## 작업 2: register_tools_from_cspace() 구현

### 파일
`crates/oxios-kernel/src/tools/registration.rs` (신규)

### 로직
```rust
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
    registry.register(GetSearchResultsTool::new(search_cache));

    // CSpace에 있는 ResourceRef에 따라 등록
    for (_, cap) in &cspace.capabilities {
        match &cap.resource {
            ResourceRef::Exec { .. } if cap.rights.has(Rights::EXECUTE) => {
                registry.register(ExecTool::from_kernel(kernel));
            }
            ResourceRef::Browser if cap.rights.has(Rights::EXECUTE) => {
                registry.register(BrowserTool::from_kernel(kernel));
            }
            ResourceRef::KernelDomain { domain } => {
                match domain.as_str() {
                    "memory" if cap.rights.has(Rights::READ) => {
                        registry.register(MemoryReadTool::from_kernel(kernel));
                        registry.register(MemorySearchTool::from_kernel(kernel));
                    }
                    "memory" if cap.rights.has(Rights::WRITE) => {
                        registry.register(MemoryWriteTool::from_kernel(kernel));
                    }
                    "space" => registry.register(SpaceTool::from_kernel(kernel)),
                    "agent" => registry.register(KernelAgentTool::from_kernel(kernel)),
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
            ResourceRef::Program { .. } if cap.rights.has(Rights::EXECUTE) => {
                // Program은 별도 tool 등록 안 함
                // ToolRetriever가 Index에 포함 → exec으로 직접 실행
            }
            _ => {}
        }
    }
}
```

---

## 작업 3: 기존 Tool 마이그레이션

기존 tool들이 내부 모듈을 직접 참조하지 않고
KernelHandle의 API를 참조하도록 변경.

### ExecTool
```
Before: ExecTool::for_agent(cfg, access, name)
        → ExecConfig + AccessManager 직접

After:  ExecTool::from_kernel(kernel)
        → kernel.exec.config() + kernel.exec.access_manager()
```

### BrowserTool
```
Before: BrowserTool::new(backend.clone())
        → OxibrowserBackend 직접

After:  BrowserTool::from_kernel(kernel)
        → kernel.browser.backend()
```

### MemoryTools
```
Before: MemoryReadTool::new(mm.clone())
        → MemoryManager 직접

After:  MemoryReadTool::from_kernel(kernel)
        → kernel.agents의 memory 메서드
```

### A2aTools
```
Before: A2aDelegateTool::new(a2a.clone(), agent_id)
        → A2AProtocol 직접

After:  A2aTool::from_kernel(kernel)
        → kernel.a2a.protocol()
```

### 변경 파일
- `crates/oxios-kernel/src/tools/exec_tool.rs`
- `crates/oxios-kernel/src/tools/browser/browser_tool.rs`
- `crates/oxios-kernel/src/tools/memory_tools.rs`
- `crates/oxios-kernel/src/tools/a2a_tools.rs`
- `crates/oxios-kernel/src/tools/mcp_tool.rs`

각 파일에 `from_kernel(&KernelHandle)` 생성자 추가.
기존 생성자는 deprecated 표시하고 유지 (하위 호환성).

---

## 작업 4: System Prompt 교체

### 파일
`crates/oxios-kernel/src/agent_runtime.rs` 내 build_system_prompt()

### 현재
```rust
fn build_system_prompt(seed, skill_contents, persona_prompt) {
    // Seed (goal, constraints, criteria)
    // SKILL.md 전체 주입 ← 문제
    // Persona
}
```

### 목표
```rust
fn build_system_prompt(
    seed: &Seed,
    persona_prompt: Option<&str>,
    capabilities_xml: &str,        // ToolRetriever 검색 결과
    kernel_manifest: Option<&str>,  // CSpace 기반 자동 생성
) -> String {
    // 1. Core Identity
    // 2. Seed (goal, constraints, criteria, ontology)
    // 3. Persona
    // 4. Relevant Capabilities (XML)
    // 5. Kernel Manifest (있으면)
}
```

SKILL.md 전체 주입 제거 → ToolRetriever가 검색한 상위 K개만 XML로 주입.

---

## 작업 5: kernel.rs 연결

### 파일
`src/kernel.rs`

### 변경점

**Kernel 구조체에 tool_retriever 추가:**
```rust
pub struct Kernel {
    // ... 기존 필드 ...
    tool_retriever: OnceLock<Arc<ToolRetriever>>,
}
```

**AgentRuntime 생성 변경:**
```rust
// Before:
let agent_runtime = AgentRuntime::new(provider, model_id)
    .with_program_manager(pm)
    .with_oxios_config(config)
    // ... 7개 with_*

// After:
let kernel_handle = self.handle();
let tool_retriever = self.tool_retriever();
let agent_runtime = AgentRuntime::new(
    provider, model_id,
    kernel_handle, tool_retriever,
).with_persona_manager(Arc::new(persona_manager.clone()));
```

**ToolRetriever 초기화:**
```rust
fn tool_retriever(&self) -> Arc<ToolRetriever> {
    self.tool_retriever.get_or_init(|| {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let embedder = Arc::new(TfIdfEmbeddingProvider::new());
            let mut retriever = ToolRetriever::new(embedder);
            // 인덱스 빌드: OS tools + programs
            // ...
            Arc::new(retriever)
        })
    }).clone()
}
```

---

## 작업 6: 테스트 수정

### 남은 테스트 오류
1. `lib test` — SpaceSource import (detection.rs) ✅ 이미 수정
2. `lib test` — supervisor.rs Seed에 cspace_hint ✅ 이미 수정
3. `integration_tests` — 괄호 문제 ✅ 이미 수정
4. `integration_tests` — 테스트 내부에서 AgentRuntime 생성 방식 변경 필요

### 테스트 전략
- 단위 테스트: 새 모듈별로 추가
  - capability types/template/resolve
  - retrieval
  - kernel tools (space, agent, persona, ...)
- 통합 테스트: AgentRuntime 교체 후 업데이트

---

## 구현 순서

```
Step 1: registration.rs (CSpace → Tool Registry 매핑)
  ├── register_tools_from_cspace() 구현
  └── 항상 활성 tool + CSpace 조건부 tool

Step 2: 기존 Tool 마이그레이션
  ├── ExecTool::from_kernel()
  ├── BrowserTool::from_kernel()
  ├── MemoryTools::from_kernel()
  ├── A2aTools::from_kernel()
  └── McpTool::from_kernel()

Step 3: AgentRuntime 재작성
  ├── 필드 교체 (7개 → 3개)
  ├── execute() 재작성
  ├── run_agent_loop() 재작성
  └── build_system_prompt() 교체

Step 4: kernel.rs 연결
  ├── ToolRetriever 초기화
  ├── AgentRuntime 생성 변경
  └── with_* 메서드 제거

Step 5: 테스트 수정
  ├── lib test 오류 해결
  ├── integration test 업데이트
  └── 새 모듈 단위 테스트
```

---

## 위험 요소

| 위험 | 대응 |
|------|------|
| AgentRuntime 재작성 범위가 큼 | 한 번에 전체 교체. 점진적 불가 |
| run_agent_loop가 spawn_blocking 안에서 실행 | KernelHandle은 Send+Sync 필요 확인 |
| ToolRetriever가 async인데 spawn_blocking 안에서 호출 | embed 미리 계산 후 결과만 전달 |
| 기존 with_* API 제거 시 하위 호환성 | deprecated 표시 후 제거 |
