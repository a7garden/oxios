# Loop 5 — 에이전트 메모리 시스템 설계

> Oxios의 메모리 시스템은 현재 "사람이 채우는 정적 스토어" 수준.
> 에이전트가 자율적으로 읽고 쓰고 검색하는 **활성 메모리 시스템**을 설계한다.

---

## 1. 현황 분석

### 무엇이 있는가

| 컴포넌트 | 상태 | 역할 |
|----------|------|------|
| `StateStore` | ✅ | 파일 기반 영속 스토어 (category/name → Markdown/JSON) |
| `Session` | ✅ | 대화 히스토리 저장 (user_messages + agent_responses) |
| oxi-ai `CompactionManager` | ✅ | LLM 기반 컨텍스트 압축 (summary 생성) |
| oxi-agent `CompactionEvent` | ✅ | 압축 완료 이벤트 (summary, kept_messages, compacted_count) |
| `memory/` API | ⚠️ | GET만 있음. POST/PUT/DELETE 없음 |
| `memory/knowledge/` API | ⚠️ | GET만 있음 |

### 무엇이 없는가

| 항목 | 설명 |
|------|------|
| **에이전트 메모리 도구** | 에이전트가 메모리를 읽고 쓸 수 있는 AgentTool이 없음 |
| **compaction → 메모리 저장** | LLM이 생성한 compaction summary가 그냥 버려짐 |
| **세션 간 기억** | 새 세션이 시작되면 이전 대화의 지식이 전달되지 않음 |
| **자동 요약** | 대화 종료 후 자동 요약이 생성되지 않음 |
| **의미 검색** | 파일명 매칭만. 키워드/의미 검색 없음 |
| **메모리 블렌딩** | 새 세션의 시스템 프롬프트에 관련 메모리가 주입되지 않음 |

### 핵심 인사이트

**oxi-ai의 CompactionManager가 이미 요약을 생성한다.** 이 요약이 메모리로 저장되면 80%의 문제가 해결된다.

```
현재:  CompactionManager → summary 생성 → 이벤트 발생 → 아무도 안 씀 → 사라짐
목표:  CompactionManager → summary 생성 → 이벤트 발생 → MemoryManager가 저장 → 다음 세션에서 검색
```

---

## 2. 설계 원칙

1. **oxi-ai 재사용** — CompactionManager의 요약을 캡처하여 메모리로 저장. 새 요약 엔진을 만들지 않음.
2. **AgentTool 인터페이스** — `memory_read`, `memory_write`, `memory_search` 세 도구를 oxi-agent의 AgentTool trait으로 구현.
3. **Unix 철학** — MemoryManager는 저장/검색만. 요약은 CompactionManager. 블렌딩은 AgentRuntime.
4. **점진적 복잡도** — Phase A (파일 기반) → Phase B (벡터 검색) → Phase C (자동 큐레이션).
5. **메모리 카테고리** — 대화 요약, 사실, 에피소드, 지식의 4가지 타입.

---

## 3. 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                       AgentRuntime                          │
│                                                             │
│  ① 세션 시작 → MemoryManager.recall() → 관련 메모리       │
│     → system_prompt에 블렌딩                               │
│                                                             │
│  ② 에이전트 실행 중 → memory_read/write/search 도구 사용   │
│                                                             │
│  ③ CompactionEvent::Completed → MemoryManager.remember()    │
│     → summary를 conversation 메모리로 저장                  │
│                                                             │
│  ④ 세션 종료 → MemoryManager.summarize_session()           │
│     → 전체 세션 요약을 저장                                 │
└─────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────┐  ┌──────────────┐  ┌──────────────────┐
│  MemoryManager  │  │  AgentTool   │  │  StateStore      │
│  (신규 모듈)    │  │  (신규 3개)  │  │  (기존, 확장)    │
│                 │  │              │  │                  │
│  remember()     │  │  memory_read │  │  memory/         │
│  recall()       │  │  memory_write│  │  ├── daily/      │
│  search()       │  │  memory_search│ │  ├── facts/      │
│  summarize()    │  │              │  │  ├── episodes/   │
│                 │  │              │  │  └── knowledge/  │
└─────────────────┘  └──────────────┘  └──────────────────┘
```

---

## 4. 메모리 모델

### 4.1 메모리 타입

```rust
/// 메모리 엔트리의 종류
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryType {
    /// 대화 요약 (CompactionManager가 자동 생성)
    Conversation,
    /// 세션 요약 (세션 종료 시 자동 생성)
    Session,
    /// 에이전트가 명시적으로 저장한 사실
    Fact,
    /// 에피소드 기억 (특정 이벤트/경험)
    Episode,
    /// 정적 지식 (프로그램/사용자가 제공)
    Knowledge,
}

