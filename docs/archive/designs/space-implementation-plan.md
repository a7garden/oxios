# Space System Implementation Plan

Design: `docs/design/space-system-design.md`

## Phase 1: Foundation

### T1 — Space 타입 정의
- **File**: `crates/oxios-kernel/src/space.rs` (new)
- **What**: `SpaceId`, `SpaceSource`, `Space` struct 정의
  - `SpaceId = uuid::Uuid`
  - `SpaceSource::AutoResource | AutoTopic | Manual`
  - `Space` struct: id, name, source, paths, workspace_dir, tags, active, created_at, last_active_at, interaction_count, knowledge_visible
- **How**: serde Serialize/Deserialize derive, Debug, Clone
- **Verify**: `cargo check --package oxios-kernel` 컴파일 확인

### T2 — ConversationBuffer 구현
- **File**: `crates/oxios-kernel/src/space/conversation_buffer.rs` (new)
- **What**: `ConversationTurn`, `ConversationBuffer` struct
  - `ConversationBuffer`: VecDeque<ConversationTurn>, max_turns: usize (default 50)
  - `push_user(msg)`, `push_agent(msg, space_id)`, `recent(n)` 메서드
  - `pattern_changed()` — 발화 패턴 변화 감지 (단어 수, 평균 길이 등)
- **Verify**: 단위 테스트

### T3 — SpaceManager 기본 구조
- **File**: `crates/oxios-kernel/src/space/manager.rs` (new)
- **What**: `SpaceManager` struct + 기본 메서드
  - `new(state_store, event_bus) -> Self`
  - `create_from_path(name, path) -> Space`
  - `create_from_topic(topic) -> Space`
  - `list() -> Vec<Space>`
  - `get_space(id) -> Option<Space>`
  - `activate(id) -> Result<()>`
  - `current_space_id() -> SpaceId`
  - `default_space_id() -> SpaceId`
  - `is_in_default_space() -> bool`
  - 인메모리 인덱스: `spaces: HashMap<SpaceId, Space>`, `default_space: SpaceId`
  - StateStore에서 로드, 변경시 저장
- **Storage**: `~/.oxios/spaces/_index.json`에 SpaceId 리스트, 각 `~/.oxios/spaces/{id}/space.json`에 Space 데이터
- **Default Space**: `_default` ID 사용, 앱 초기화시 없으면 자동 생성
- **Verify**: `cargo check`, StateStore 저장/로드 수동 테스트

### T4 — KernelEvent Space 이벤트 추가
- **File**: `crates/oxios-kernel/src/event_bus.rs`
- **What**: `KernelEvent`에 추가
  - `SpaceCreated { space_id, name, source }`
  - `SpaceActivated { space_id, name }`
  - `SpaceArchived { space_id, name }`
  - `KnowledgeCrossReferenced { from_space, to_space, entries, flow }`
  - `SpacesMerged { survivor, absorbed, entries_migrated }`
- **Also**: `kernel_event_to_audit_action()`에 새 이벤트 매핑
- **Verify**: `cargo check`

### T5 — lib.rs export 추가
- **File**: `crates/oxios-kernel/src/lib.rs`
- **What**: 새 모듈 export
  - `pub mod space;`
  - `pub use space::{Space, SpaceId, SpaceSource, SpaceManager, ConversationBuffer, KnowledgeBridge, KnowledgeFlow, CrossRefEntry};`
- **Verify**: `cargo check --package oxios-kernel`

### T6 — Orchestrator 통합 (SpaceManager 연동)
- **File**: `crates/oxios-kernel/src/orchestrator.rs`
- **What**: `Orchestrator`에 SpaceManager + ConversationBuffer 추가
  - `orchestrator.rs`에 `use crate::space::{SpaceManager, ConversationBuffer, SpaceId};` 추가
  - `Orchestrator` struct 필드: `space_manager: SpaceManager`, `conversation_buffer: ConversationBuffer`
  - `Orchestrator::new()` 수정: SpaceManager, ConversationBuffer 초기화
  - `handle_message()` 시작부분 수정:
    1. `conversation_buffer.push_user(user_message)`
    2. `space_manager.detect_or_create(message, &conversation_buffer).await?`
    3. 응답 후 `conversation_buffer.push_agent(response, space_id)`
  - `tag_response()` helper: response에 Space 이모지 태그 부착 (`[🔧 name]` 또는 `[🏠 name]`)
- **Verify**: `cargo check`, 기존 테스트 통과 확인

