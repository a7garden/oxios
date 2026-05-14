# Space System Design

> **대화가 유일한 인터페이스. 내부는 자율. 투명성은 선택.**

## 1. Motivation

Oxios는 에이전트 OS다. 그런데 현재 모든 작업이 flat한 세계에서 일어난다 —
메모리, 워크스페이스, 에이전트 컨텍스트가 하나로 뒤섞여 있다.

사용자가 "oxios 버그 잡아줘" 다음에 "저녁 뭐 먹지?"라고 말하면,
두 작업은 완전히 다른 컨텍스트인데 시스템은 이를 구분하지 못한다.

**Space**는 이 문제를 해결하는 논리적 파티션이다.
OS가 사용자의 대화를 분석해 자동으로 적절한 Space에서 작업을 처리하고,
각 Space는 독립된 메모리와 워크스페이스를 가진다.

## 2. Design Principles

### P1: Conversation is the only interface
사용자는 클릭, 드래그, 이름 지정, 설정 변경을 할 필요가 없다.
대화만으로 모든 것이 이루어진다.
Space 생성, 전환, 병합 — 모두 OS가 알아서 한다.

### P2: Internal autonomy
내부에서 Space를 어떻게 나눌지, 어떤 에이전트 팀을 구성할지,
서브에이전트를 어떻게 배치할지 — 모두 OS가 자율적으로 결정한다.
"oxios 버그 잡아줘" 한마디면, OS가 알아서:
- oxios Space를 식별하고
- 코드리뷰 에이전트, 테스트 에이전트를 배치하고
- 결과를 사용자에게 자연스럽게 전달한다.

### P3: Transparency on demand
사용자는 내부를 몰라도 되지만, **원하면 언제든 투명하게 볼 수 있다**.
Web UI나 TUI 대시보드에서:
- 현재 활성 Space 목록
- 각 Space의 에이전트 활동
- 메모리 상태
- 작업 이력

이 모든 걸 실시간으로 볼 수 있고, 개입할 수도 있다.
Unix의 `/proc`이나 Activity Monitor와 같은 철학.

### P4: Single channel
사용자는 항상 하나의 인터페이스(채널)와 대화한다.
Telegram 방이 1개, Web이 1개.
Space 전환이 일어나도 사용자의 대화 창은 그대로다.
OS 내부에서만 라우팅이 바뀐다.

## 3. Core Concept: Space

```
Space = 논리적 작업 파티션
  ├── 고유 ID (UUID)
  ├── 이름 (자동 생성 또는 사용자가 대화에서 지정)
  ├── 독립된 Memory (해당 Space 관련 메모리만)
  ├── 독립된 Workspace (해당 Space의 작업 디렉토리)
  ├── 연결된 리소스 (파일시스템 경로, git repo 등)
  ├── 활성화 이력 (생성 시간, 마지막 사용 시간)
  └── 메타데이터 (태그, 생성 출처)
```

### 3.1 Space Source (생성 방식)

| 출처 | 트리거 | 예시 |
|------|--------|------|
| `AutoResource` | 파일시스템 경로 감지 | "/projects/oxios" 경로 언급 → "oxios" Space |
| `AutoTopic` | 주제 전환 감지 | 개발 → 일상 대화 전환 → "일상" Space |
| `Manual` | 사용자 명시 | "새 스페이스 만들어" (드물겠지만 지원) |

### 3.2 Space Isolation (계층적 격리)

| 리소스 | 격리 수준 | 설명 |
|--------|----------|------|
| Memory | **완전 격리** | 각 Space는 독립된 메모리 (facts, episodes, conversations) |
| Workspace | **완전 격리** | 각 Space는 자체 작업 디렉토리 |
| Agent | 전역 | 에이전트는 전역 리소스, Space가 필요시 요청해서 사용 |
| Persona | 전역 | 페르소나는 전역, Space가 선택해서 사용 |
| Program | 전역 | 프로그램은 전역, Space의 컨텍스트에 따라 활성화 여부 결정 |
| EventBus | 태깅 | 전역이지만 모든 이벤트에 `space_id` 태그 부착, 필터링 가능 |