/// 단일 메모리 엔트리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// 고유 ID
    pub id: String,
    /// 메모리 타입
    pub memory_type: MemoryType,
    /// 내용 (Markdown)
    pub content: String,
    /// 메모리를 생성한 에이전트/사용자
    pub source: String,
    /// 관련 세션 ID
    pub session_id: Option<String>,
    /// 태그 (검색용)
    pub tags: Vec<String>,
    /// 중요도 (0.0 ~ 1.0)
    pub importance: f32,
    /// 생성 시각
    pub created_at: DateTime<Utc>,
    /// 마지막 접근 시각
    pub accessed_at: DateTime<Utc>,
    /// 접근 횟수
    pub access_count: u32,
}
```

### 4.2 파일 시스템 레이아웃

```
state/
├── memory/
│   ├── conversations/        ← CompactionManager 요약
│   │   ├── 2024-05-07-session-abc123.json
│   │   └── 2024-05-06-session-def456.json
│   ├── sessions/             ← 세션 종료 요약
│   │   └── session-abc123.json
│   ├── facts/                ← 에이전트가 명시적으로 저장
│   │   └── rust-best-practices.json
│   ├── episodes/             ← 에피소드 기억
│   │   └── fixed-bug-123.json
│   └── knowledge/            ← 정적 지식 (기존)
│       └── api-reference.md
├── sessions/                 ← 기존 세션 히스토리
├── seeds/                    ← 기존
└── evals/                    ← 기존
```

---

## 5. MemoryManager

### 5.1 공개 API

```rust
/// 에이전트 메모리 관리자
pub struct MemoryManager {
    state_store: Arc<StateStore>,
}

impl MemoryManager {
    pub fn new(state_store: Arc<StateStore>) -> Self;

    /// 메모리 저장 (에이전트 도구 또는 시스템에서 호출)
    pub async fn remember(&self, entry: MemoryEntry) -> Result<String>;

    /// 관련 메모리 검색 (새 세션 시작 시)
    pub async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>>;

    /// 키워드/태그 검색
    pub async fn search(&self, query: &str, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryEntry>>;

    /// 단일 메모리 조회
    pub async fn get(&self, id: &str) -> Result<Option<MemoryEntry>>;

    /// 메모리 삭제
    pub async fn forget(&self, id: &str) -> Result<bool>;

    /// 타입별 목록
    pub async fn list(&self, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryEntry>>;

    /// 세션 종료 시 전체 요약 생성 (LLM 호출)
    pub async fn summarize_session(&self, session: &Session, model_id: &str, provider: Arc<dyn Provider>) -> Result<MemoryEntry>;

    /// 컨텍스트 블렌딩: 관련 메모리를 system prompt에 주입
    pub fn blend_into_prompt(&self, memories: &[MemoryEntry], system_prompt: &str) -> String;
}
```

### 5.2 recall 알고리즘 (Phase A — 휴리스틱)

Phase A에서는 벡터 임베딩 없이 휴리스틱으로 관련 메모리를 찾는다:

```rust
pub async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
    // 1. 최근 대화 요약 상위 3개 (항상 포함)
    let recent = self.list(Some(MemoryType::Conversation), 3).await?;

    // 2. 최근 세션 요약 상위 2개
    let sessions = self.list(Some(MemoryType::Session), 2).await?;

    // 3. 키워드 매칭 facts/episodes
    let keywords = extract_keywords(query);
    let mut relevant = Vec::new();
    for entry in self.list(Some(MemoryType::Fact), 20).await? {
        if keywords.iter().any(|k| entry.content.contains(k) || entry.tags.contains(k)) {
            relevant.push(entry);
        }
    }
    for entry in self.list(Some(MemoryType::Episode), 20).await? {
        if keywords.iter().any(|k| entry.content.contains(k) || entry.tags.contains(k)) {
            relevant.push(entry);
        }
    }

    // 4. 중요도순 정렬 + limit
    relevant.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));
    relevant.truncate(limit);

    // 5. 합치기 (중복 제거)
    let mut combined = recent;
    combined.extend(sessions);
    combined.extend(relevant);
    dedup_by_id(&mut combined);
    combined.truncate(limit);
    Ok(combined)
}
```

### 5.3 blend_into_prompt

```rust
pub fn blend_into_prompt(&self, memories: &[MemoryEntry], system_prompt: &str) -> String {
    if memories.is_empty() {
        return system_prompt.to_string();
    }

    let memory_block = memories.iter().map(|m| {
        format!("- [{}] {}", m.memory_type_label(), m.content)
    }).collect::<Vec<_>>().join("\n");

    format!(
        "{system_prompt}\n\n## Relevant Memory\n\n{memory_block}"
    )
}
```

---

## 6. 에이전트 도구 (AgentTool)

### 6.1 memory_write

```rust
/// 에이전트가 메모리에 쓰는 도구
pub struct MemoryWriteTool {
    memory_manager: Arc<MemoryManager>,
}

