# RFC-003: Knowledge Base 독립 분리

> **상태:** 설계 완료  
> **날짜:** 2026-05-20  
> **범위:** oxios-markdown, oxios-kernel, oxios-web

---

## 1. 동기

사용자의 삶이 마크다운으로 흐른다. 소설, 기획서, 일정, 일기, 습관 — 전부 `.md` 파일이다.
이 지식 베이스는 **에이전트 OS와 무관하게 독립적으로 존재**해야 한다.
사용자가 노트 에디터로 글을 쓸 때 에이전트 스케줄러, 예산 관리자, 수퍼바이저를 거칠 이유가 없다.

현재 `KnowledgeApi`가 `oxios-kernel` 내부에 있어서:
1. 마크다운 CRUD가 kernel 전체를 의존하게 됨
2. oxios-web이 노트 읽기/쓰기만 할 때도 KernelHandle을 거쳐야 함
3. 지식 베이스 앱 로직과 에이전트 연동 로직이 한 struct에 혼재
4. 같은 내용이 `.md`와 JSON에 이중 저장됨
5. Space별로 knowledge 디렉토리가 분리되어 "하나의 원천" 원칙에 어긋남

---

## 2. 핵심 원칙

```
1. .md 파일이 유일한 원천(Source of Truth)이다
2. 마크다운 앱은 kernel 없이도 동작한다
3. 세션 메모리와 지식 베이스는 별개의 영역이다
4. 지식은 전역 하나다 (Space별 분리 안 함)
5. Semantic search는 kernel 영역이다 (AI 기능 = 에이전트 영역)
6. 에이전트는 기본적으로 knowledge에 쓸 수 있다 (audit trail로 추적)
```

---

## 3. 현재 구조 (Before)

```
┌─ oxios-web ─────────────────────────────────────────────────┐
│                                                              │
│  AppState { kernel: Arc<KernelHandle> }                      │
│                                                              │
│  GET /api/knowledge/file/brain/Rust.md                       │
│    → state.kernel.knowledge.note_read()                      │
│    → kernel_handle::KnowledgeApi                             │
│    → oxios_markdown::VirtualFs                               │
│                                                              │
│  PUT /api/knowledge/file/brain/Rust.md                       │
│    → state.kernel.knowledge.note_write()                     │
│    → KnowledgeApi → VirtualFs (.md 저장)                     │
│    → KnowledgeApi → MemoryManager (JSON 복사본 저장)  ← ❌    │
│                                                              │
│  kernel 없이는 아무것도 할 수 없음                             │
└──────────────────────────────────────────────────────────────┘

의존성:
  oxios-web → oxios-kernel → oxios-markdown
  
  웹 앱이 마크다운 파일 하나를 읽으려면:
  supervisor, scheduler, budget, circuit_breaker, cron, 
  audit_trail, access_manager, persona, MCP, A2A... 전부 거쳐야 함
```

---

## 4. 제안 구조 (After)

### 4.1 전체 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                        oxios-web                             │
│                                                              │
│  AppState {                                                  │
│    knowledge: Arc<KnowledgeBase>,   ← 마크다운 앱 직접 접근  │
│    kernel:   Arc<KernelHandle>,     ← 에이전트 전용          │
│  }                                                           │
│                                                              │
│  knowledge 라우트     → state.knowledge.note_read()          │
│  chat/agent 라우트    → state.kernel (기존)                   │
└──────────────┬───────────────────────────┬───────────────────┘
               │                           │
               │ (직접)                     │ (에이전트 전용)
               ▼                           ▼
