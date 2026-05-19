# RFC-005: Knowledge System — 실제 통합 설계

> **날짜**: 2026-05-20
> **상태**: 초안
> **관련**: RFC-004 (지식 시스템 설계), RFC-001 (KernelHandle Facade)
> **대체**: RFC-004의 Phase 5+를 현실적 로드맵으로 대체

---

## 0. 지금 무엇이 문제인가

RFC-004는 비전 문서였다. 4개 Phase의 포팅은 완료됐다. 하지만 "돌아가는 포팅"과
"OS의 일부"는 다른 차원이다. 현재 상태의 구체적 문제:

### 문제 1: knowledge_routes가 KnowledgeApi를 우회한다

```
knowledge_routes.rs (1,055줄)
├── tree/file/delete/search/backlinks/graph → 직접 파일시스템 I/O (tokio::fs)
├── copilot → call_ai_copilot() → oxi 엔진 직접 호출
└── find_relevant_files → 미사용 (데드코드)
```

`state.kernel.knowledge`가 존재하지만 8개 핸들러 중 7개가 이를 무시하고
`knowledge_base_path(state)`로 직접 디스크를 찔러넣는다.
백링크 인덱싱, 메모리 동기화, 에이전트 접근 — 전무.

### 문제 2: VirtualFs API 분리가 비자연스럽다

```rust
// oxios-markdown (files.md 포팅)의 API:
fs.read("brain", "Rust.md")   // dir + filename 분리
fs.write("brain", "Rust.md", content)

// KnowledgeApi가 제공하는 API:
api.note_read("brain/Rust.md")  // 단일 path string
api.note_write("brain/Rust.md", content)
```

`split_path("brain/Rust.md")` → `("brain", "Rust.md")` 변환을
KnowledgeApi에서 수행하는데, 이는 files.md의 설계를 억지로 끼워맞춘 것이다.
files.md 원본의 `dir/filename` 분리는 URL 라우팅에서 온 제약사항이다.
OS 레벨에서는 POSIX path가 자연스럽다.

### 문제 3: 코파일럿이 라우트 핸들러에 하드코딩되어 있다

`call_ai_copilot()`이 `knowledge_routes.rs`에 정의되어 있다.
엔진 생성, 모델 해석, 스트림 수집 — 전부 웹 레이어에.
이건 CLI에서도, 에이전트 내부에서도 copilot을 쓸 수 없다는 뜻이다.

### 문제 4: Space × Knowledge가 연결되지 않았다

`KnowledgeApi::for_space()` 메서드는 있지만 아무도 호출하지 않는다.
KernelHandle의 knowledge는 항상 `workspace/knowledge/`로 고정.
Space 전환 시 knowledge base가 바뀌지 않는다.

### 문제 5: oxios-markdown의 잉여 모듈

`schedule.rs`, `habits.rs`, `journal.rs`, `chat.rs`, `fslog.rs`, `tokens.rs`는
files.md 서버 전용 기능이다. 이들은:
- 온라인 서버의 동기화 클라이언트(fslog, tokens)
- Telegram 봇 전용(chat, schedule)
- files.md 웹앱의 특수 UI(habits, journal)

Oxios에서 이들은 에이전트 도구나 웹 UI로 재구현되어야 한다.
Go 코드를 그대로 포팅해놓고 쓰지 않는 상태.

---

## 1. 설계 원칙

### 1.1 "files.md 포팅"이 아니라 "Oxios Knowledge Layer"다

files.md는 출발점이었다. 이제 Oxios의 아키텍처 언어로 재표현해야 한다.
Go의 함수 시그니처를 Rust로 옮기는 게 아니라, files.md가 해결하던
**사용자 문제**를 Oxios의 **컴포넌트 모델**로 해결하는 것이다.

### 1.2 데이터는 한 곳에, 접근은 여러 경로로

```
~/.oxios/workspace/knowledge/    ← 단일 진실 원천 (.md 파일)
    ↑ write                ↑ write
  인간 (에디터)          에이전트 (KnowledgeApi)
    ↓ read                ↓ search
  인간 (에디터)          에이전트 (KnowledgeTool)
         ↑               ↑
         └── 코파일럿 ──┘ (oxi 엔진, in-process)
```

### 1.3 Space는 곧 Knowledge Scope

Space는 Oxios의 핵심 격리 단위다. Space 전환 = knowledge base 전환이어야 한다.
현재 `KernelHandle::knowledge`는 싱글톤이 아닌 Space-scoped여야 한다.

### 1.4 AI는 커널 기능이다

코파일럿은 웹 라우트의 부속품이 아니다. CLI에서도, 에이전트 내부에서도,
Cron 작업에서도 호출 가능한 **커널 기능**이다.
`EngineProvider`는 이미 커널의 퍼스트클래스 개념이다.

---

## 2. 변경 계획 — 5개 트랙