// AgentTool 구현:
// name: "memory_write"
// description: "Store information in long-term memory for future sessions"
// parameters: {
//   content: string (required) — 메모리 내용
//   memory_type: "fact" | "episode" (optional, default "fact")
//   tags: string[] (optional) — 검색용 태그
//   importance: number (optional, default 0.5) — 중요도 0.0~1.0
// }
```

### 6.2 memory_read

```rust
/// 에이전트가 메모리에서 읽는 도구
pub struct MemoryReadTool {
    memory_manager: Arc<MemoryManager>,
}

// name: "memory_read"
// description: "Retrieve a specific memory entry by ID"
// parameters: {
//   id: string (required) — 메모리 ID
// }
```

### 6.3 memory_search

```rust
/// 에이전트가 메모리를 검색하는 도구
pub struct MemorySearchTool {
    memory_manager: Arc<MemoryManager>,
}

// name: "memory_search"
// description: "Search long-term memory for relevant information"
// parameters: {
//   query: string (required) — 검색 쿼리
//   memory_type: string (optional) — 필터링할 타입
//   limit: number (optional, default 5)
// }
```

---

## 7. 통합 포인트

### 7.1 Compaction → 메모리 자동 저장

`run_agent_loop`에서 CompactionEvent를 가로채서 메모리에 저장:

```rust
// agent_runtime.rs의 run_agent_loop에서
// 에이전트 루프 실행 후 이벤트 처리 시
match event {
    AgentEvent::Compaction { event } => match event {
        CompactionEvent::Completed { result, .. } => {
            // compaction summary를 conversation 메모리로 저장
            if let Some(ref memory_manager) = memory_manager {
                let entry = MemoryEntry {
                    id: format!("compact-{}", Uuid::new_v4()),
                    memory_type: MemoryType::Conversation,
                    content: result.summary.clone(),
                    source: "compaction".into(),
                    session_id: session_id.clone(),
                    tags: vec![],
                    importance: 0.5,
                    created_at: Utc::now(),
                    accessed_at: Utc::now(),
                    access_count: 0,
                };
                let _ = memory_manager.remember(entry).await;
            }
        }
        _ => {}
    },
    _ => {}
}
```

### 7.2 세션 시작 → 메모리 블렌딩

`run_agent_loop` 시작 전, 관련 메모리를 system_prompt에 주입:

```rust
// run_agent_loop 시작 부분
let system_prompt = if let Some(ref memory_manager) = memory_manager {
    let memories = memory_manager.recall(&user_message, 10).await
        .unwrap_or_default();
    memory_manager.blend_into_prompt(&memories, &base_system_prompt)
} else {
    base_system_prompt.clone()
};
```

### 7.3 세션 종료 → 전체 요약

세션 종료 시 (Orchestrator의 handle_message 완료 후):

```rust
// orchestrator.rs — 에이전트 실행 완료 후
if let Some(ref memory_manager) = self.memory_manager {
    if let Ok(session) = self.state_store.load_session(&session_id).await {
        let _ = memory_manager.summarize_session(
            &session,
            &model_id,
            provider.clone(),
        ).await;
    }
}
```

### 7.4 API 확장

routes에 메모리 CRUD 추가:

```
POST   /api/memory              ← 메모리 생성
GET    /api/memory               ← 메모리 목록 (기존, 타입 필터 추가)
GET    /api/memory/{id}          ← 메모리 조회
PUT    /api/memory/{id}          ← 메모리 수정
DELETE /api/memory/{id}          ← 메모리 삭제
POST   /api/memory/search        ← 메모리 검색
```

---

## 8. 구현 계획

### Phase A: 파일 기반 메모리 (Loop 5)

목표: compaction summary 자동 저장 + 에이전트 도구 + 세션 간 기억.

| Step | 내용 | 파일 | 예상 줄수 |
|------|------|------|----------|
| A1 | MemoryEntry, MemoryType 타입 정의 | `kernel/src/memory.rs` (신규) | ~150 |
| A2 | MemoryManager (remember, recall, search, list, get, forget) | `kernel/src/memory.rs` | ~250 |
| A3 | MemoryWriteTool, MemoryReadTool, MemorySearchTool | `kernel/src/tools/memory_tools.rs` (신규) | ~250 |
| A4 | run_agent_loop에 memory_manager 파라미터 추가, compaction 이벤트 캡처 | `kernel/src/agent_runtime.rs` | ~50 변경 |
| A5 | 세션 시작 시 recall → blend_into_prompt | `kernel/src/agent_runtime.rs` | ~20 변경 |
| A6 | 세션 종료 시 summarize_session | `kernel/src/orchestrator.rs` | ~30 변경 |
| A7 | MemoryManager를 Kernel에 추가, AgentRuntime에 전달 | `src/kernel.rs`, `src/main.rs` | ~30 변경 |
| A8 | API 라우트 추가 (POST/PUT/DELETE /api/memory, POST /api/memory/search) | `routes/workspace.rs` 확장 | ~120 |
| A9 | 통합 테스트 | `kernel/tests/` | ~100 |

**합계: ~1,000줄新增**

### Phase B: 벡터 검색 (Loop 6, 선택)

| 항목 | 설명 |
|------|------|
| 임베딩 | oxi-ai의 provider로 임베딩 생성 (OpenAI embedding API) |
| 저장 | 각 MemoryEntry에 embedding 필드 추가 |
| 인덱스 | `memory/index.json`에 {id → embedding} 매핑 |
| 검색 | cosine similarity로 recall 개선 |
| 의존성 | 외부 벡터 DB 없이 파일 기반 (Phase C에서 SQLite 고려) |

### Phase C: 자동 큐레이션 (Loop 7, 선택)

| 항목 | 설명 |
|------|------|
| 중복 제거 | 유사한 메모리 자동 병합 |
| 중요도 감쇠 | 시간 경과에 따른 importance 감소 |
| 자동 압축 | 오래된 메모리 자동 요약 |
| 메모리 예산 | 총 메모리 크기 제한, LRU 제거 |

---

## 9. KernelEvent 확장

메모리 변경 사항을 SSE로 브로드캐스트:

```rust
// event_bus.rs에 추가
pub enum KernelEvent {
    // ... 기존 변형들 ...