┌──────────────────────────┐  ┌────────────────────────────────┐
│   oxios-markdown         │  │   oxios-kernel                  │
│                          │  │                                 │
│  ┌────────────────────┐  │  │  ┌──────────────────────────┐  │
│  │  KnowledgeBase     │  │  │  │  KnowledgeBridge         │  │
│  │  (신규)            │  │  │  │  (기존 KnowledgeApi 대체)│  │
│  │                    │  │  │  │                          │  │
│  │  VirtualFs         │  │  │  │  semantic_search()      │  │
│  │  BacklinkIndex     │  │◄─┼──│  copilot_chat()         │  │
│  │  note CRUD         │  │  │  │  recall_for_context()   │  │
│  │  chat/journal/     │  │  │  │  agent_write()          │  │
│  │  habits/checklist  │  │  │  └──────────────────────────┘  │
│  │  search (파일명)   │  │  │                                 │
│  │  worker/stats      │  │  │  ┌──────────────────────────┐  │
│  └────────────────────┘  │  │  │  MemoryManager (기존)    │  │
│                          │  │  │                          │  │
│  kernel 의존 없음        │  │  │  세션 메모리 (JSON)      │  │
│  AI 의존 없음            │  │  │  팩트/에피소드           │  │
│                          │  │  │  Space별 격리             │  │
└──────────────────────────┘  │  └──────────────────────────┘  │
                               └────────────────────────────────┘
