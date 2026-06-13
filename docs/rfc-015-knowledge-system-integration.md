# RFC-015: Knowledge System Integration

> **Status**: Implemented  
> **Date**: 2026-06-13  
> **Scope**: `oxios-kernel`, `oxios-ouroboros`, binary crate (`src/kernel.rs`)

## Problem

사용자가 Web UI에서 해커뉴스 기사 3건을 가져오라고 했다. 에이전트가 잘 수행했다. 그 후 사용자가 "지식 저장소에 저장해줘"라고 했다. 에이전트는 `memory/` 디렉토리를 뒤지며 `memory/facts/`, `memory/preferences.md`를 보여주고, 뭘 저장할지 물어봤다.

에이전트는 `knowledge` tool과 `memory_*` tools를 같은 것으로 혼용했다. 이 사건은 네 가지 구조적 결함의 합작이다.

### 결함 1: Kernel manifest와 capability 도메인에 knowledge가 빠져 있다

Tool 등록 경로는 `kernel_bridge.rs::register_tools()` → `builtin/mod.rs::register_all_kernel_tools()`이며, KnowledgeTool은 **무조건(unconditional) 등록**된다. CSpace 게이팅은 존재하지 않는다.

그러나 `CapabilityTemplate`에는 knowledge 도메인이 없다:

```
worker()     → Exec, Browser
standard()   → worker + memory(READ)
operator()   → standard + Space, Agent, A2A, Persona, Program, MCP, memory(WRITE)
supervisor() → operator + Security, Budget, Resource, Cron
```

결과적으로:

- `CSpace::active_domains()`가 `"knowledge"`를 반환하지 않음
- `build_kernel_manifest()`가 knowledge 섹션을 생성하지 않음
- Kernel Manifest에 knowledge 설명이 표시되지 않음

에이전트 입장에서는 knowledge tool이 존재하지만, Kernel Manifest에서 이 도메인에 대한 설명을 볼 수 없다.

### 결함 2: ToolRetriever 인덱스에 knowledge가 없다

`src/kernel.rs`의 `builtin_tools` 배열에 knowledge가 빠져 있다. `memory_read`, `memory_write`, `memory_search`는 다 들어있는데 `knowledge`만 없다.

에이전트가 seed goal을 임베딩해서 ToolRetriever에 질의할 때, "저장", "노트", "마크다운" 같은 의미가 knowledge tool과 연결되지 않는다. `<available_capabilities>` XML에 knowledge가 나타나지 않는다.

### 결함 3: Tool description이 의미적 구분을 하지 않는다

에이전트가 보는 것:

```
memory_write: "Write a memory entry that persists across sessions.
               Use this to save important facts, episodes, or knowledge for future reference."

knowledge:    "Manage markdown knowledge notes. Actions: read, write, delete, move, tree,
               search, backlinks, checklist_items, ..."
```

`memory_write`의 description이 *"save ... knowledge for future reference"* 라고 한다. 사용자가 "지식 저장소에 저장해줘"라고 했을 때, LLM이 `memory_write`를 선택하는 것은 합리적 추론이다.

한편 `knowledge`의 description은 *"Manage markdown knowledge notes"* 라고만 한다. "노트"라는 단어는 vague하다. 이것이 사용자의 자료를 파일로 저장하는 vault라는 점이 명확하지 않다.

### 결함 4: 시스템 프롬프트가 memory와 knowledge를 같은 카테고리로 묶는다

`build_system_prompt()`:

```rust
"- **Kernel tools**: memory, knowledge, agent management, etc.\n\n\
```

memory와 knowledge가 "Kernel tools"라는 같은 줄에 나열되어 있다. LLM 입장에서는 둘이 같은 계통의 tool이라고 인식한다. description이나 manifest로 아무리 구분해도, 분류 카테고리가 같으면 효과가 반감된다.

### 두 시스템의 본질

