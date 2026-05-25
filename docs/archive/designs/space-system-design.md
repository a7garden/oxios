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
  ├── 바인딩된 경로 (실제 프로젝트/리소스 경로)
  ├── 독립된 Memory (해당 Space 관련 메모리만)
  ├── 독립된 Workspace (Space 전용 스크래치 공간)
  ├── 활성화 이력 (생성 시간, 마지막 사용 시간)
  └── 메타데이터 (태그, 생성 출처)
```

### 3.1 Space Source (생성 방식)

| 출처 | 트리거 | 예시 |
|------|--------|------|
| `AutoResource` | 파일시스템 경로 감지 | "/projects/oxios" 경로 언급 → "oxios" Space |
| `AutoTopic` | 주제 전환 감지 | 개발 → 일상 대화 전환 → "일상" Space |
| `Manual` | 사용자 명시 | "새 스페이스 만들어" (드물겠지만 지원) |

### 3.2 Space Isolation — Scoped Namespace + Controlled Bridge

각 Space는 **scoped namespace**다. 기본적으로 메모리와 작업 공간이 격리되어 있지만,
OS가 **controlled bridge**를 통해 필요시 다른 Space의 지식에 접근할 수 있다.
"완전 격리"가 아니라 "기본은 닫힌, OS가 열쇠를 가진" 모델.

| 리소스 | 격리 모델 | 설명 |
|--------|----------|------|
| Memory | Scoped namespace | 각 Space는 자체 메모리. OS가 KnowledgeBridge로 다른 Space 참조 가능 |
| Workspace | 분리된 scratch | 각 Space는 자체 스크래치 디렉토리. 바인딩된 실제 경로와 분리 |
| Agent | 전역 | 에이전트는 전역 리소스, 필요시 생성/소멸 |
| Persona | 전역 | 페르소나는 전역, Space가 선택해서 사용 |
| Program | 전역 | 프로그램은 전역, Space의 컨텍스트에 따라 활성화 여부 결정 |
| EventBus | 태깅 | 전역이지만 모든 이벤트에 `space_id` 태그 부착, 필터링 가능 |

### 3.3 Paths vs Workspace — 실제 경로와 스크래치 공간의 분리

개발 Space의 작업 대상은 `~/.oxios/spaces/` 안이 아니라 **실제 프로젝트 경로**다.
두 개념을 명확히 분리한다:

| 필드 | 역할 | 예시 |
|------|------|------|
| `paths` | 실제 작업 대상 경로. AgentRuntime이 CWD로 설정 | `/Volumes/MERCURY/PROJECTS/oxios` |
| `workspace_dir` | Space 전용 스크래치 공간. 임시 파일, 로그, 산출물 | `~/.oxios/spaces/{id}/workspace/` |

```rust
Space {
    paths: vec!["/Volumes/MERCURY/PROJECTS/oxios"],
    workspace_dir: "~/.oxios/spaces/{id}/workspace/",
}
// AgentRuntime:
//   CWD = paths[0] (실제 코드를 편집해야 하니까)
//   임시 파일, 로그 = workspace_dir (Space별로 격리)
```

경로가 없는 Space(일상, 요리 등)는 `paths`가 비어 있고,
AgentRuntime은 `workspace_dir`만 사용한다.

## 4. Architecture

### 4.1 SpaceManager는 Orchestrator 내부에 위치

**핵심 결정**: SpaceManager를 Gateway와 Orchestrator 사이의 독립 계층이 아니라,
**Orchestrator 내부 컴포넌트**로 둔다.

이유:
- Gateway는 "얇은 라우터"로 유지해야 한다. 책임이 무거워지면 안 된다.
- Orchestrator가 이미 sessions, history, event_bus를 다 들고 있다.
- Space 감지에는 대화 컨텍스트가 필요한데, 이건 Orchestrator만이 가지고 있다.
- Gateway는 여전히 stateless.

```
Gateway (stateless, thin router)
  │
  │  IncomingMessage
  ▼
Orchestrator
  ├── SpaceManager          ← 내부 컴포넌트
  │   ├── ConversationBuffer  (최근 N턴 대화 유지)
  │   ├── detect_or_create()  (3-레이어 감지)
  │   └── KnowledgeBridge     (Cross-Space 지식 흐름)
  │
  ├── Ouroboros Protocol
  ├── Supervisor / AgentRuntime
  └── EventBus