```

### 4.2 데이터 영역 분리

```
~/.oxios/
│
├── knowledge/                    ← 전역 지식 베이스 (유일 원천)
│   ├── brain/                        KnowledgeBase 관리
│   │   ├── Rust.md                   사용자 + 에이전트 모두 접근
│   │   ├── 아키텍처-고민.md
│   │   └── 소설-아이디어.md
│   ├── dev/
│   ├── 일상/
│   ├── Chat.md
│   ├── Later.md
│   ├── Done.md
│   ├── journal/
│   ├── habits/
│   └── config.json
│
├── workspace/                    ← 에이전트 영역 (kernel 관리)
│   ├── sessions/                 세션 데이터
│   ├── seeds/                    Ouroboros 스펙
│   ├── programs/                 설치된 프로그램
│   ├── skills/                   스킬 정의
│   └── spaces/
│       ├── {space-id}/
│       │   └── memory/           Space별 에이전트 메모리 (JSON)
│       │       ├── conversations/
│       │       ├── facts/
│       │       └── episodes/
│       └── ...
│
└── config.toml                   전역 설정
```

**지식 = 사용자의 평생 마크다운. 전역 하나.**
**메모리 = 에이전트의 작업 기억. Space별로 격리.**

둘은 완전히 다른 영역이다:
- 사용자는 knowledge/만 보고 편집한다
- 에이전트는 memory/를 자동으로 관리한다 (사용자가 직접 안 봄)
- 에이전트는 knowledge/를 읽어 컨텍스트로 쓴다

### 4.3 세션 메모리 vs 지식 베이스

```
┌────────────────────────────────────────────────────────────┐
│                  Session Memory                             │
│              (kernel::MemoryManager)                        │
│                                                             │
│  위치: workspace/spaces/{id}/memory/                        │
│  포맷: JSON                                                 │
│  유형: Conversation / Session / Fact / Episode              │
│  주체: 에이전트가 자동으로 생성/관리                          │
│  접근: 사용자 직접 접근 안 함                                │
│  라이프사이클: 세션 단위, 중요도 기반 큐레이션                │
│  Space 스코프: 각 Space마다 독립                             │
│                                                             │
│  = 에이전트의 "작업 메모리"                                   │
│  = "이전에 Rust 프로젝트에서 어떤 버그를 고쳤는지" 같은 것    │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│                  Knowledge Base                             │
│              (oxios-markdown::KnowledgeBase)                 │
│                                                             │
│  위치: ~/.oxios/knowledge/                                  │
│  포맷: .md 파일                                              │
│  유형: 노트 / 일기 / 습관 / 체크리스트 / 소설 / 기획서       │
│  주체: 사용자가 직접 작성 (에이전트도 쓰기 가능)              │
│  접근: 사용자가 웹 에디터로 CRUD                              │
│  라이프사이클: 영속 (사용자가 삭제할 때까지)                   │
│  Space 스코프: 없음. 전역 하나                                │
│                                                             │
│  = 사용자의 "평생 지식"                                      │
│  = "소설 1장 초안", "2026년 5월 일기", "Rust 학습 노트"      │
└────────────────────────────────────────────────────────────┘
```

둘을 연결하는 건 **KnowledgeBridge**뿐이다.
에이전트가 "관련 노트 찾아줘" 할 때 Bridge가 knowledge/를 검색한다.
노트 내용을 MemoryManager에 복사하지 않는다.

---

## 5. Semantic Search: kernel에 둔다

**판단 근거:**

Semantic search = TF-IDF 임베딩 + HNSW 인덱스 + (선택) LLM 임베딩.
이건 "AI가 문서를 이해해서 찾는" 기능이다. oxios-markdown은 순수 파일 시스템
라이브러리로 유지하고, AI 기능은 kernel 영역에 둔다.

**결과:**

| 검색 유형 | 위치 | 이유 |
|-----------|------|------|
| 파일명 fuzzy | oxios-markdown (KnowledgeBase) | 파일 시스템 기능 |
| 백링크 그래프 | oxios-markdown (KnowledgeBase) | 파일 시스템 기능 |
| TF-IDF/HNSW semantic | oxios-kernel (KnowledgeBridge) | AI 기능 |
| Copilot chat | oxios-kernel (KnowledgeBridge) | AI 기능 (LLM 호출) |

KnowledgeBase의 `search()`는 파일명 + 백링크만 검색한다.
KnowledgeBridge의 `semantic_search()`가 .md 파일을 읽어 임베딩을 만들고 HNSW로 검색한다.

**인덱스 저장 위치:** `~/.oxios/knowledge/.index/`

```
~/.oxios/knowledge/
├── .index/
│   ├── vectors.usearch       ← HNSW 인덱스
│   └── key_map.json          ← 파일 경로 매핑
├── brain/
│   └── Rust.md
└── ...
```

인덱스는 지식 베이스와 같은 위치에. .md 파일과 동기화하기 쉽게.

---

## 6. 에이전트 쓰기: 허용하되 audit trail로 추적

**판단:** 에이전트가 knowledge에 쓰는 건 기본적으로 허용한다.

**근거:**
- "oxios는 내 삶의 OS" → 에이전트가 내 일기에 기록하고, 체크리스트를 완료하고, 노트를 정리하는 게 자연스러움
- "허가제"로 만들면 매번 사용자가 승인해야 하는 UX 오버헤드
- 대신 **뭐를 썼는지 추적**해서 사용자가 나중에 확인/롤백할 수 있게

**구현:**
- KnowledgeBase에 이미 `mark_agent_write()` / `is_agent_write()` 있음 (유지)
- KnowledgeBridge.agent_write() 호출 시:
  1. KnowledgeBase.note_write() 실행
  2. mark_agent_write(path) 기록
  3. audit_trail에 "agent wrote to {path}" 로그
- 웹 UI에서 에이전트가 쓴 파일을 시각적으로 구분 (다른 색상/아이콘)

---

## 7. 구체적 변경 사항

### 7.1 oxios-markdown: `KnowledgeBase` 추가

`crates/oxios-markdown/src/knowledge.rs` (신규, ~400 LOC)

현재 `kernel_handle::KnowledgeApi`에서 **kernel 의존이 없는** 모든 메서드를 옮긴다.

```rust
pub struct KnowledgeBase {
    fs: RwLock<VirtualFs>,
    backlinks: RwLock<BacklinkIndex>,
    agent_writes: Mutex<HashSet<String>>,
}
```

옮기는 메서드 (kernel 의존 없음):
- note_read, note_write, note_delete, note_move, note_tree
- backlinks_for, link_graph, index_all
- search (파일명 fuzzy + 백링크만, semantic 없음)
- chat_append, chat_messages, chat_delete, chat_rename, chat_move_to
- journal_add_record, journal_add_emoji, journal_today_path
- habits, habits_write, habits_last_week
- checklist_items, checklist_add, checklist_complete, checklist_remove
- config, set_config
- run_nightly_cleanup, run_scheduled_tasks
- today_report, done_today
- markdown_to_html, auto_emoji
- mark_agent_write, is_agent_write, clear_agent_write

**kernel 의존이 있는 것 (옮기지 않음):**
- copilot_chat (EngineProvider 필요)
- semantic_search (EmbeddingProvider + HNSW 필요)
- index_to_memory (MemoryManager 필요) — 이건 제거

### 7.2 oxios-kernel: `KnowledgeApi` → `KnowledgeBridge` 교체

`crates/oxios-kernel/src/kernel_handle/knowledge_bridge.rs` (신규, ~300 LOC)

```rust
use oxios_markdown::KnowledgeBase;