### Track A: oxios-markdown 정비 (라이브러리 크레이트)

**목표**: files.md 포팅코드 중 Oxios가 실제로 사용하는 핵심만 남기고,
나머지는 files.md 특유의 기능을 Oxios 방식으로 재구현.

#### A1. VirtualFs API 단순화

```rust
// BEFORE (files.md 포팅):
pub fn read(&self, dir: &str, filename: &str) -> Result<String>
pub fn write(&self, dir: &str, filename: &str, content: &str) -> Result<()>
pub fn del(&self, dir: &str, filename: &str) -> Result<()>
pub fn rename(&self, old_dir: &str, old_name: &str, new_dir: &str, new_name: &str) -> Result<()>

// AFTER (POSIX path 기반):
pub fn read(&self, path: &str) -> Result<String>
pub fn write(&self, path: &str, content: &str) -> Result<()>
pub fn delete(&self, path: &str) -> Result<()>
pub fn rename(&self, old_path: &str, new_path: &str) -> Result<()>
pub fn exists(&self, path: &str) -> Result<bool>
pub fn list(&self, dir_path: &str) -> Result<Vec<FileEntry>>
pub fn search(&self, query: &str) -> Result<Vec<FileEntry>>
```

`dir/filename` 분리를 제거하고 단일 path 문자열로 통합.
내부적으로는 `path.split_once('/')`로 디렉토리와 파일명을 분리하되,
공개 API는 단일 path만 받는다.

**영향 범위**: `fs.rs`, KnowledgeApi, KnowledgeTool, knowledge_routes

#### A2. 사용하지 않는 모듈 정리

| 모듈 | 판정 | 이유 |
|------|------|------|
| `fs.rs` (VirtualFs) | ✅ 유지 | 핵심 — 샌드박스 파일 I/O |
| `types.rs` | ✅ 유지 | 핵심 타입 |
| `merge.rs` | ✅ 유지 | 3-way merge (동기화, 충돌 해결) |
| `backlinks.rs` | ✅ 유지 | 백링크 인덱스 — knowledge의 핵심 가치 |
| `parser.rs` | ✅ 유지 | 링크/헤딩 추출 |
| `sync.rs` | ⚠️ 보류 | SyncEngine — 현재 사용 안 함. 멀티디바이스 동기화 시 필요 |
| `chat.rs` | ❌ 제거 | files.md 서버의 Chat.md 조작. Oxios는 `note_read/write`로 처리 |
| `journal.rs` | ❌ 제거 | files.md 서버의 저널 생성. `note_write`로 충분 |
| `habits.rs` | ❌ 제거 | files.md 웹앱 전용. Oxios 에이전트 도구로 재구현 |
| `schedule.rs` | ❌ 제거 | Telegram 봇용. Oxios CronScheduler로 대체 |
| `fslog.rs` | ⚠️ 보류 | 동기화 로그. sync와 함께 멀티디바이스 시 활성화 |
| `tokens.rs` | ❌ 제거 | files.md 서버의 인증 토큰. Oxios AuthManager로 대체 |

`chat/journal/habits/schedule/tokens`은 **Oxios의 기존 메커니즘으로
이미 대체 가능**하다:

- Chat.md → `note_read("Chat.md")` + `note_write("Chat.md", ...)`
- Journal → `note_write("journal/2026.05 May.md", ...)`
- Habits → CronScheduler + StateStore
- Schedule → CronScheduler
- Auth → AuthManager

제거 모듈의 함수 중 재사용 가치가 있는 것들은 `parser.rs`나
별도 유틸리티로 흡수:

- `chat::today_header()` → `parser::today_chat_header()`
- `journal::today_journal_filename()` → `parser::today_journal_path()`
- `parser::similar()` / `parser::levenshtein()` → 유지 (검색에 사용)

#### A3. sync 모듈의 기능 재배치

files.md의 동기화는 "클라이언트-서버 간 파일 동기화"를 위한 것이다.
Oxios는 단일 서버(24/7 데몬)이므로 클라이언트-서버 동기화가 필요 없다.

하지만 **merge 알고리즘**은 인간-에이전트 동시 편집 시 필수다.
따라서:

- `sync.rs`의 `SyncEngine` → 보류 (멀티디바이스용)
- `merge.rs`의 `merge()` → 적극 활용 (인간-에이전트 충돌 해결)

---

### Track B: KnowledgeApi 재설계 (커널 레이어)

**목표**: KnowledgeApi를 "파일 읽기/쓰기 래퍼"에서 "지식 운영 시스템"으로 격상.

#### B1. 현재 책임 재점검