| | Knowledge | Memory |
|---|---|---|
| 저장소 | `~/.oxios/knowledge/` (마크다운 파일) | SQLite (kernel workspace) |
| 성격 | 자료 (documents, articles, notes) | 기억 (facts, preferences, episodes) |
| 구조 | 위키/볼트 — 디렉토리, 백링크, 저널 | 벡터 DB — tiered decay, HNSW |
| 도구 | `knowledge` (단일 tool, action dispatch) | `memory_read`, `memory_write`, `memory_search` |
| UI | Web UI Knowledge 패널 (파일 편집기) | 세션 내 암묵적 참조 |
| 사용자 인지 | "내 노트", "내 자료" | 사용자가 직접 접근하지 않음 |

## Design

### §1. CapabilityTemplate에 knowledge 도메인 추가

**파일**: `crates/oxios-kernel/src/capability/template.rs`

`worker()`에 knowledge 도메인을 추가한다. worker가 모든 템플릿의 기저이므로 standard/operator/supervisor가 자동 상속한다.

```rust
pub fn worker() -> Self {
    let mut t = Self { caps: Vec::new() };
    t.caps.push((
        ResourceRef::Exec { mode: "shell".into() },
        Rights::EXECUTE | Rights::READ,
    ));
    t.caps.push((ResourceRef::Browser, Rights::READ | Rights::EXECUTE));
    t.caps.push((
        ResourceRef::KernelDomain { domain: "knowledge".into() },
        Rights::READ | Rights::WRITE,
    ));
    t
}
```

이 변경으로 두 가지가 즉시 효과를 발휘한다:

1. `active_domains()`가 `"knowledge"`를 반환 → `build_kernel_manifest()`가 knowledge 섹션 생성
2. 모든 템플릿이 worker에서 파생되므로, 어떤 role이든 knowledge 접근 보장

> **참고**: KnowledgeTool은 `register_all_kernel_tools()`에서 이미 무조건 등록되므로, 이 변경은 tool 등록 자체에는 영향을 주지 않는다. 순수히 manifest 가시성과 CSpace 도메인 일관성을 위한 것이다.

### §2. Kernel Manifest에 knowledge 도메인 추가, memory description 개선

**파일**: `crates/oxios-kernel/src/tools/retrieval.rs`

`KNOWN_DOMAINS`에 `"knowledge"`를 추가한다:

```rust
const KNOWN_DOMAINS: &[&str] = &[
    "space", "agent", "a2a", "memory", "knowledge",
    "security", "budget", "resource", "program",
];
```

`domain_description()`에 knowledge 케이스를 추가하고, memory의 설명도 같이 개선한다:

```rust
fn domain_description(domain: &str) -> &'static str {
    match domain {
        "space" => "Filesystem workspace management and conversation buffers.",
        "agent" => "Agent lifecycle, runtime, and supervisor.",
        "a2a" => "Agent-to-agent communication and delegation.",
        "memory" => "Internal agent recall — facts, preferences, behavioral patterns. Not user-visible.",
        "knowledge" => "Personal markdown vault — documents, articles, notes, journal. File-based with backlinks and full-text search.",
        "security" => "RBAC access control and audit trail.",
        "budget" => "Token and cost budget enforcement.",
        "resource" => "System resource monitoring and overload protection.",
        "program" => "Installable OS-level programs and tools.",
        _ => "Unknown domain.",
    }
}
```

에이전트가 시스템 프롬프트에서 두 도메인을 나란히 보게 된다:

```markdown
### memory
Internal agent recall — facts, preferences, behavioral patterns. Not user-visible.

### knowledge
Personal markdown vault — documents, articles, notes, journal. File-based with backlinks and full-text search.
```

"Not user-visible" vs "documents, articles, notes" — 의미적 구분이 manifest 레벨에서 확립된다.

### §3. ToolRetriever 인덱스에 knowledge 등록

**파일**: `src/kernel.rs`

`builtin_tools` 배열에 knowledge를 추가한다:

```rust
let builtin_tools: &[(&str, &str, &str)] = &[
    ("exec", "os-tool", "Execute shell commands or structured binaries in workspace"),
    ("read", "os-tool", "Read file contents"),
    ("write", "os-tool", "Write content to files"),
    ("edit", "os-tool", "Make precise text edits in files"),
    ("grep", "os-tool", "Search file contents with regex"),
    ("find", "os-tool", "Find files by name or pattern"),
    ("ls", "os-tool", "List directory contents"),
    ("web_search", "os-tool", "Search the web for information"),
    ("memory_read", "os-tool", "Recall persistent memories"),
    ("memory_write", "os-tool", "Store persistent memories"),
    ("memory_search", "os-tool", "Semantic search over memories"),
    ("knowledge", "os-service", "Personal markdown vault — save, read, search documents and notes"),
    ("browser", "os-tool", "Headless browser for web automation and scraping"),
];
```