pub struct KnowledgeBridge {
    knowledge: Arc<KnowledgeBase>,
    embedding: Arc<dyn EmbeddingProvider>,
    engine: Arc<dyn EngineProvider>,
    default_model: String,
    hnsw_index: RwLock<Option<Arc<HnswMemoryIndex>>>,
}

impl KnowledgeBridge {
    // 에이전트 컨텍스트용: 관련 노트 경로+내용 반환
    pub fn recall_for_context(&self, query: &str, limit: usize) -> Result<Vec<(String, String)>>;
    
    // Semantic search over .md files
    pub fn semantic_search(&self, query: &str, limit: usize) -> Result<Vec<SemanticHit>>;
    
    // AI copilot chat
    pub fn copilot_chat(&self, question: &str, context_path: Option<&str>) -> Result<CopilotResponse>;
    
    // 에이전트가 쓸 때 (audit + mark)
    pub fn agent_write(&self, path: &str, content: &str) -> Result<()>;
}
```

### 7.3 oxios-web: AppState 분리

```rust
// Before
pub struct AppState {
    pub kernel: Arc<KernelHandle>,     // 모든 것이 이걸 거침
    ...
}

// After
pub struct AppState {
    pub knowledge: Arc<KnowledgeBase>, // ← NEW: kernel 거치지 않음
    pub kernel: Arc<KernelHandle>,     // 에이전트 채팅/관리 전용
    ...
}
```

- `knowledge_routes.rs`: 전부 `state.knowledge.xxx()`로 변경
- `chat.rs`, `agent_groups.rs` 등: `state.kernel.xxx()` 그대로
- knowledge_routes에서 semantic search 필요 시 → `state.kernel.knowledge.semantic_search()`

### 7.4 KernelHandle 변경

```rust
pub struct KernelHandle {
    // ...
    pub knowledge: KnowledgeBridge,  // KnowledgeApi → KnowledgeBridge
}
```

### 7.5 Space에서 knowledge 분리 제거

```rust
// Before (kernel_handle/mod.rs)
pub async fn activate_space(&self, id: &str) -> Result<()> {
    self.spaces.activate(id).await?;
    self.knowledge.switch_space(&workspace_dir)?;  // ← 제거
    self.knowledge.index_all()?;                   // ← 제거
}

// After
pub async fn activate_space(&self, id: &str) -> Result<()> {
    self.spaces.activate(id).await?;
    // knowledge는 전역. Space 전환해도 안 바뀜
    // Space별 메모리만 격리됨 (MemoryManager가 담당)
}
```

### 7.6 이중 저장 제거

```rust
// Before (knowledge_api.rs)
fn index_to_memory(&self, path: &str, content: &str) {
    let entry = MemoryEntry { memory_type: MemoryType::Knowledge, ... };
    memory.remember(entry);  // ← .md 내용을 JSON에 복사
}

// After: 이 메서드 자체를 삭제
// .md가 유일 원천. semantic search는 KnowledgeBridge가 .md를 읽어서 처리
```

### 7.7 MemoryType::Knowledge 제거

```rust
pub enum MemoryType {
    Conversation,
    Session,
    Fact,
    Episode,
    // Knowledge ← 삭제
}
```

### 7.8 KnowledgeTool 수정

```rust
// Before: 매 요청마다 KnowledgeApi를 새로 만듦
fn make_api(&self) -> KnowledgeApi {
    KnowledgeApi::new(self.knowledge_dir.clone(), ...)
}

// After: Arc<KnowledgeBase>를 공유
pub struct KnowledgeTool {
    knowledge: Arc<KnowledgeBase>,
    bridge: Arc<KnowledgeBridge>,
}
```

---

## 8. 의존성 그래프 변화

```
Before:

  oxios-web
    ├── oxios-kernel
    │     ├── oxios-markdown
    │     ├── oxi-sdk, oxi-ai
    │     ├── usearch, gix, wasmtime, oxibrowser-core ...
    │     └── (40+ heavy dependencies)
    └── (기타)

  노트 읽기 = oxios-kernel(+40 deps)을 거쳐야 함