```rust
// 현재 KnowledgeApi의 책임:
pub fn note_read(&self, path: &str) -> Result<Option<String>>
pub fn note_write(&self, path: &str, content: &str) -> Result<()>
pub fn note_delete(&self, path: &str) -> Result<()>
pub fn note_move(&self, old: &str, new: &str) -> Result<()>
pub fn note_tree(&self, dir: &str) -> Result<Vec<FileEntry>>
pub fn search(&self, query: &str, limit: usize) -> Result<Vec<NoteHit>>
pub fn backlinks_for(&self, path: &str) -> Vec<Backlink>
pub fn link_graph(&self) -> LinkGraph
```

**누락된 책임**:
- AI 코파일럿 (라우트에 하드코딩됨)
- 파일 변경 감지 (FileWatcher 미구현)
- 초기 일괄 인덱싱 (index_all 미구현)
- 중요도 계산 (PageRank 미구현)

#### B2. 새 KnowledgeApi 설계

```rust
pub struct KnowledgeApi {
    fs: Arc<RwLock<VirtualFs>>,
    memory: Arc<MemoryManager>,
    backlinks: Arc<RwLock<BacklinkIndex>>,
    engine: Arc<dyn EngineProvider>,    // NEW: 코파일럿용
    default_model: String,              // NEW: 기본 모델
    agent_writes: Arc<Mutex<HashSet<String>>>,  // NEW: 순환 방지
}

impl KnowledgeApi {
    // ── 라이프사이클 ──────────────────────────────────────

    /// 초기 인덱싱: knowledge/ 디렉토리의 모든 .md 파일을
    /// BacklinkIndex + MemoryManager에 인덱싱.
    pub async fn index_all(&self) -> Result<usize>

    /// 파일 변경 감지 시작 (notify 크레이트).
    /// 에이전트 write는 스킵 (agent_writes로 추적).
    pub async fn start_watcher(&self) -> Result<()>

    // ── 파일 I/O ──────────────────────────────────────────

    pub fn note_read(&self, path: &str) -> Result<Option<String>>
    pub fn note_write(&self, path: &str, content: &str) -> Result<()>
    pub fn note_delete(&self, path: &str) -> Result<()>
    pub fn note_move(&self, old: &str, new: &str) -> Result<()>
    pub fn note_tree(&self, dir: &str) -> Result<Vec<FileEntry>>

    // ── 검색 ──────────────────────────────────────────────

    /// 통합 검색: 이름 + 내용 + 백링크 + 시맨틱.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<NoteHit>>

    // ── 백링크 & 그래프 ───────────────────────────────────

    pub fn backlinks_for(&self, path: &str) -> Vec<Backlink>
    pub fn link_graph(&self) -> LinkGraph

    // ── AI 코파일럿 ───────────────────────────────────────

    /// 코파일럿 질의응답.
    /// 현재 파일 컨텍스트 + 관련 노트 + 관련 메모리를 조합하여
    /// EngineProvider로 응답 생성.
    pub async fn copilot_chat(
        &self,
        question: &str,
        context_path: Option<&str>,
    ) -> Result<CopilotResponse>
}
```

**핵심 변화**:
1. `EngineProvider`가 KnowledgeApi에 주입됨 — 코파일럿이 커널 기능이 됨
2. `index_all()` + `start_watcher()` — 데몬 시작 시 자동 인덱싱
3. `agent_writes` — FileWatcher 순환 방지

#### B3. KernelHandle 조정

현재: `KernelHandle.knowledge`는 전역 싱글톤.
변경: Space 전환 시 KnowledgeApi가 해당 Space의 knowledge/를 가리키도록.

```rust
// kernel.rs에서 KnowledgeApi 생성:
let knowledge = KnowledgeApi::new(
    space_dir.join("knowledge"),  // ← Space-scoped!
    memory_manager.clone(),
    engine_provider.clone(),      // NEW: EngineProvider 주입
    config.engine.default_model.clone(),
);
```

Space 전환 시 `KernelHandle`을 재구성하거나,
`KnowledgeApi` 내부에서 `fs` 루트를 동적으로 교체.

**설계 선택**: 현재 Oxios는 활성 Space 개념이 있다.
`SpaceManager::get_active_space()`로 현재 Space를 찾고,
그 Space의 `knowledge/` 하위 디렉토리를 사용.

#### B4. EngineProvider 주입 문제

현재 `OxiEngineProvider`는 `oxios-kernel`에 정의되어 있다.
하지만 `KnowledgeApi`가 `EngineProvider`를 필요로 하면
`oxios-kernel`이 자기 자신의 trait에 의존하게 된다.

이는 이미 해결된 패턴이다:
- `AgentRuntime`이 `EngineProvider`를 받음
- `AgentApi`가 `EngineProvider`를 간접적으로 사용

동일하게, `KnowledgeApi::new()`가 `Arc<dyn EngineProvider>`를 받는다.
`kernel.rs`에서 조립 시 `OxiEngineProvider`를 생성해서 주입.

---

### Track C: Web 라우트 재설계 (프레젠테이션 레이어)

**목표**: knowledge_routes.rs를 KnowledgeApi의 얇은 어댑터로 만들기.