    /// 메모리가 저장됨
    MemoryStored {
        id: String,
        memory_type: String,
        source: String,
    },
    /// 메모리가 검색됨
    MemoryRecalled {
        query: String,
        count: usize,
    },
}
```

---

## 10. config.toml 확장

```toml
[memory]
# 메모리 시스템 활성화
enabled = true
# 세션 시작 시 불러올 최대 메모리 수
max_recall = 10
# 자동 요약 활성화 (세션 종료 시)
auto_summarize = true
# 컴팩션 요약 자동 저장
capture_compaction = true
# 메모리 만료 일수 (0 = 무제한)
retention_days = 90
```

---

## 11. 의존성 그래프

```
A1 (타입) ← A2 (MemoryManager) ← A3 (AgentTool)
                                    ↓
A4 (runtime 통합) ← A5 (recall/blend) ← A6 (summarize)
        ↓
A7 (Kernel/ main.rs)
        ↓
A8 (API routes)
        ↓
A9 (테스트)
```

실행 순서: A1 → A2 → A3 → A4 → A5 → A6 → A7 → A8 → A9

병렬 가능: A2+A3 (A1 완료 후), A5+A6 (A4 완료 후)

---

## 12. 검증 기준

- [ ] 에이전트가 `memory_write`로 정보를 저장하고, 새 세션에서 `memory_search`로 찾을 수 있음
- [ ] CompactionManager의 summary가 자동으로 conversation 메모리에 저장됨
- [ ] 세션 종료 시 전체 요약이 session 메모리로 저장됨
- [ ] 새 세션 시작 시 관련 메모리가 system prompt에 블렌딩됨
- [ ] API로 메모리 CRUD가 가능함
- [ ] SSE로 메모리 변경 이벤트가 브로드캐스트됨
- [ ] `cargo test --workspace` 통과
- [ ] Clippy 0 경고