After:

  oxios-web
    ├── oxios-markdown           ← 가벼운 마크다운 앱
    │     └── (serde, chrono, walkdir 정도)
    └── oxios-kernel             ← 에이전트 전용
          ├── oxios-markdown
          ├── oxi-sdk, oxi-ai
          └── (heavy deps)

  노트 읽기 = oxios-markdown만 거치면 됨
  에이전트 = oxios-kernel 거침 (기존대로)
```

---

## 9. 마이그레이션 경로

### Phase 1: oxios-markdown에 KnowledgeBase 추가
- `knowledge.rs` 신규 작성
- 기존 KnowledgeApi에서 kernel 의존 없는 메서드 복사
- 테스트 작성
- **기존 코드 변경 없음** (동작 영향 없음)

### Phase 2: oxios-web 전환
- AppState에 `knowledge: Arc<KnowledgeBase>` 추가
- knowledge_routes의 handler를 `state.knowledge`로 변경
- **기존 KnowledgeApi는 아직 삭제 안 함** (KnowledgeTool이 참조)

### Phase 3: kernel KnowledgeApi → KnowledgeBridge 교체
- `knowledge_bridge.rs` 신규 작성
- KernelHandle의 `knowledge` 필드 타입 변경
- KnowledgeTool이 KnowledgeBase + KnowledgeBridge 사용하도록 수정
- 기존 `knowledge_api.rs` 삭제
- `index_to_memory` (이중 저장) 제거
- MemoryType::Knowledge 제거

### Phase 4: Space 분리 정리
- `activate_space`에서 knowledge.switch_space 제거
- 전역 knowledge 경로를 `~/.oxios/knowledge/`로 고정
- 기존 Space별 knowledge 디렉토리 마이그레이션 스크립트 (선택)

---

## 10. 파일 변경 요약

| 파일 | 액션 | 설명 |
|------|------|------|
| `crates/oxios-markdown/src/knowledge.rs` | 신규 | KnowledgeBase (~400 LOC) |
| `crates/oxios-markdown/src/lib.rs` | 수정 | knowledge 모듈 + re-export 추가 |
| `crates/oxios-kernel/src/kernel_handle/knowledge_bridge.rs` | 신규 | KnowledgeBridge (~300 LOC) |
| `crates/oxios-kernel/src/kernel_handle/knowledge_api.rs` | 삭제 | KnowledgeApi (878 LOC) |
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | 수정 | knowledge 필드 타입 변경 |
| `crates/oxios-kernel/src/memory/mod.rs` | 수정 | MemoryType::Knowledge 제거 |
| `crates/oxios-kernel/src/tools/kernel/knowledge_tool.rs` | 수정 | KnowledgeBase + Bridge 사용 |
| `channels/oxios-web/src/server.rs` | 수정 | AppState에 knowledge 추가 |
| `channels/oxios-web/src/routes/knowledge_routes.rs` | 수정 | state.knowledge 사용 |
| `channels/oxios-web/Cargo.toml` | 수정 | oxios-markdown 직접 의존 추가 |
| `src/kernel.rs` | 수정 | KnowledgeBase 생성 로직 추가 |

**LOC 변화:** KnowledgeApi(878 LOC) 삭제 → KnowledgeBase(400 LOC) + KnowledgeBridge(300 LOC) = 700 LOC. 순감 178 LOC. kernel에서 878 LOC 제거.

---

## 11. 확인된 결정 사항

| 결정 | 선택 | 근거 |
|------|------|------|
| Semantic search 위치 | kernel (KnowledgeBridge) | AI 기능은 kernel 영역. markdown은 순수 파일 시스템 유지 |
| Semantic index 저장 | `~/.oxios/knowledge/.index/` | .md 파일과 같은 위치. 동기화 편의 |
| 에이전트 쓰기 | 기본 허용 + audit trail | UX 오버헤드 최소화. 대신 추적으로 투명성 확보 |
| 지식 스코프 | 전역 하나 | Space는 에이전트 컨텍스트. 지식은 삶 전체 |
| 메모리 스코프 | Space별 격리 | 각 Space의 에이전트가 독립적으로 기억 |
| oxios-markdown AI 의존 | 없음 | 순수 라이브러리 유지. 컴파일 가볍게 |