#### C1. 현재 구조의 문제

```
knowledge_routes.rs (1,055줄)
├── knowledge_base_path(state) → 직접 경로 계산
├── tokio::fs::read_dir() → 직접 파일시스템 접근
├── tokio::fs::read() / write() → 직접 파일 I/O
├── call_ai_copilot() → oxi 엔진 직접 호출
└── collect_stream_text() → 스트림 유틸
```

이 핸들러들은 **커널을 우회**한다. 커널의 존재 이유인
"모든 접근은 KernelHandle을 통해" 원칙을 위반.

#### C2. 목표 구조

```
knowledge_routes.rs (~300줄)
├── handle_knowledge_tree → state.kernel.knowledge.note_tree()
├── handle_knowledge_file_get → state.kernel.knowledge.note_read()
├── handle_knowledge_file_put → state.kernel.knowledge.note_write()
├── handle_knowledge_file_delete → state.kernel.knowledge.note_delete()
├── handle_knowledge_search → state.kernel.knowledge.search()
├── handle_knowledge_backlinks → state.kernel.knowledge.backlinks_for()
├── handle_knowledge_graph → state.kernel.knowledge.link_graph()
└── handle_knowledge_copilot → state.kernel.knowledge.copilot_chat()
```

각 핸들러는:
1. HTTP 요청 파싱 (path, body, query)
2. `state.kernel.knowledge.*()` 호출
3. 결과를 JSON으로 직렬화하여 반환

파일시스템 접근, 엔진 호출, 스트림 수집 — **전부 KnowledgeApi 내부로 이동**.

#### C3. 미디어 파일 처리

현재 files.md는 이미지(`media/`)도 다룬다. KnowledgeApi는 `.md`만 담당.
이미지는 별도 엔드포인트로 직접 서빙:

```rust
// 미디어는 Axum의 ServeDir로 직접 서빙 (API 거치지 않음)
.nest_service("/knowledge/media", ServeDir::new(knowledge_dir.join("media")))
```

---

### Track D: files.md JS 프론트엔드 현대화

**목표**: files.md의 JS 코드를 Oxios의 실제 API에 완전히 맞추고,
부족한 UI 요소(사이드바 Knowledge 링크, 그래프 뷰)를 추가.

#### D1. oxios-adapter.js → app.js 직접 수정

현재 구조:
```
app.js → oxios-adapter.js의 함수를 호출하려 했지만...
실제로는 app.js가 여전히 files.md 원본 API 호출을 사용
```

문제: `app.js` (27KB), `files.js` (48KB), `chat.js` (39KB) —
세 파일이 files.md의 원본 동기화 프로토콜에 깊이 묶여 있다.
`oxios-adapter.js`는 스텁일 뿐, 실제 연동이 안 됨.

**접근 방식 변경**: adapter 패턴을 버리고 app.js를 직접 수정.

변경점:
1. `app.js`의 `post()` / `get()` 함수를 Oxios REST API로 교체
2. files.md의 동기화 프로토콜(SyncFilenames) 제거 → 단순 CRUD
3. `chat.js`의 코파일럿을 `POST /api/knowledge/copilot`에 연결
4. `files.js`의 파일 트리를 `GET /api/knowledge/tree`에 연결

#### D2. 사이드바에 Knowledge 추가

```rust
// channels/oxios-web/frontend/src/components/sidebar.rs
// Panel enum에 Knowledge 변형 추가:
pub enum Panel {
    // ... 기존 패널들 ...
    Knowledge,  // NEW
}

// NavItem에 추가:
NavItem { panel: Panel::Knowledge, label: "Knowledge", section: Section::System },
```

Knowledge 패널 클릭 시 → `/knowledge/`로 이동 (files.md 에디터 로드).

#### D3. 그래프 시각화

files.md 원본에는 링크 그래프 시각화가 없다.
백링크 데이터는 있으니 D3.js나 Sigma.js로 간단한 force-directed graph를
`/knowledge/` 내에 패널로 추가.

초기 구현: `GET /api/knowledge/graph`의 JSON을 D3.js force layout으로 렌더링.

---

### Track E: Space × Knowledge 통합

**목표**: Space 전환 시 knowledge base가 함께 전환.

#### E1. Space 디렉토리 구조

```
~/.oxios/workspace/
├── spaces/
│   ├── {space-id-1}/
│   │   ├── knowledge/        ← Space 1의 노트
│   │   │   ├── Chat.md
│   │   │   ├── brain/
│   │   │   └── journal/
│   │   └── memory/           ← Space 1의 에이전트 메모리
│   └── {space-id-2}/
│       ├── knowledge/        ← Space 2의 노트
│       └── memory/
└── knowledge/                ← 디폴트 (Space 미지정 시)
```

#### E2. SpaceApi × KnowledgeApi 연동