```

### 4.2 전체 흐름

```
User: "oxios supervisor.rs에 버그 있어"
  │
  ▼
Gateway.route(msg)   ← 변경 없음, stateless
  │
  ▼
Orchestrator.handle_message(user_id, msg, session_id)
  │
  ├── 1. ConversationBuffer.push(msg)        ← 대화 기록 누적
  │
  ├── 2. SpaceManager.detect_or_create(msg, buffer)
  │      → 1차: 경로 감지 → "/projects/oxios" → 기존 "oxios" Space 매칭
  │      → Space 활성화, space_id 반환
  │
  ├── 3. space.memory_manager.recall()       ← oxios Space 전용 메모리
  │
  ├── 4. Ouroboros interview → seed → execute
  │      → AgentRuntime에 space.workspace_dir, space.paths[0] 전달
  │
  └── 5. 결과를 ConversationBuffer에 기록
         → 응답에 [🔧 oxios] 태그 부착
```

```
User: "근데 오늘 저녁 뭐 먹지?"
  │
  ▼
Orchestrator.handle_message(...)
  │
  ├── 1. ConversationBuffer.push(msg)
  │
  ├── 2. SpaceManager.detect_or_create(msg, buffer)
  │      → 1차: 경로 없음
  │      → 2차: 키워드 매칭 없음
  │      → 3차: LLM 주제 분류 → "일상/음식"
  │      → 기존 "일상" Space 있음 → 활성화
  │      → 없으면 → 새 "일상" Space 자동 생성
  │
  └── 3. 일상 Space에서 처리 → [🏠 일상] 태그
```

## 5. SpaceManager

### 5.1 감지 전략 (Moderate Aggressiveness)

파일시스템 경로는 자동 매핑, 주제 전환은 OS가 판단해서 자동 분리.
사용자는 나중에 대시보드에서 확인 및 수정 가능.

```
SpaceManager.detect_space(message, conversation_buffer)
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

### 5.2 ConversationBuffer — 대화 히스토리 유지

3차 감지(LLM 주제 분류)를 하려면 최근 대화 컨텍스트가 필요하다.
Orchestrator 내부에 메모리 상의 순환 버퍼를 둔다.

```rust
/// In-memory circular buffer of recent conversation turns.
/// Used by SpaceManager for topic shift detection.
pub struct ConversationBuffer {
    /// Recent turns (bounded, oldest evicted first).
    turns: VecDeque<ConversationTurn>,
    /// Maximum number of turns to retain.
    max_turns: usize,
}

pub struct ConversationTurn {
    /// User message.
    pub user: String,
    /// Agent response (truncated to first 200 chars for efficiency).
    pub agent: String,
    /// Active Space at the time.
    pub space_id: SpaceId,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}
```

- 기본 크기: 최근 50턴 유지
- Orchestrator가 매 handle_message마다 push
- SpaceManager가 3차 감지시 참조
- 디스크에 저장하지 않음 (재시작시 빈 버퍼에서 시작, Space 감지는 1차/2차로 충분)

### 5.3 감지 최적화

LLM 호출은 비싸므로 최소화:

| 레이어 | 방식 | 비용 | 정확도 |
|--------|------|------|--------|
| 1차 | 경로 정규식 매칭 | 무료 | 높음 |
| 2차 | 키워드/태그 매칭 | 무료 | 중간 |
| 3차 | LLM 주제 분류 | 토큰 소모 | 높음 |

실제 사용에서 1차+2차로 80% 이상 커버될 것으로 예상.
3차는 "오늘 날씨 어때?" 같이 경로/키워드가 전혀 없는 메시지에만 필요.