## 4. Architecture

```
                     ┌──────────────────┐
                     │     Gateway      │  단일 인터페이스
                     │  (Web/CLI/TG...) │  사용자는 여기만 앎
                     └────────┬─────────┘
                              │ 모든 메시지
                              ▼
                     ┌─────────────────┐
                     │  SpaceManager   │  ← NEW: 핵심 컴포넌트
                     │                 │
                     │ 1. Space 감지   │  메시지 → 어느 Space?
                     │ 2. 라우팅      │  적절한 Space로 전달
                     │ 3. 격리 관리   │  Memory/Workspace 스코핑
                     │ 4. 자동 생성   │  필요시 새 Space 생성
                     └────────┬────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────┐   ┌──────────┐
        │ Space A  │   │ Space B  │   │ Space C  │
        │ "oxios"  │   │ "일상"   │   │ "blog"   │
        │──────────│   │──────────│   │──────────│
        │ Memory ✓ │   │ Memory ✓ │   │ Memory ✓ │
        │ Workdir ✓│   │ Workdir  │   │ Workdir ✓│
        │ Paths:   │   │ Paths:   │   │ Paths:   │
        │ /oxios   │   │ (none)   │   │ /blog    │
        └─────┬────┘   └─────┬────┘   └─────┬────┘
              │              │              │
              └──────────────┼──────────────┘
                             ▼
                    ┌─────────────────┐
                    │  Orchestrator   │  기존 + space_id 확장
                    │  Supervisor     │
                    │  Agents (전역)  │  Space가 필요시 대여
                    └─────────────────┘
```

## 5. SpaceManager

### 5.1 감지 전략 (Moderate Aggressiveness)

파일시스템 경로는 자동 매핑, 주제 전환은 OS가 판단해서 자동 분리.
사용자는 나중에 대시보드에서 확인 및 수정 가능.

```
SpaceManager.detect_space(message, conversation_history)
  │
  ├── 1차: 파일시스템 경로 추출 (정규식, 빠름, LLM 불필요)
  │     - "/projects/oxios/src/main.rs" → oxios Space
  │     - "~/Documents/recipe.md" → recipe Space (또는 기존 일상 Space)
  │     - 경로가 기존 Space의 paths와 매칭? → 해당 Space 활성화
  │
  ├── 2차: 리소스 감지 (키워드 매칭, 빠름)
  │     - "supervisor.rs", "Cargo.toml" → oxios Space (이미 paths에 있음)
  │     - "oven", "recipe" → 일상/요리 Space?
  │
  └── 3차: 주제 전환 감지 (LLM, 필요시만)
        - 대화 컨텍스트와 현재 Space의 주제가 확실히 다른가?
        - 1차/2차에서 감지 안 된 경우에만 호출
        - 빈도 제한: N턴에 한 번, 또는 사용자 발화 패턴 변화 감지시
```

### 5.2 감지 최적화

LLM 호출은 비싸므로 최소화:

| 레이어 | 방식 | 비용 | 정확도 |
|--------|------|------|--------|
| 1차 | 경로 정규식 매칭 | 무료 | 높음 |
| 2차 | 키워드/태그 매칭 | 무료 | 중간 |
| 3차 | LLM 주제 분류 | 토큰 소모 | 높음 |

실제 사용에서 1차+2차로 80% 이상 커버될 것으로 예상.
3차는 "오늘 날씨 어때?" 같이 경로/키워드가 전혀 없는 메시지에만 필요.

### 5.3 전환 알림 전략

Space 전환은 **조용히** 일어난다. 하지만 사용자가 혼동하지 않도록:

- **첫 전환시**: 간단한 인라인 태그 `[oxios]` 또는 이모지 🔧
- **연속 대화에서는**: 알림 생략 (이미 해당 Space 안에 있음)
- **대시보드**: 항상 현재 Space 표시