```rust
// Space 전환 시:
impl SpaceApi {
    pub async fn activate(&self, space_id: SpaceId) -> Result<()> {
        // ... 기존 Space 전환 로직 ...
        // KnowledgeApi의 루트를 새 Space의 knowledge/로 교체
        // → KernelHandle 재구성 또는 KnowledgeApi::set_root()
    }
}
```

#### E3. KnowledgeBridge 실구현

현재 `knowledge_bridge.rs`의 `reference()` / `transfer()`는 스텁.
KnowledgeApi가 Space-scoped가 되면 실제 구현 가능:

```rust
impl KnowledgeBridge {
    /// 다른 Space의 노트를 읽기 전용으로 검색.
    pub async fn reference(
        &self,
        from_space: SpaceId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<NoteHit>> {
        let from_knowledge = self.get_knowledge_api(from_space).await?;
        from_knowledge.search(query, limit)
    }

    /// 한 Space의 노트를 다른 Space로 복사.
    pub async fn transfer(
        &self,
        from_space: SpaceId,
        to_space: SpaceId,
        paths: &[String],
    ) -> Result<usize> {
        let from_knowledge = self.get_knowledge_api(from_space).await?;
        let to_knowledge = self.get_knowledge_api(to_space).await?;
        let mut count = 0;
        for path in paths {
            if let Some(content) = from_knowledge.note_read(path)? {
                to_knowledge.note_write(path, &content)?;
                count += 1;
            }
        }
        Ok(count)
    }
}
```

---

## 3. 구현 순서

의존성 그래프 기반으로 정렬. 위에서 아래로 순차 진행:

```
Track A (oxios-markdown 정비)
  ↓
Track B (KnowledgeApi 재설계) ← A의 새 VirtualFs API에 의존
  ↓
Track C (Web 라우트 재설계) ← B의 새 KnowledgeApi에 의존
  ↓
Track D (JS 프론트엔드) ← C의 API 엔드포인트에 의존
  ↓
Track E (Space 통합) ← B가 완료된 후 가능
```

### 마일스톤

| 마일스톤 | 내용 | 예상 공수 |
|----------|------|----------|
| **M1** | Track A: oxios-markdown 정비 | 2-3시간 |
| **M2** | Track B: KnowledgeApi 재설계 (copilot 포함) | 3-4시간 |
| **M3** | Track C: knowledge_routes 경량화 | 1-2시간 |
| **M4** | Track D: JS 연동 + 사이드바 + 그래프 뷰 | 3-4시간 |
| **M5** | Track E: Space × Knowledge 통합 | 2-3시간 |

M1-M3은 순차. M4-M5는 M3 이후 병렬 가능.

---

## 4. Track A 상세 — oxios-markdown 정비

### A1. VirtualFs API 리팩토링

```rust
// fs.rs — 새 공개 API

impl VirtualFs {
    pub fn new(root: PathBuf) -> Result<Self> { /* 그대로 */ }
    pub fn root(&self) -> &Path { /* 그대로 */ }

    // 새 POSIX-style API
    pub fn read(&self, path: &str) -> Result<String> { ... }
    pub fn write(&self, path: &str, content: &str) -> Result<()> { ... }
    pub fn delete(&self, path: &str) -> Result<()> { ... }
    pub fn rename(&self, old: &str, new: &str) -> Result<()> { ... }
    pub fn exists(&self, path: &str) -> Result<bool> { ... }
    pub fn list(&self, dir: &str) -> Result<Vec<FileEntry>> { ... }
    pub fn search(&self, query: &str) -> Result<Vec<FileEntry>> { ... }
    pub fn mtime(&self, path: &str) -> Result<i64> { ... }

    // 기존 dir/filename API는 deprecated로 표시
    #[deprecated(note = "Use read(path) instead")]
    pub fn read_split(&self, dir: &str, filename: &str) -> Result<String> { ... }
}
```

구현: 새 메서드는 내부적으로 `split_path(path)`를 호출하여 기존
`read(dir, filename)` 로직을 재사용. 점진적 마이그레이션.

### A2. 모듈 제거 계획

```
src/lib.rs에서 제거:
- pub mod chat;
- pub mod journal;
- pub mod habits;
- pub mod schedule;
- pub mod tokens;

src/lib.rs에서 유지:
- pub mod fs;
- pub mod types;
- pub mod merge;
- pub mod backlinks;
- pub mod parser;
- pub mod sync;     // 보류, #[allow(dead_code)]
- pub mod fslog;    // 보류, #[allow(dead_code)]
```

삭제 파일은 Git 히스토리에 남으니 언제든 복구 가능.

### A3. 유용한 함수 이관

```rust
// parser.rs에 추가:
/// 오늘 날짜의 Chat.md 헤더 문자열 생성.
pub fn today_chat_header() -> String {
    format!("#### {}", chrono::Local::now().format("%d %B, %A"))
}

/// 오늘 날짜의 저널 파일 경로.
pub fn today_journal_path() -> String {
    format!("journal/{}.{}", chrono::Local::now().format("%Y.%m"), 
            chrono::Local::now().format(" %B"))
}
```