### 5.4 전환 알림 전략

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
    /// Human-readable name.
    /// - AutoResource: derived from directory name (e.g. "oxios")
    /// - AutoTopic: estimated from LLM classification (e.g. "일상")
    /// - Default Space: empty string (빈 문자열, 주제 형성 후 이름 부여)
    pub name: String,
    /// How this Space was created.
    pub source: SpaceSource,
    /// Actual filesystem paths bound to this Space.
    /// AgentRuntime sets CWD to paths[0] when executing.
    /// Empty for non-filesystem Spaces (일상, 요리 등).
    pub paths: Vec<PathBuf>,
    /// Scratch workspace directory for this Space.
    /// Temporary files, logs, build artifacts go here.
    pub workspace_dir: PathBuf,
    /// Tags for keyword matching (Layer 2 detection).
    pub tags: Vec<String>,
    /// Whether this Space is currently active.
    pub active: bool,
    /// When this Space was created.
    pub created_at: DateTime<Utc>,
    /// When this Space was last active.
    pub last_active_at: DateTime<Utc>,
    /// Number of interactions in this Space.
    pub interaction_count: u64,
    /// Whether this Space allows cross-Space knowledge access.
    /// Default: true. Set to false for private Spaces.
    #[serde(default = "default_true")]
    pub knowledge_visible: bool,
}

fn default_true() -> bool { true }
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
│   │   └── workspace/           # Space 전용 스크래치 공간
│   │       ├── tmp/             # 임시 파일
│   │       ├── logs/            # 에이전트 작업 로그
│   │       └── artifacts/       # 빌드 산출물 등
│   ├── _default/                # 기본 Space (fallback)
│   │   ├── space.json
│   │   ├── memory/
│   │   └── workspace/
│   └── _archived/               # 30일 비활성 Space 보관
│       └── {space_id}/
├── global/
│   ├── personas/
│   ├── programs/
│   └── skills/
└── audit/
    └── knowledge_flow.jsonl     # Cross-Space 지식 접근 로그
```

## 7. Integration Points

### 7.1 Gateway — 변경 없음

Gateway는 stateless를 유지한다. Space 감지 로직이 Gateway에 추가되지 않는다.

```rust
// Gateway.route() — 기존 코드 그대로
pub async fn route(&self, msg: IncomingMessage) -> Result<()> {
    let session_id = msg.metadata.get("session_id").cloned();
    let result = self.orchestrator
        .handle_message(&msg.user_id, &msg.content, session_id.as_deref())
        .await;
    // ...
}
```

### 7.2 Orchestrator 확장

Orchestrator가 SpaceManager를 내부에 품는다.

```rust
pub struct Orchestrator {
    // ... 기존 필드 ...
    /// Space manager for context partitioning.
    space_manager: SpaceManager,
    /// Recent conversation buffer for topic detection.
    conversation_buffer: ConversationBuffer,
    /// Knowledge bridge for cross-Space knowledge flow.
    knowledge_bridge: KnowledgeBridge,
}

impl Orchestrator {
    pub async fn handle_message(
        &self,
        user_id: &str,
        user_message: &str,
        session_id: Option<&str>,
    ) -> Result<OrchestrationResult> {
        // 1. Record in conversation buffer.
        self.conversation_buffer.push_user(user_message);

        // 2. Detect or create Space.
        let space_id = self.space_manager
            .detect_or_create(user_message, &self.conversation_buffer)
            .await?;

        // 3. Get Space-scoped resources.
        let space = self.space_manager.get_space(&space_id).await?;

        // 4. Recall Space-scoped memories.
        if let Some(ref mm) = space.memory_manager {
            let memories = mm.recall(user_message).await?;
            // inject into system prompt...
        }

        // 5. Run Ouroboros with Space context.
        //    AgentRuntime receives space.paths, space.workspace_dir
        let result = self.run_ouroboros(user_id, user_message, session_id, &space).await?;

        // 6. Record response in buffer.
        self.conversation_buffer.push_agent(&result.response, &space_id);

        // 7. Tag response with Space indicator.
        let tagged = self.tag_response(&result, &space);

        Ok(tagged)
    }
}
```

### 7.3 AgentRuntime 확장

```rust
// AgentRuntimeConfig에 추가:
pub struct AgentRuntimeConfig {
    // ... 기존 필드 ...
    /// Space ID for scoped memory and workspace.
    pub space_id: Option<SpaceId>,
    /// Bound project paths. AgentRuntime sets CWD to paths[0].
    pub project_paths: Vec<PathBuf>,
    /// Scratch workspace directory for temp files.
    pub workspace_dir: Option<PathBuf>,
}
```

AgentRuntime 실행시:
```rust
// run_agent_loop() 내부
if let Some(cwd) = ctx.project_paths.first() {
    std::env::set_current_dir(cwd)?;
    // CWD = 실제 프로젝트 경로 (코드 편집, git 작업 등)
} else if let Some(ref ws) = ctx.workspace_dir {
    std::env::set_current_dir(ws)?;
    // CWD = 스크래치 공간 (경로가 없는 Space)
}
```

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

각 Space가 자체 `StateStore` 인스턴스를 가지면서 자연스럽게 메모리 namespace 분리.

### 7.5 KernelEvent 확장

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
    /// A Space has been archived.
    SpaceArchived {
        space_id: SpaceId,
        name: String,
    },
    /// Cross-Space knowledge accessed.
    KnowledgeCrossReferenced {
        from_space: SpaceId,
        to_space: SpaceId,
        entries: usize,
        flow: String, // "reference" | "transfer" | "synthesis"
    },
    /// Spaces have been merged.
    SpacesMerged {
        survivor: SpaceId,
        absorbed: SpaceId,
        entries_migrated: usize,
    },
}
```