```
User: oxios에 버그 있어
AI: [🔧 oxios] 어떤 버그인가요?

User: (설명)
AI: [🔧 oxios] supervisor.rs를 확인해볼게요...

User: 근데 오늘 저녁 뭐 먹지?
AI: [🏠 일상] 저녁 메뉴 추천해드릴게요! 오늘 날씨가...

User: 다시 oxios 버그로 돌아가서
AI: [🔧 oxios] 네, 아까 버그 이어서 보겠습니다.
```

## 6. Data Model

### 6.1 Rust Types

```rust
/// Unique identifier for a Space.
pub type SpaceId = uuid::Uuid;

/// How a Space was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceSource {
    /// Auto-created from a detected filesystem path.
    AutoResource,
    /// Auto-created from a detected topic shift.
    AutoTopic,
    /// Explicitly created by the user (rare).
    Manual,
}

/// A logical work partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    /// Unique identifier.
    pub id: SpaceId,
    /// Human-readable name (auto-generated or user-specified).
    pub name: String,
    /// How this Space was created.
    pub source: SpaceSource,
    /// Filesystem paths associated with this Space.
    pub paths: Vec<PathBuf>,
    /// Workspace directory for this Space.
    pub workspace_dir: PathBuf,
    /// Tags for keyword matching.
    pub tags: Vec<String>,
    /// Whether this Space is currently active.
    pub active: bool,
    /// When this Space was created.
    pub created_at: DateTime<Utc>,
    /// When this Space was last active.
    pub last_active_at: DateTime<Utc>,
    /// Conversation count in this Space.
    pub interaction_count: u64,
}
```

### 6.2 Storage Layout

```
~/.oxios/
├── config.toml
├── spaces/
│   ├── _index.json              # Space 메타데이터 인덱스
│   ├── {space_id}/
│   │   ├── space.json           # Space 정의
│   │   ├── memory/
│   │   │   ├── facts/
│   │   │   ├── episodes/
│   │   │   ├── conversations/
│   │   │   └── knowledge/
│   │   ├── state/
│   │   │   └── sessions/
│   │   └── workspace/           # Space 작업 디렉토리
│   └── _default/                # 기본 Space (fallback)
│       ├── space.json
│       ├── memory/
│       └── workspace/
├── global/
│   ├── personas/
│   ├── programs/
│   └── skills/
└── audit/
```

## 7. Integration Points

### 7.1 KernelEvent 확장

```rust
// event_bus.rs에 추가
pub enum KernelEvent {
    // ... 기존 이벤트들 ...

    /// A new Space has been created.
    SpaceCreated {
        space_id: SpaceId,
        name: String,
        source: String, // "auto_resource" | "auto_topic" | "manual"
    },
    /// Active Space has changed.
    SpaceActivated {
        space_id: SpaceId,
        name: String,
    },
    /// A Space has been archived or merged.
    SpaceArchived {
        space_id: SpaceId,
    },
}
```

### 7.2 Orchestrator 확장

`handle_message`가 `space_id`를 받도록 변경:

```rust
// Before:
orchestrator.handle_message(user_id, message, session_id).await

// After:
let space_id = space_manager.detect_or_create(message, history).await?;
let space = space_manager.get_space(&space_id).await?;
orchestrator.handle_message(user_id, message, session_id, &space).await
```

Orchestrator는 `space.memory_manager`를 사용해서 Space-scoped 메모리에 접근하고,
`space.workspace_dir`를 AgentRuntime에 전달해서 격리된 워크스페이스에서 작업한다.

### 7.3 AgentRuntime 확장

```rust
// AgentRuntimeConfig에 추가:
pub struct AgentRuntimeConfig {
    // ... 기존 필드 ...
    /// Space ID for scoped memory and workspace.
    pub space_id: Option<SpaceId>,
    /// Workspace directory override (from Space).
    pub workspace_dir: Option<PathBuf>,
}
```