---

## 5. Track B 상세 — KnowledgeApi 재설계

### B1. 새 생성자

```rust
impl KnowledgeApi {
    pub fn new(
        knowledge_dir: PathBuf,
        memory: Arc<MemoryManager>,
        engine: Arc<dyn EngineProvider>,
        default_model: String,
    ) -> Self {
        let fs = VirtualFs::new(knowledge_dir).expect("VirtualFs creation failed");
        Self {
            fs: Arc::new(RwLock::new(fs)),
            memory,
            backlinks: Arc::new(RwLock::new(BacklinkIndex::new())),
            engine,
            default_model,
            agent_writes: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}
```

### B2. copilot_chat 구현

```rust
impl KnowledgeApi {
    pub async fn copilot_chat(
        &self,
        question: &str,
        context_path: Option<&str>,
    ) -> Result<CopilotResponse> {
        let mut context_parts = Vec::new();
        let mut referenced_notes = Vec::new();

        // 1. 현재 파일 컨텍스트
        if let Some(path) = context_path {
            if let Some(content) = self.note_read(path)? {
                let snippet: String = content.chars().take(2000).collect();
                context_parts.push(format!("## Current: {}\n\n{}", path, snippet));
                referenced_notes.push(path.to_string());
            }
        }

        // 2. 관련 노트 검색
        let hits = self.search(question, 5).unwrap_or_default();
        for hit in &hits {
            if referenced_notes.contains(&hit.path) { continue; }
            if let Some(content) = self.note_read(&hit.path)? {
                let snippet: String = content.chars().take(500).collect();
                context_parts.push(format!("## Related: {}\n\n{}", hit.path, snippet));
                referenced_notes.push(hit.path.clone());
            }
        }

        // 3. 관련 메모리 검색
        let mem_rt = tokio::runtime::Handle::try_current();
        let mut referenced_memories = Vec::new();
        if let Ok(rt) = mem_rt {
            if let Ok(entries) = rt.block_on(
                self.memory.search(question, None, 3)
            ) {
                for mem in &entries {
                    context_parts.push(format!(
                        "## Memory [{}]: {}",
                        mem.memory_type.label(),
                        mem.content.chars().take(300).collect::<String>()
                    ));
                    referenced_memories.push(mem.id.clone());
                }
            }
        }

        // 4. AI 호출
        let system_prompt = build_copilot_prompt(&context_parts);
        let response = self.call_engine(&system_prompt, question).await?;

        Ok(CopilotResponse {
            content: response,
            referenced_notes,
            referenced_memories,
        })
    }

    /// EngineProvider를 통한 AI 호출.
    /// block_in_place로 !Send 문제를 회피.
    fn call_engine(&self, system_prompt: &str, question: &str) -> Result<String> {
        let engine = self.engine.clone();
        let model_id = self.default_model.clone();
        let sp = system_prompt.to_string();
        let q = question.to_string();

        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let provider_name = model_id.split_once('/').map(|(p, _)| p).unwrap_or("anthropic");
                let provider = engine.create_provider(provider_name)
                    .map_err(|e| anyhow::anyhow!("Provider error: {e}"))?;
                let model = engine.resolve_model(&model_id)
                    .map_err(|e| anyhow::anyhow!("Model error: {e}"))?;

                let mut ctx = oxi_sdk::Context::new();
                ctx.set_system_prompt(&sp);
                ctx.add_message(oxi_sdk::Message::User(oxi_sdk::UserMessage::new(&q)));

                let stream = provider.stream(&model, &ctx, None).await
                    .map_err(|e| anyhow::anyhow!("Stream error: {e}"))?;

                let mut text = String::new();
                let mut pinned = std::pin::pin!(stream);
                while let Some(event) = pinned.next().await {
                    match event {
                        oxi_sdk::ProviderEvent::TextDelta { delta, .. } => text.push_str(&delta),
                        oxi_sdk::ProviderEvent::Done { .. } => break,
                        oxi_sdk::ProviderEvent::Error { error, .. } => {
                            return Err(anyhow::anyhow!("AI error: {:?}", error));
                        }
                        _ => {}
                    }
                }
                Ok(text)
            })
        })
    }
}
```

### B3. index_all + FileWatcher