ToolRetriever가 "저장", "마크다운", "노트", "자료" 같은 쿼리 임베딩에 대해 knowledge를 상위권으로 반환한다.

### §4. Tool description 정정 — 자기 설명 원칙

**원칙**: 각 tool의 description은 **자기 저장소의 물리적 특성**을 명시한다. 다른 tool의 이름을 언급하지 않는다.

#### memory_write

**파일**: `crates/oxios-kernel/src/tools/memory_tools.rs`

```
Before:
  "Write a memory entry that persists across sessions. Use this to save
   important facts, episodes, or knowledge for future reference."

After:
  "Store a recallable agent memory — facts about the user, behavioral
   patterns, session observations, preference corrections. Internal to
   the agent. Persisted across sessions via SQLite + HNSW vector index."
```

변경 포인트:
- "knowledge" 단어 제거 — 이 단어가 사용자의 "지식 저장소" 발화와 충돌
- "Internal to the agent" 명시 — 사용자 자료가 아님을 표현
- "SQLite + HNSW vector index" — 저장소의 물리적 특성 명시

또한 `parameters_schema`의 `memory_type` enum에서 `"knowledge"` 값을 제거한다:

```
Before:
  "enum": ["fact", "episode", "knowledge"]

After:
  "enum": ["fact", "episode"]
```

`MemoryType::Knowledge`는 내부적으로 HNSW 색인 목적으로 존재하지만, agent-facing enum에서는 `memory_write`와 `knowledge` tool의 혼동을 방지하기 위해 노출하지 않는다.

#### knowledge

**파일**: `crates/oxios-kernel/src/tools/builtin/knowledge_tool.rs`

```
Before:
  "Manage markdown knowledge notes. Actions: read, write, delete, move,
   tree, search, backlinks, checklist_items, checklist_add, ..."

After:
  "Personal markdown vault — documents, articles, notes, journal entries.
   File-based with backlinks, full-text search, and directory structure.
   Read, write, search, and organize user content as markdown files."
```

변경 포인트:
- "markdown vault", "user content" — 사용자의 자료를 다루는 tool임을 명시
- "File-based", "directory structure" — 파일 시스템 기반임을 명시
- action 목록은 parameters_schema에 이미 정의되어 있으므로 description에서 제거

LLM이 "저장해줘"를 받았을 때의 판단 근거:

| 신호원 | memory가 보내는 신호 | knowledge가 보내는 신호 |
|--------|---------------------|----------------------|
| Kernel Manifest | "Internal agent recall ... Not user-visible" | "Personal markdown vault ... documents, articles" |
| ToolRetriever | "Store persistent memories" | "save, read, search documents and notes" |
| Tool description | "Internal to the agent", "SQLite + HNSW" | "user content", "File-based" |
| System prompt 카테고리 | "Memory tools — agent's internal recall" | "Knowledge — personal markdown vault" |

네 가지 신호가 모두 knowledge를 가리킨다. tool 간 크로스레퍼런스 없이도 충분하다.

### §5. 시스템 프롬프트의 tool 분류 분리

**파일**: `crates/oxios-kernel/src/agent_runtime.rs` — `build_system_prompt()`

```
Before:
  "- **Kernel tools**: memory, knowledge, agent management, etc.\n\n\

After:
  "- **Memory tools**: memory_read, memory_write, memory_search — agent's internal recall\n\
   - **Knowledge**: knowledge — personal markdown vault for documents and notes\n\
   - **Kernel tools**: agent, project, persona, cron, security, budget, resource\n\n\
```

이것은 예시가 아니다. tool의 **분류 카테고리**를 정확히 하는 것이다. 현재 "Kernel tools"라는 하나의 버킷에 memory, knowledge, agent management가 다 들어있는 것은 분류 체계의 오류다.