이걸로 `WORKSPACE_MUTEX` 문제도 자연스럽게 해결될 가능성이 있다 —
각 Space가 자체 workspace_dir을 가지니까.

### 7.4 MemoryManager 확장

```rust
impl MemoryManager {
    /// Create a Space-scoped MemoryManager.
    pub fn for_space(space_dir: PathBuf) -> Self {
        let state_store = Arc::new(StateStore::new(space_dir).unwrap());
        Self::new(state_store)
    }
}
```

각 Space가 자체 `StateStore` 인스턴스를 가지면서 자연스럽게 메모리 격리.

## 8. Transparency Interface

### 8.1 Dashboard (Web UI / TUI)

사용자가 "내부를 보고 싶을 때"를 위한 대시보드:

```
┌─────────────────────────────────────────────┐
│  Oxios Dashboard                             │
│                                              │
│  Active Spaces:                              │
│  🔧 oxios    [active] 3 agents  /projects/oxios │
│  🏠 일상              0 agents               │
│  📝 blog     [active] 1 agent   /projects/blog │
│                                              │
│  Recent Activity (oxios):                    │
│  14:32 Agent#7 started  "Fix supervisor bug" │
│  14:31 Memory stored  "supervisor.rs pattern"│
│  14:28 Space activated  "oxios"              │
│                                              │
│  Memory (oxios): 47 entries                  │
│  - facts: 12  episodes: 8  convos: 27        │
│                                              │
│  [Switch Space]  [Archive]  [Merge]          │
└─────────────────────────────────────────────┘
```

### 8.2 대화 기반 조회

사용자가 대화로도 내부 상태를 조회할 수 있다:

```
User: 지금 어떤 스페이스 있어?
AI: [🔧 oxios] 현재 3개 스페이스가 있어요:
    - oxios (활성, 47개 메모리, 3 에이전트 작업 중)
    - 일상 (유휴, 12개 메모리)
    - blog (유휴, 8개 메모리)

User: 일상 스페이스 메모리 뭐 있어?
AI: [🏠 일상] 저장된 메모리 12개 중 주요 것들:
    - 선호하는 저녁 메뉴: 파스타, 샐러드
    - 이번 주 장보기 리스트
    - ...

User: blog 스페이스를 일상이랑 합쳐줘
AI: [🏠 일상] blog 스페이스를 일상에 병합했습니다.
    메모리 8개가 이전되었어요.
```

## 9. Detection Algorithm (Pseudocode)

```rust
impl SpaceManager {
    async fn detect_or_create(
        &self,
        message: &str,
        history: &[Message],
    ) -> Result<SpaceId> {
        // Layer 1: Filesystem path detection (fast, free)
        if let Some(path) = extract_filesystem_path(message) {
            if let Some(space) = self.find_by_path(&path) {
                self.activate(&space.id).await?;
                return Ok(space.id);
            }
            // New path detected → create new Space
            let space = self.create_from_path(&path).await?;
            return Ok(space.id);
        }

        // Layer 2: Keyword/tag matching (fast, free)
        if let Some(space) = self.match_keywords(message) {
            self.activate(&space.id).await?;
            return Ok(space.id);
        }

        // Layer 3: Topic shift detection (LLM, only when needed)
        if self.should_check_topic_shift(history) {
            let topic = self.classify_topic(message).await?;
            if let Some(space) = self.find_by_topic(&topic) {
                self.activate(&space.id).await?;
                return Ok(space.id);
            }
            let space = self.create_from_topic(&topic).await?;
            return Ok(space.id);
        }

        // Default: stay in current Space
        Ok(self.current_space_id())
    }

    /// Should we invoke LLM for topic classification?
    /// Heuristic: check every N turns, or when message pattern changes.
    fn should_check_topic_shift(&self, history: &[Message]) -> bool {
        let turns_since_last_check = self.turns_since_topic_check();
        turns_since_last_check >= 3 // Every 3 turns
            || self.pattern_changed(history) // Detectable shift in language
    }
}
```