## 8. Default Space의 명명 전략

기본 Space는 빈 문자열 이름으로 시작하지만, **주제가 형성되면 자동으로 새 Space로 분리**된다.

```
동작 흐름:

1. 사용자가 첫 메시지: "오늘 날씨 어때?"
   → 경로 감지 없음, 키워드 매칭 없음
   → 기본 Space에 머뭄 (아직 주제 불명확)
   → 기본 Space name = ""

2. 사용자: "점심으로 파스타 만들고 싶은데 레시피 알려줘"
   → 3차 LLM 감지: "요리/음식" 주제 감지
   → 기본 Space에 계속 쌓이는 대신 새 Space 자동 생성
   → 새 Space name = "요리" (LLM이 추정)
   → 기본 Space의 최근 대화를 새 Space로 이관

3. 이후 "요리" 관련 대화는 자동으로 "요리" Space로 라우팅
   → 기본 Space는 다시 빈 상태로 대기
```

즉, **기본 Space는 대기실**이다. 주제가 명확해지면 대기실에서 적절한 Space로 분리된다.
사용자는 이 과정을 전혀 의식하지 않는다.

## 9. Knowledge Flow — Cross-Space 지식 흐름

### 9.1 모델: Scoped Namespace + Controlled Bridge

Space의 메모리는 기본적으로 자체 namespace에 격리되어 있다.
하지만 OS가 **KnowledgeBridge**를 통해 필요시 다른 Space의 지식에 접근한다.

"완전 격리"가 아니라 **"기본은 닫힌, OS가 열쇠를 가진"** 모델.

### 9.2 세 가지 Flow 타입

| 타입 | 설명 | 자동/수동 | 구현 시점 |
|------|------|----------|-----------|
| **Reference** | 다른 Space 메모리를 읽어와서 현재 작업에 활용 | OS 자동 | Phase 3 |
| **Transfer** | 한 Space의 지식을 다른 Space로 복사 | 새 Space 생성시 OS 자동 | Phase 3 |
| **Synthesis** | 여러 Space의 지식을 종합해서 새 통찰 생성 | 백그라운드 (cron) | Phase 6 (후속) |

**주의**: Synthesis는 LLM 비용이 높으므로 Phase 6으로 분리한다.
Phase 1-5에서는 Reference와 Transfer만 구현하고, 인프라만 준비한다.

### 9.3 KnowledgeBridge

```rust
/// Manages knowledge flow between Spaces.
pub struct KnowledgeBridge {
    space_manager: Arc<SpaceManager>,
    /// Cross-reference log (appended to audit/knowledge_flow.jsonl).
    xref_log: Arc<Mutex<Vec<CrossRefEntry>>>,
}

pub struct CrossRefEntry {
    pub from: SpaceId,
    pub to: SpaceId,
    pub entry_ids: Vec<String>,
    pub flow: KnowledgeFlow,
    pub timestamp: DateTime<Utc>,
}

pub enum KnowledgeFlow {
    /// Read-only access to another Space's memory.
    Reference,
    /// Copy entries from one Space to another.
    Transfer,
    /// Synthesize insights from multiple Spaces. (Phase 6)
    Synthesis { sources: Vec<SpaceId> },
}
```

### 9.4 자동 발동 시나리오