### T7 — 저장소 레이아웃 초기화
- **File**: kernel.rs (main binary)
- **What**: 앱 시작시 `~/.oxios/spaces/` 디렉토리 + 기본 Space 자동 생성
- **Where**: `crates/oxios/src/` 확인 후 해당 위치에서 초기화
- **Verify**: 앱 실행 후 `ls ~/.oxios/spaces/` 확인

---

## Batch 1: T1–T5 (병렬 — 상호 의존 없음)
## Batch 2: T6 (T3, T4, T5 의존)
## Batch 3: T7 (T3 의존 — default space 생성 로직)

## Phase 2: Detection (1차 + 2차)

### T8 — 파일시스템 경로 추출 (1차 감지)
- **File**: `crates/oxios-kernel/src/space/detection.rs` (new)
- **What**: `extract_filesystem_path(message) -> Option<PathBuf>`
  - 정규식: `/[a-zA-Z0-9_./~\-]+` 계열의 경로 패턴 감지
  - `~/`, `/`, `./` 로 시작하는 경로 인식
  - `find_by_path()`: 주어진 경로와 매칭되는 Space 찾기 (paths 배열에서 prefix 매칭)
- **Verify**: 단위 테스트 (경로 샘플 10개 이상)

### T9 — 키워드/태그 매칭 (2차 감지)
- **File**: `crates/oxios-kernel/src/space/detection.rs` (이어서)
- **What**: `match_keywords(message, spaces) -> Option<SpaceId>`
  - Space.tags 기반 단순 매칭
  - 태그 없이 name 기반 폴백 (이름을 단어로 분리해서 매칭)
- **Verify**: 단위 테스트

### T10 — 3차 LLM 감지 준비 (스켈레톤)
- **File**: `crates/oxios-kernel/src/space/detection.rs` (이어서)
- **What**: `should_check_topic()` + `classify_topic()` 스켈레톤
  - `should_check_topic()`: 3턴마다 또는 `pattern_changed()` 시 true
  - `classify_topic()`: LLM 호출 — 현재는 "일상", "개발", "요리" 등 기본 태그 반환하는 하드코딩으로 구현
  - 이후 Phase 4에서 실제 LLM 연동
- **Verify**: `cargo check`

### T11 — Default Space → Named Space 자동 승격
- **File**: `crates/oxios-kernel/src/space/manager.rs` (수정)
- **What**: `promote_from_default(topic)` 메서드
  - 기본 Space에서 대화가 진행되다가 주제가 명확해지면
  - 새 Named Space 생성 + 최근 ConversationBuffer 내용을 새 Space로 이전 (메모리만)
  - 기본 Space는 다음 대기를 위해 name="" 상태로 리셋
- **Also**: `detect_or_create()`에서 3차 감지 후 `is_in_default_space()` + 주제 명확시 `promote_from_default()` 호출
- **Verify**: 단위 테스트

---

## Batch 4: T8–T9 (병렬 — 같은 파일, sequential로 작성)
## Batch 5: T10–T11 (T8–T9 의존)

## Phase 3: Memory Isolation + Knowledge Bridge

### T12 — Space-scoped MemoryManager
- **File**: `crates/oxios-kernel/src/memory/mod.rs` (수정)
- **What**: `MemoryManager::for_space(space_dir) -> Self` 메서드
  - 기존 `MemoryManager::new(state_store)`基础上
  - `for_space()`가 StateStore를 space별 경로로 초기화
  - `StateStore::new(space_dir.join("memory"))` — space.json의 `workspace_dir` 사용
- **Also**: `MemoryManager`에 `space_id` 필드 추가 (audit용)
- **Verify**: `cargo check`, 기존 memory 테스트 통과

### T13 — AgentRuntime에 Space 컨텍스트 전달
- **File**: `crates/oxios-kernel/src/agent_runtime.rs` (수정)
- **What**: `AgentRuntimeConfig`에 필드 추가
  - `project_paths: Vec<PathBuf>` — Space.paths에서而来
  - `workspace_dir: Option<PathBuf>` — Space.workspace_dir
  - `run_agent_loop()` 수정: `project_paths.first()`를 CWD로 설정, 없으면 `workspace_dir`
  - `WORKSPACE_MUTEX` 제거 검토 — Space마다 경로가 다르므로 동시 실행 가능?
    - **주의**: `set_current_dir`는 프로세스 전역이므로 여전히 mutex 필요할 수 있음
    - 일단 유지하되, 주석으로 Space별 CWD 이슈 명시
- **Verify**: `cargo check`