## 10. Implementation Plan

### Phase 1: Foundation (SpaceManager + 데이터 모델)
- `Space`, `SpaceId`, `SpaceSource` 타입 정의
- `SpaceManager` 기본 구조 (create, list, activate, detect)
- 파일시스템 저장소 레이아웃 (`~/.oxios/spaces/`)
- KernelEvent에 Space 이벤트 추가

### Phase 2: Detection (1차 + 2차)
- 파일시스템 경로 추출 (정규식)
- 키워드/태그 매칭
- 기존 Space와의 매칭 로직
- 기본 Space(_default) 항상 존재 보장

### Phase 3: Memory Isolation
- Space-scoped MemoryManager
- Space-scoped StateStore 경로
- Orchestrator에 space_id 전달
- AgentRuntime에 workspace_dir 전달

### Phase 4: Topic Detection (3차, LLM)
- LLM 기반 주제 분류
- 주제 전환 감지 휴리스틱
- 자동 Space 생성/매핑

### Phase 5: Transparency
- Space 상태 조회 API (kernel_handle)
- 대시보드용 이벤트 스트림 (EventBus 필터링)
- 대화 기반 Space 관리 ("어떤 스페이스 있어?")

## 11. Resolved Decisions

### D1: Space 병합 — 자동 병합 + 감사 로그

OS가 두 Space의 유사도를 판단해서 자동 병합한다.
모든 병합 이력은 감사 로그에 기록되며, 대시보드에서 확인 가능.
GitLayer를 활용해 병합 전 상태를 커밋해두면 되돌리기도 가능.

```
Space A "oxios-dev" + Space B "oxios-bugfix"
→ OS: 두 Space의 paths, tags, 대화 주제가 유사함 (유사도 0.87)
→ 자동 병합 → "oxios" Space로 통합
→ 감사 로그: "merged oxios-bugfix into oxios at 2024-..."
→ 대시보드에서 되돌리기 가능
```

### D2: Space 보존 — 30일 자동 archive + 즉시 복구

- `last_active_at` 기준 30일 비활성시 자동 archive
- Archive된 Space의 **Memory와 Workspace는 디스크에 보존**
- 에이전트는 항상 동적 생성/소멸이므로 archive와 무관
- 사용자가 archive된 Space의 주제나 경로를 언급하면 **즉시 복구**
- 대화로 복구: "oxios 작업 다시 하자" → archive에서 즉시 활성화

```
~/.oxios/spaces/
├── {space_id}/          # 활성 Space
└── _archived/
    └── {space_id}/      # 보존됨, 언제든 복구 가능
```

### D3: Cross-Space 지식 흐름 — OS가 지속적으로 자동 관리

Space 간 지식은 **투명한 반투명 막**으로 연결되어 있다.
기본적으로 각 Space는 독립된 메모리를 가지지만,
OS가 지식의 흐름을 지속적으로 관리한다.

#### D3-1: 참조 (Reading)

OS가 현재 작업에 다른 Space의 메모리가 필요하다고 판단하면,
다른 Space의 메모리를 검색해서 가져올 수 있다.

- 사용자가 명시적으로 요청할 필요 없음
- OS가 컨텍스트를 분석해서 자동으로 판단
- 참조 시 해당 Space 메모리에 "Space X에서 참조됨" 로그 남김

#### D3-2: 이전 (Transfer)

한 Space에서 학습한 지식을 다른 Space로 자동 주입.

```
새 프로젝트 Space 생성됨
→ OS: 기존 프로젝트 Space들에서 관련 패턴/지식 검색
→ 관련 지식을 새 Space의 memory에 주입
→ 로그: "5개 지식을 oxios→new-project로 이전"
```

#### D3-3: 통합 (Synthesis)

OS가 주기적으로 여러 Space의 지식을 분석해서
새로운 통찰을 자동 생성.