| 시나리오 | 트리거 | Flow 타입 | 구현 Phase |
|----------|--------|-----------|-----------|
| 작업 중 관련 지식 필요 | 에이전트 실행 중 | Reference | Phase 3 |
| 새 Space 생성 | Space 생성시 | Transfer | Phase 3 |
| 주기적 통합 분석 | 백그라운드 (cron) | Synthesis | Phase 6 |
| 사용자 명시 요청 | "다른 스페이스에서 가져와" | Transfer/Reference | Phase 5 |

### 9.5 프라이버시 제어

- `knowledge_visible: false`인 Space는 KnowledgeBridge가 접근하지 않음
- 개별 메모리에도 `private: true` 태그 가능 → Cross-Space 이전에서 제외
- 모든 접근은 `audit/knowledge_flow.jsonl`에 기록
- 대시보드에서 지식 흐름 이력 조회 가능

## 10. Space 병합

### 10.1 보수적 자동 병합 + 확인

자동 병합은 **매우 보수적**으로 동작한다.

**자동 병합 조건** (모두 충족해야 함):
1. 두 Space가 **동일한 paths를 공유** (가장 강한 신호)
2. 태그 유사도 0.9 이상
3. 어느 한쪽의 interaction_count가 5 미만 (아직 활성화되지 않은 Space)

**자동 병합 외의 경우** — OS가 1줄 제안:
```
AI: [🔧 oxios] 참, oxios-dev 스페이스와 oxios-bugfix 스페이스가
    비슷한 것 같은데 합칠까요?
User: 응
AI: [🔧 oxios] 합쳤습니다. 23개 메모리가 통합되었어요.
```

이건 "대화가 유일한 인터페이스" 원칙에 부합한다 — 확인이 필요한 경우에도
별도 UI가 아니라 대화로 자연스럽게 이루어진다.

### 10.2 병합 이력과 되돌리기

- 병합 전 GitLayer에 커밋 → 언제든 되돌리기 가능
- 감사 로그에 `SpacesMerged` 이벤트 기록
- 대시보드에서 병합 이력 조회

## 11. Space 보존 정책

### 30일 자동 archive + 즉시 복구

- `last_active_at` 기준 30일 비활성시 자동 archive
- Archive된 Space의 **Memory와 Workspace는 디스크에 보존**
- 에이전트는 항상 동적 생성/소멸이므로 archive와 무관
- 사용자가 archive된 Space의 주제나 경로를 언급하면 **즉시 복구**
- 대화로 복구: "oxios 작업 다시 하자" → archive에서 즉시 활성화

### Archive 청소

- Archive Space도 영구 보존 (기본)
- 사용자가 "안 쓰는 스페이스 삭제해줘"라고 하면 그때 삭제
- 삭제 전에 한 번 더 확인 (대화로)

## 12. Transparency Interface

### 12.1 Dashboard (Web UI / TUI)

사용자가 "내부를 보고 싶을 때"를 위한 대시보드:

```
┌─────────────────────────────────────────────────┐
│  Oxios Dashboard                                 │
│                                                  │
│  Spaces:                                         │
│  🔧 oxios    [active]  /projects/oxios  47 mem   │
│  🏠 일상               (no paths)       12 mem   │
│  📦 blog     [archived] /projects/blog   8 mem   │
│                                                  │
│  Recent Activity (oxios):                        │
│  14:32 Agent#7 started  "Fix supervisor bug"     │
│  14:31 Memory stored  "supervisor.rs pattern"    │
│  14:28 Space activated  "oxios"                  │
│                                                  │
│  Knowledge Flow:                                 │
│  14:15 Reference: 일상→oxios (2 entries)         │
│  13:40 Transfer: oxios→blog (3 entries)          │
│                                                  │
│  [Switch]  [Archive]  [Merge]  [Settings]        │
└─────────────────────────────────────────────────┘
```

### 12.2 대화 기반 조회

사용자가 대화로도 내부 상태를 조회할 수 있다:

```
User: 지금 어떤 스페이스 있어?
AI: [🔧 oxios] 현재 3개 스페이스가 있어요:
    - oxios (활성, 47개 메모리, 2 에이전트 작업 중)
    - 일상 (유휴, 12개 메모리)
    - blog (보관됨, 8개 메모리)

User: 일상 스페이스 메모리 뭐 있어?
AI: [🏠 일상] 저장된 메모리 12개 중 주요 것들:
    - 선호하는 저녁 메뉴: 파스타, 샐러드
    - 이번 주 장보기 리스트
    - ...

User: blog 스페이스 복구해줘
AI: [📝 blog] 보관된 blog 스페이스를 복구했어요.
    8개 메모리가 그대로 있어요.

User: blog 스페이스를 일상이랑 합쳐줘
AI: [🏠 일상] blog 스페이스를 일상에 병합했습니다.
    메모리 8개가 이전되었어요.
```