```rust
impl KnowledgeApi {
    /// knowledge/ 디렉토리의 모든 .md 파일을 인덱싱.
    /// 데몬 시작 시 한 번 호출.
    pub fn index_all(&self) -> Result<usize> {
        let fs = self.fs.read();
        let entries = fs.list("")?; // 루트
        let mut count = 0;

        for entry in entries {
            if entry.is_dir {
                let sub = fs.list(&entry.name)?;
                for sub_entry in sub {
                    if !sub_entry.is_dir && sub_entry.name.ends_with(".md") {
                        let path = format!("{}/{}", entry.name, sub_entry.name);
                        if let Ok(content) = fs.read(&path) {
                            self.backlinks.write().index_file(&path, &content);
                            count += 1;
                        }
                    }
                }
            } else if entry.name.ends_with(".md") {
                if let Ok(content) = fs.read(&entry.name) {
                    self.backlinks.write().index_file(&entry.name, &content);
                    count += 1;
                }
            }
        }

        tracing::info!(files = count, "Knowledge base indexed");
        Ok(count)
    }
}
```

FileWatcher는 Phase 2로 미루고, 초기 구현은 `index_all()`만.

---

## 6. Track C 상세 — 라우트 경량화

### C1. 각 핸들러가 KnowledgeApi만 호출

```rust
pub(crate) async fn handle_knowledge_tree(
    state: State<Arc<AppState>>,
    Query(params): Query<KnowledgeTreeParams>,
) -> Result<Json<Vec<KnowledgeTreeEntry>>, AppError> {
    let dir = params.dir.as_deref().unwrap_or("");
    let entries = state.kernel.knowledge.note_tree(dir)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(entries.into_iter().map(|e| KnowledgeTreeEntry {
        name: e.name,
        is_dir: e.is_dir,
        size: e.size,
    }).collect()))
}

pub(crate) async fn handle_knowledge_copilot(
    state: State<Arc<AppState>>,
    Json(body): Json<KnowledgeCopilotBody>,
) -> Result<Json<CopilotResponse>, AppError> {
    let result = state.kernel.knowledge.copilot_chat(
        &body.question,
        body.context_path.as_deref(),
    ).await.map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(result))
}
```

코드량: ~300줄로 축소 (1,055 → 300).

### C2. 미디어 파일 서빙

```rust
// routes/mod.rs에 추가:
.nest_service(
    "/knowledge/media",
    ServeDir::new(knowledge_dir.join("media"))
)
```

---

## 7. Track D 상세 — JS 프론트엔드

### D1. app.js 수정 전략

files.md의 `app.js`는 동기화 프로토콜(SyncFilenames)에 깊이 묶여 있다.
Oxios는 동기화가 필요 없다 (단일 서버).

수정 포인트:
1. `API_HOST` / `API_URL` → 빈 문자열 (same-origin)
2. `post()` 함수의 동기화 로직 제거 → 단순 PUT
3. `getFiles()` → `GET /api/knowledge/tree`
4. `getFile()` → `GET /api/knowledge/file/{path}`
5. `saveFile()` → `PUT /api/knowledge/file/{path}`
6. `chat.js`의 코파일럿 → `POST /api/knowledge/copilot`

### D2. 에디터 임베딩 방식

현재: `/knowledge/`가 `static/knowledge/index.html`을 서빙.
Dioxus 사이드바에서 `<a href="/knowledge/">`로 이동.

문제: Dioxus 라우팅과 files.md 라우팅이 충돌하지 않아야 함.
해결: `/knowledge/` 경로를 Axum에서 직접 서빙하고,
Dioxus의 client-side routing은 `/` 하위 경로만 담당.

---

## 8. Track E 상세 — Space × Knowledge

### E1. KernelHandle 재구성

현재 `KernelHandle`은 불변(immutable)이다. Space 전환 시
전체 핸들을 재구성해야 하는데, 이는 이미 `Kernel::handle()`이
`OnceLock`로 캐싱하므로 문제가 된다.

해결 방안:
1. **옵션 A**: `KnowledgeApi` 내부에 `active_space: RwLock<SpaceId>`를 두고
   Space 전환 시 `fs` 루트만 교체
2. **옵션 B**: Space 전환 시 `KernelHandle`을 재생성 (OnceLock 무효화)

옵션 A가 더 현실적. KnowledgeApi가 현재 Space의 knowledge/를 추적:

```rust
impl KnowledgeApi {
    /// Space 전환 시 knowledge base 루트를 변경.
    pub fn switch_space(&self, space_dir: &Path) {
        let new_root = space_dir.join("knowledge");
        let new_fs = VirtualFs::new(new_root).expect("VirtualFs creation failed");
        *self.fs.write() = new_fs;
        self.backlinks.write().clear();
        // TODO: index_all() 재실행
    }
}
```

### E2. KnowledgeApi 레지스트리

또는 Space별 KnowledgeApi를 캐싱:

```rust
pub struct KnowledgeRegistry {
    apis: RwLock<HashMap<SpaceId, Arc<KnowledgeApi>>>,
    memory: Arc<MemoryManager>,
    engine: Arc<dyn EngineProvider>,
    default_model: String,
}

impl KnowledgeRegistry {
    pub fn get_or_create(&self, space_id: SpaceId, space_dir: &Path) -> Arc<KnowledgeApi> {
        let apis = self.apis.read();
        if let Some(api) = apis.get(&space_id) {
            return api.clone();
        }
        drop(apis);

        let api = Arc::new(KnowledgeApi::new(
            space_dir.join("knowledge"),
            self.memory.clone(),
            self.engine.clone(),
            self.default_model.clone(),
        ));

        self.apis.write().insert(space_id, api.clone());
        api
    }
}
```