memory와 knowledge는 저장 대상과 물리적 저장소가 완전히 다른 별개의 하위시스템이다. 같은 카테고리에 묶으면 LLM이 둘을 같은 계통으로 인식한다.

## Implementation Order

```
① capability/template.rs       → worker()에 knowledge RW 추가
                                 (manifest 가시성 및 CSpace 일관성)
② tools/retrieval.rs           → KNOWN_DOMAINS에 "knowledge" 추가
                                 domain_description("knowledge") 추가
                                 domain_description("memory") 개선
③ src/kernel.rs                → builtin_tools에 knowledge ToolEntry 추가
④ memory_tools.rs              → memory_write description 정정
                                 memory_type enum에서 "knowledge" 제거
⑤ knowledge_tool.rs            → knowledge description 정정
⑥ agent_runtime.rs             → build_system_prompt의 tool 분류를
                                 Memory / Knowledge / Kernel 세 줄로 분리
⑦ 테스트                        → 각 변경에 대한 단위 테스트 업데이트
```

각 단계는 독립적이며, 어떤 순서로 구현해도 빌드가 깨지지 않는다. 다만 ①을 먼저 적용해야 ②의 manifest 변경이 실제로 에이전트에게 노출된다.

## What This Does NOT Do

- **시스템 프롬프트에 사용 예시나 지시문을 추가하지 않음** — tool description, domain description, kernel manifest, tool 분류 카테고리라는 네 가지 구조적 메타데이터로 해결.
- **자연어 의도 분류를 추가하지 않음** — "저장해줘" vs "기억해둬" 구분은 LLM이 네 가지 신호를 종합해서 스스로 판단.
- **새로운 tool을 추가하지 않음** — 기존 `KnowledgeTool`, `MemoryWriteTool` 그대로 사용.
- **MemoryType::Knowledge enum variant를 제거하지 않음** — memory 시스템 내부의 knowledge 타입은 HNSW 색인 목적이며, 사용자가 "지식 저장소"라고 부르는 것과 다른 개념. 다만 agent-facing `memory_type` enum에서는 제거하여 `memory_write`와 `knowledge` tool 간 혼동을 방지한다.
- **ToolRetriever의 다른 tool description을 변경하지 않음** — memory_read, memory_search의 description도 개선 여지가 있지만, 이 RFC의 범위를 벗어남.

## Verification

1. `CapabilityTemplate::worker().build().active_domains()` → `"knowledge"` 포함
2. `build_kernel_manifest(&["memory", "knowledge"])` → 두 도메인이 명확히 구분되는 설명 출력
3. ToolRetriever에서 "저장", "마크다운", "노트" 쿼리에 knowledge가 상위 반환
4. ToolRetriever에서 "기억", "선호", "패턴" 쿼리에 memory가 상위 반환
5. `memory_write` description에 "knowledge" 단어가 포함되지 않음
6. `memory_write`의 `parameters_schema` enum에서 `"knowledge"` 제거됨
7. `knowledge` description에 "vault", "user content", "File-based" 포함
8. `build_system_prompt()` 출력에서 Memory / Knowledge / Kernel이 각각 별도 줄에 표시됨

## Real-World Test Case

실제 발생한 대화:

```
User: "해커뉴스 베스트 3건 가져와줘"
Agent: [web_search → 3건 결과 반환] ✅

User: "지식 저장소에 저장해줘"
Agent (현재): memory/facts/ 확인 → memory_write 선택 시도 → 혼동 → 질문
Agent (개선 후):
  1. ToolRetriever가 "저장" 임베딩에 knowledge 상위 반환
  2. <available_capabilities> XML에 knowledge 포함
  3. Kernel Manifest에 "Personal markdown vault — documents, articles, notes"
  4. Tool 분류가 Knowledge / Memory로 분리되어 있음
  5. knowledge tool description이 "user content as markdown files"
  6. memory_write의 memory_type enum에서 "knowledge"가 제거되어 선택지에서 사라짐
  → knowledge tool, write 액션 선택
  → ~/.oxios/knowledge/hackernews/best-2026-06-13.md 생성
```