```
OS (주기적 분석):
→ oxios, blog, side-project 세 Space에서
  "에러 핸들링 패턴" 관련 메모리 발견
→ 공통점 분석 → "사용자의 에러 핸들링 철학" 통찰 생성
→ 현재 활성 Space에 통합 인사이트로 저장
→ 감사 로그: "synthesized insight from 3 spaces"
```

### D4: Default Space — 이름 없음, 주제 형성시 자동 명명

기본 Space는 **이름이 없다**.
모든 메시지의 fallback이며, 사용자가 Space를 명시하지 않고
경로/주제도 감지되지 않으면 여기로 떨어진다.

- 대화가 진행되면서 주제가 형성되면 OS가 자동으로 이름을 붙임
- 사용자가 "이거 일상 스페이스야"라고 하면 그때 이름이 명확해짐
- 이름 없는 상태에서도 Memory, Workspace는 정상 작동

```
~/.oxios/spaces/
└── _default/            # 이름 없는 기본 Space
    ├── space.json       # name: "" 또는 null
    ├── memory/
    └── workspace/
```

### D5: Space 개수 — 무제한 + archive로 자연 관리

Space 개수에 하드 리밋은 없다.
대신 D2의 archive 정책(30일 비활성)으로 활성 Space 수가 자연스럽게 관리된다.

- 활성 Space: 사용자가 최근 대화에서 언급한 것들
- Archive Space: 30일 이상 비활성, 디스크에 보존
- 복구: 언제든 대화로 즉시 복구
- 성능: Space 인덱스는 메모리에 올리지만, archive는 lazy load

## 12. Knowledge Flow Architecture

D3의 지식 흐름을 구현하기 위한 아키텍처.

### 12.1 KnowledgeBridge

SpaceManager 내부에 KnowledgeBridge 컴포넌트가 지식 흐름을 관리.

```rust
/// Manages knowledge flow between Spaces.
pub struct KnowledgeBridge {
    space_manager: Arc<SpaceManager>,
    /// Cross-reference log for audit trail.
    xref_log: RwLock<Vec<CrossRefEntry>>,
}

/// Record of a cross-Space knowledge access.
pub struct CrossRefEntry {
    /// Source Space.
    from: SpaceId,
    /// Target Space.
    to: SpaceId,
    /// Memory entries accessed.
    entries: Vec<String>,
    /// Type of flow.
    flow: KnowledgeFlow,
    /// When this happened.
    timestamp: DateTime<Utc>,
}

/// Type of knowledge flow.
pub enum KnowledgeFlow {
    /// Read from another Space (reference).
    Reference,
    /// Copied from one Space to another (transfer).
    Transfer,
    /// Synthesized from multiple Spaces (synthesis).
    Synthesis { sources: Vec<SpaceId> },
}
```

### 12.2 자동 발동 시나리오

| 시나리오 | 트리거 | Flow 타입 | 빈도 |
|----------|--------|-----------|------|
| 새 Space 생성 | Space 생성시 | Transfer | 생성시 1회 |
| 작업 중 관련 지식 발견 | 에이전트 작업 중 | Reference | 필요시 |
| 주기적 통합 분석 | 백그라운드 (cron) | Synthesis | 일 1회 |
| 사용자 명시 요청 | "다른 스페이스에서 가져와" | Transfer/Reference | 요청시 |

### 12.3 보안/프라이버시 고려

지식이 자동으로 흐르다 보니 프라이버시 우려가 있을 수 있다.

- **민감 정보 태깅**: 사용자가 특정 메모리를 "이 Space에서만"으로 태그하면 자동 이전 불가
- **Space visibility**: Space를 private로 설정하면 다른 Space에서 참조 불가
- **감사 로그**: 모든 지식 흐름은 기록, 대시보드에서 확인 가능
- **되돌리기**: 이전/통합된 지식은 삭제 가능 (GitLayer 활용)