이 접근이 더 깔끔하다. KernelHandle은 `knowledge` 대신
`knowledge_registry: KnowledgeRegistry`를 가지고,
현재 Space에 해당하는 API를 동적으로 제공.

---

## 9. 파일 변경 요약

### 변경 파일

| 파일 | 변경 유형 | 설명 |
|------|----------|------|
| `crates/oxios-markdown/src/fs.rs` | 수정 | POSIX path API 추가, 기존 API deprecated |
| `crates/oxios-markdown/src/lib.rs` | 수정 | 미사용 모듈 제거 (chat, journal, habits, schedule, tokens) |
| `crates/oxios-markdown/src/parser.rs` | 수정 | 유용한 함수 이관 (today_chat_header 등) |
| `crates/oxios-markdown/src/chat.rs` | 삭제 | note_read/write로 대체 |
| `crates/oxios-markdown/src/journal.rs` | 삭제 | note_read/write로 대체 |
| `crates/oxios-markdown/src/habits.rs` | 삭제 | 에이전트 도구로 재구현 |
| `crates/oxios-markdown/src/schedule.rs` | 삭제 | CronScheduler로 대체 |
| `crates/oxios-markdown/src/tokens.rs` | 삭제 | AuthManager로 대체 |
| `crates/oxios-kernel/src/kernel_handle/knowledge_api.rs` | 재작성 | EngineProvider 주입, copilot, index_all |
| `crates/oxios-kernel/src/tools/kernel/knowledge_tool.rs` | 수정 | 새 API에 맞춤 |
| `channels/oxios-web/src/routes/knowledge_routes.rs` | 재작성 | KnowledgeApi 어댑터로 경량화 |
| `channels/oxios-web/src/routes/mod.rs` | 수정 | 라우트 정리 |
| `channels/oxios-web/frontend/src/components/sidebar.rs` | 수정 | Knowledge 패널 추가 |
| `channels/oxios-web/static/knowledge/app.js` | 수정 | Oxios API 직접 호출 |
| `channels/oxios-web/static/knowledge/oxios-adapter.js` | 삭제 | app.js에 직접 통합 |
| `src/kernel.rs` | 수정 | KnowledgeApi에 EngineProvider 주입 |
| `docs/rfc-004-knowledge-system.md` | 수정 | Phase 5+를 이 RFC로 대체 |

### 신규 파일

| 파일 | 설명 |
|------|------|
| `docs/rfc-005-knowledge-integration.md` | 이 문서 |
| `crates/oxios-kernel/src/kernel_handle/knowledge_registry.rs` | Space × Knowledge 레지스트리 (Track E) |

---

## 10. 리스크

### 10.1 files.md JS 수정 범위

`app.js` (27KB) + `files.js` (48KB) + `chat.js` (39KB) = 114KB의 JS를 수정.
files.md 원본은 동기화 프로토콜에 깊이 묶여 있어 수정량이 많을 수 있다.

**완화**: 수정 범위를 API 호출 함수로 한정.
UI 렌더링, 에디터, CSS는 그대로 유지.

### 10.2 block_in_place의 안전성

`copilot_chat`에서 `tokio::task::block_in_place`를 사용.
멀티스레드 tokio 런타임에서만 안전함.

**완화**: 현재 Oxios는 멀티스레드 런타임(`#[tokio::main]`)을 사용.
문제가 되면 `tokio::spawn` + 채널로 교체.

### 10.3 EngineProvider의 Send 문제

`provider.stream()`이 `!Send` future를 반환하는 근본 원인은
oxi-ai의 provider 구현에 있다. `block_in_place`는 우회책이다.

**근본 해결**: oxi-ai의 provider 구현에서 `Send` 보장.
이건 oxi-ai 업스트림 수정이 필요.

---

## 11. 성공 기준

| 기준 | 측정 방법 |
|------|----------|
| knowledge_routes.rs < 400줄 | `wc -l` |
| 모든 핸들러가 KnowledgeApi만 호출 | `grep -r 'tokio::fs' knowledge_routes.rs` → 0 |
| 코파일럿이 CLI에서도 동작 | `oxios run --json "copilot: 내 노트에서 X 찾아줘"` |
| Space 전환 시 knowledge base 전환 | Space A 노트 ≠ Space B 노트 |
| 전체 테스트 통과 | `cargo test --workspace` |
| oxios-markdown 미사용 모듈 0개 | `cargo check` 경고 0 |
| 백링크가 write 시 자동 인덱싱 | note_write → backlinks_for 즉시 반환 |