### T14 — KnowledgeBridge 기초 구현
- **File**: `crates/oxios-kernel/src/space/knowledge_bridge.rs` (new)
- **What**: `KnowledgeBridge` struct
  - `new(space_manager, audit_trail) -> Self`
  - `CrossRefEntry`, `KnowledgeFlow` enum
  - `reference(from_space, to_space, query) -> Vec<MemoryEntry>` — 다른 Space 메모리 검색
    - `knowledge_visible == false`인 Space는 거부
    - audit trail에 기록
  - `transfer(from_space, to_space, entries)` — 메모리 복사
    - 새 Space 생성시 호출 (설계 §9.2)
    - audit trail에 기록
- **Verify**: `cargo check`, 단위 테스트

### T15 — Orchestrator에 KnowledgeBridge 통합
- **File**: `crates/oxios-kernel/src/orchestrator.rs` (수정)
- **What**: `Orchestrator` struct에 `knowledge_bridge: Arc<KnowledgeBridge>` 추가
  - `Orchestrator::new()` 수정
  - 새 Space 생성시: `knowledge_bridge.transfer()` 호출하여 관련 기존 Space에서 지식 이전
  - 에이전트 실행중 필요시: `knowledge_bridge.reference()` — 현재는 하드코딩된 트리거로 (향후 Phase 4에서 LLM이 판단)
- **Verify**: `cargo check`, 기존 테스트 통과

---

## Batch 6: T12 (의존 없음)
## Batch 7: T13 (T12 의존 — MemoryManager 변경 확인)
## Batch 8: T14 (의존 없음, space module)
## Batch 9: T15 (T3, T14 의존)

## Phase 4: Topic Detection (3차, LLM)

### T16 — LLM 기반 주제 분류
- **File**: `crates/oxios-kernel/src/space/detection.rs` (수정)
- **What**: `classify_topic()` 실제 구현
  - LLM 호출: 현재 대화 + ConversationBuffer 내용으로 "주제" 분류
  - 반환: `Topic { name: String, confidence: f32 }` — confidence 낮으면 unclear 취급
  - LLM 비용 최적화: 최근 5턴만 전달, 결과 캐싱
  - 주기적 호출 제한: 같은 주제 반복 분류 방지 (30분 캐시)
- **Provider**: `oxi-ai::Provider` 사용 — KernelConfig에서 provider 참조
- **Also**: `TopicShiftDetector` — 현재 Space 주제 vs 새 주제 비교
- **Verify**: 수동 통합 테스트

## Phase 5: Transparency + Merge/Preserve

### T17 — KernelHandle Space API 추가
- **File**: `crates/oxios-kernel/src/kernel_handle/mod.rs` + `infra_api.rs`
- **What**: Space 관리 REST API 노출
  - `GET /api/spaces` — 목록
  - `GET /api/spaces/{id}` — 상세
  - `POST /api/spaces/{id}/activate` — 활성화
  - `POST /api/spaces/merge` — 병합 요청
  - `POST /api/spaces/{id}/archive` — 보관
  - `POST /api/spaces/{id}/restore` — 복구
- **Verify**: `cargo check`

### T18 — Space 병합
- **File**: `crates/oxios-kernel/src/space/manager.rs` (수정)
- **What**:
  - `merge_spaces(survivor_id, absorbed_id) -> Result<()>`
    - absorbed Space의 메모리를 survivor로 이전
    - absorbed Space의 workspace_dir 내용 보존 (선택적)
    - GitLayer에 병합 전 상태 커밋
    - `SpacesMerged` 이벤트 publish
  - `should_auto_merge()` — 보수적 조건 (동일 paths + 태그 유사도 0.9+ + interaction_count < 5)
- **Also**: OS가 자동 병합 대신 "합칠까요?" 제안하는 로직 (`propose_merge()`)
- **Verify**: 단위 테스트

### T19 — 30일 자동 archive + 즉시 복구
- **File**: `crates/oxios-kernel/src/space/manager.rs` (수정)
- **What**:
  - `archive_stale()` — 30일 비활성 Space archive
  - `restore_from_archive(space_id)` — 보관된 Space 복구
  - Storage: `_archived/{id}/` 디렉토리로 이동 (기존 경로 저장)
  - `detect_or_create()` 수정: archive된 Space 언급시 자동 복구
- **Cron 연동**: `cron.rs`에 archive check job 등록 (일 1회)
- **Verify**: 단위 테스트

---

## Batch 10: T16 (의존 T10)
## Batch 11: T17 (의존 없음)
## Batch 12: T18–T19 (T3 의존)

---

## 검증 기준

모든 Phase 완료 후:
- `cargo build --package oxios-kernel` 성공
- `cargo test --package oxios-kernel` 전부 통과
- `cargo check --package oxios --all-features` (메인 바이너리에서도 확인)