## 13. Detection Algorithm (Pseudocode)

```rust
impl SpaceManager {
    pub async fn detect_or_create(
        &self,
        message: &str,
        buffer: &ConversationBuffer,
    ) -> Result<SpaceId> {
        // Layer 1: Filesystem path detection (fast, free)
        if let Some(path) = extract_filesystem_path(message) {
            if let Some(space) = self.find_by_path(&path) {
                self.activate(&space.id).await?;
                return Ok(space.id);
            }
            // New path detected → create new Space
            let name = path_name(&path); // e.g. "oxios" from "/projects/oxios"
            let space = self.create_from_path(&name, &path).await?;
            return Ok(space.id);
        }

        // Layer 2: Keyword/tag matching (fast, free)
        if let Some(space) = self.match_keywords(message) {
            self.activate(&space.id).await?;
            return Ok(space.id);
        }

        // Layer 3: Topic shift detection (LLM, only when needed)
        if self.should_check_topic(buffer) {
            let topic = self.classify_topic(message, buffer).await?;

            // Check if current default Space should be promoted
            if self.is_in_default_space() {
                if topic.is_clear() {
                    // Promote: split default into new named Space
                    let space = self.promote_from_default(&topic).await?;
                    return Ok(space.id);
                }
                // Topic unclear, stay in default
                return Ok(self.default_space_id());
            }

            // Non-default: check for topic shift
            if self.topic_shifted(&topic, buffer) {
                if let Some(space) = self.find_by_topic(&topic) {
                    self.activate(&space.id).await?;
                    return Ok(space.id);
                }
                let space = self.create_from_topic(&topic).await?;
                return Ok(space.id);
            }
        }

        // Default: stay in current Space
        Ok(self.current_space_id())
    }

    /// Should we invoke LLM for topic classification?
    fn should_check_topic(&self, buffer: &ConversationBuffer) -> bool {
        let turns_since_last = self.turns_since_topic_check();
        turns_since_last >= 3
            || buffer.pattern_changed()
    }
}
```

## 14. Implementation Plan

### Phase 1: Foundation (SpaceManager + 데이터 모델)
- `Space`, `SpaceId`, `SpaceSource` 타입 정의
- `SpaceManager` 기본 구조 (create, list, activate, get)
- `ConversationBuffer` 구현
- 파일시스템 저장소 레이아웃 (`~/.oxios/spaces/`)
- 기본 Space(_default) 항상 존재 보장
- KernelEvent에 Space 이벤트 추가

### Phase 2: Detection (1차 + 2차)
- 파일시스템 경로 추출 (정규식)
- 키워드/태그 매칭
- 기존 Space와의 매칭 로직
- Default Space → named Space 자동 승격 로직

### Phase 3: Memory Isolation + Knowledge Reference/Transfer
- Space-scoped MemoryManager
- Space-scoped StateStore 경로
- Orchestrator에 space_id 전달
- AgentRuntime에 project_paths, workspace_dir 전달
- KnowledgeBridge: Reference (다른 Space 메모리 읽기)
- KnowledgeBridge: Transfer (새 Space에 기존 지식 주입)

### Phase 4: Topic Detection (3차, LLM)
- LLM 기반 주제 분류
- 주제 전환 감지 휴리스틱
- 자동 Space 생성/매핑

### Phase 5: Transparency + 병합/보존
- Space 상태 조회 API (kernel_handle)
- 대시보드용 이벤트 스트림 (EventBus 필터링)
- 대화 기반 Space 관리 ("어떤 스페이스 있어?")
- Space 병합 (자동 + 확인)
- 30일 자동 archive + 즉시 복구

### Phase 6: Knowledge Synthesis (후속 설계)
- 여러 Space 지식의 주기적 통합 분석
- 비용/품질 트레이드오프 분석 후 별도 설계
- 인프라는 Phase 3에서 준비 (KnowledgeFlow::Synthesis enum variant 등)
