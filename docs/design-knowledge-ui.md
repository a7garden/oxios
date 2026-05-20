# Knowledge UI — files.md 프론트엔드 React 통합 설계

> **상태**: 설계 (v2 — 백엔드 스택 정확히 반영)
> **날짜**: 2026-05-20

---

## 1. 백엔드 스택 (이미 구현됨)

```
┌─────────────────────────────────────────────────────────────┐
│  knowledge_routes.rs (Axum HTTP 핸들러)                      │
│  29개 엔드포인트 — 요청 파싱 + JSON 직렬화만 담당              │
│                    │                                        │
│                    ▼                                        │
│  KnowledgeApi (kernel_handle/knowledge_api.rs)               │
│  Oxios의 12번째 API 도메인. 다음을 조합:                       │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ VirtualFs    │  │ BacklinkIndex│  │ MemoryManager    │  │
│  │ (oxios-md)   │  │ (oxios-md)   │  │ (oxios-kernel)   │  │
│  │              │  │              │  │                  │  │
│  │ 파일 CRUD    │  │ 양방향 링크   │  │ 시맨틱 검색      │  │
│  │ 샌드박스     │  │ 그래프 생성   │  │ 코파일럿 컨텍스트 │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ oxios-markdown (순수 라이브러리, oxi-sdk 의존 없음)     │  │
│  │                                                      │  │
│  │ VirtualFs    파일 트리 순회, 읽기/쓰기/삭제/이동/검색   │  │
│  │ BacklinkIndex 파일 내 링크 파싱, 역참조 인덱스          │  │
│  │ chat         Chat.md 파싱, 메시지 해시/삭제/이동/이름변경│  │
│  │ journal      일별 저널 엔트리, 이모지 추가               │  │
│  │ checklist    Later/Read/Watch/Shop 체크리스트 관리       │  │
│  │ habits       연간 습관 트래커, 이모지 시각화             │  │
│  │ stats        오늘 완료 보고서                            │  │
│  │ worker       야간 정리, 예약 작업 실행                   │  │
│  │ html         Markdown → HTML 변환                      │  │
│  │ i18n         키워드 → 이모지 매핑                       │  │
│  │ parser       유사도 비교, 헤딩/링크 추출                 │  │
│  │ merge        LCS 기반 병합                              │  │
│  │ sync         텍스트/미디어 동기화 엔진                    │  │
│  │ tokens       토큰 관리                                   │  │
│  │ schedule     예약 작업 관리                               │  │
│  │ plugins      세계 시계 등                                 │  │
│  │ tgtxt        텍스트/이미지/링크 추출                      │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 1.1 API 엔드포인트 ↔ KnowledgeApi ↔ oxios-markdown

| # | HTTP 엔드포인트 | KnowledgeApi 메서드 | oxios-markdown 함수 |
|---|----------------|--------------------|--------------------|
| 1 | `GET /api/knowledge/tree` | `note_tree(dir)` | `VirtualFs::files_and_dirs(dir)` |
| 2 | `GET /api/knowledge/file/{path}` | `note_read(path)` | `VirtualFs::read_path(path)` |
| 3 | `PUT /api/knowledge/file/{path}` | `note_write(path, content)` | `VirtualFs::write_path()` + `BacklinkIndex::index_file()` |
| 4 | `DELETE /api/knowledge/file/{path}` | `note_delete(path)` | `VirtualFs::delete_path()` + `BacklinkIndex::remove_file()` |
| 5 | `POST /api/knowledge/search` | `search(query, limit)` | `VirtualFs::search_files_by_name()` + `parser::similar()` + `MemoryManager::search()` |
| 6 | `GET /api/knowledge/backlinks` | `backlinks_for(path)` | `BacklinkIndex::backlinks_for()` |
| 7 | `GET /api/knowledge/graph` | `link_graph()` | `BacklinkIndex::link_graph()` |
| 8 | `POST /api/knowledge/copilot` | `copilot_chat(question, context_path)` | `search()` + `note_read()` + `EngineProvider::stream()` |
| 9 | `POST /api/knowledge/checklist/items` | `checklist_items(path)` | `checklist::checklist_items(content)` |
| 10 | `POST /api/knowledge/checklist/add` | `checklist_add(path, item, checked)` | `checklist::add_checklist_item(content, item, checked)` |
| 11 | `POST /api/knowledge/checklist/complete` | `checklist_complete(path, hash)` | `checklist::complete_checklist_item(content, hash)` |
| 12 | `POST /api/knowledge/checklist/remove` | `checklist_remove(path, item)` | `checklist::remove_checklist_item(content, item)` |
| 13 | `POST /api/knowledge/chat/append` | `chat_append(message)` | `parser::today_chat_header()` → `note_write(CHAT_FILENAME)` |
| 14 | `GET /api/knowledge/chat/messages` | `chat_messages()` | `chat::read_chat_msgs(content)` |
| 15 | `POST /api/knowledge/chat/delete` | `chat_delete(hash)` | `chat::delete_chat_msg(content, hash)` |
| 16 | `POST /api/knowledge/chat/move` | `chat_move_to(hash, target)` | `chat::move_from_chat(chat, hash, target)` |
| 17 | `POST /api/knowledge/journal/add` | `journal_add_record(record)` | `journal::add_record(&fs, record, tz)` |
| 18 | `POST /api/knowledge/journal/emoji` | `journal_add_emoji(emoji)` | `journal::add_emoji(&fs, emoji, tz)` |
| 19 | `GET /api/knowledge/journal/today` | `journal_today_path()` | `journal::today_journal_filename(tz)` |
| 20 | `GET /api/knowledge/habits` | `habits(year)` | `habits::habits(&fs, year)` |
| 21 | `GET /api/knowledge/habits/last-week` | `habits_last_week()` | `habits::last_week_habits(&fs, tz)` |
| 22 | `GET /api/knowledge/stats/today` | `today_report()` | `stats::today_report(&fs)` |
| 23 | `GET /api/knowledge/stats/done-today` | `done_today()` | `stats::done_today(&fs)` |
| 24 | `GET /api/knowledge/config` | `config()` | `VirtualFs::read_path("config.json")` → `serde_json` |
| 25 | `PUT /api/knowledge/config` | `set_config(config)` | `note_write("config.json", json)` |
| 26 | `POST /api/knowledge/worker/nightly` | `run_nightly_cleanup()` | `worker::remove_completed_items(&fs, config)` |
| 27 | `POST /api/knowledge/worker/scheduled` | `run_scheduled_tasks()` | `worker::move_due_tasks(&fs, config)` |
| 28 | `POST /api/knowledge/convert/html` | `markdown_to_html(md)` | `html::markdown_to_html(md)` |
| 29 | `GET /api/knowledge/emoji` | `auto_emoji(text)` | `i18n::emoji_for(text)` |

### 1.2 oxios-markdown 주요 상수

```rust
// types.rs
CHAT_FILENAME   = "Chat.md"
LATER_FILENAME  = "Later.md"
DONE_FILENAME   = "Done.md"
SHOP_FILENAME   = "Shop.md"
WATCH_FILENAME  = "Watch.md"
READ_FILENAME   = "Read.md"
DIR_USER_ROOT   = "/"
DIR_ARCHIVE     = "archive"
DIR_MEDIA       = "media"
DIR_JOURNAL     = "journal"
DIR_HABITS      = "habits"
```

---

## 2. files.md 원본 → React 매핑

### 2.1 기능 매핑

| # | files.md 기능 | 원본 JS | React 컴포넌트 | API # |
|---|--------------|---------|----------------|-------|
| **F1** | 파일 트리 사이드바 | `lib/sidebar.js` | `<KnowledgeSidebar>` + `<FileTree>` | #1 |
| **F2** | 마크다운 에디터 | `editor.js` + `lib/hypermd.*` | `<MarkdownEditor>` | #2, #3 |
| **F3** | 새 파일/폴더 | `app.js` → `newFile()` | `<KnowledgeSidebar>` 액션 | #3 |
| **F4** | 파일 삭제 | `app.js` → `removeCurrentFile()` | `<KnowledgeSidebar>` 컨텍스트 | #4 |
| **F5** | 빠른 검색 (⌘K) | `modals.js` → `SearchModal` | `<SearchModal>` | #5 |
| **F6** | 파일 이동 (⌘M) | `modals.js` → `MoveModal` | `<MoveModal>` | #3+#4 |
| **F7** | Quick Notes 채팅 | `chat.js` | `<KnowledgeChat>` | #13, #14, #15 |
| **F8** | 채팅 → 파일 이동 | `chat.js` → `to-file-btn` | `<ChatMessage>` + `<SearchModal>` | #16 |
| **F9** | 채팅 → 저널 이동 | `chat.js` → `to-journal-btn` | `<ChatMessage>` 액션 | #17 |
| **F10** | 채팅 → 체크리스트 이동 | `chat.js` → `to-checklist-btn` | `<ChatMessage>` 액션 | #10 |
| **F11** | 채팅 완료 토글 | `chat.js` → `complete-btn` | `<ChatMessage>` 체크박스 | #13 |
| **F12** | 저널 단축어 (`jj`) | `chat.js` → `addToJournal()` | `<KnowledgeChat>` 입력 | #17 |
| **F13** | 위키 링크 자동완성 | `lib/autocomplete-link.js` | `<MarkdownEditor>` 플러그인 | #1 |
| **F14** | 링크 클릭 → 열기 | `lib/click.js` | `<MarkdownEditor>` 플러그인 | 라우팅 |
| **F15** | 이미지 붙여넣기 | `editor.js` → paste 핸들러 | `<MarkdownEditor>` 플러그인 | #3 |
| **F16** | 스플릿 에디터 | `app.js` → `editor2-container` | `<SplitEditor>` | #2, #3 |
| **F17** | 사이드바 리사이즈 | `app.js` → resize handle | `<ResizeHandle>` | — |
| **F18** | 사이드바 토글 (⌘~) | `app.js` → `toggleSidebar()` | 레이아웃 상태 | — |
| **F19** | 백링크 | _(Oxios 확장)_ | `<Backlinks>` | #6 |
| **F20** | 링크 그래프 | _(Oxios 확장)_ | `<LinkGraph>` | #7 |
| **F21** | AI 코파일럿 | _(Oxios 확장)_ | `<Copilot>` | #8 |
| **F22** | 습관 트래커 | _(Oxios 확장)_ | `<Habits>` | #20, #21 |
| **F23** | 오늘 통계 | _(Oxios 확장)_ | `<TodayStats>` | #22, #23 |
| **F24** | 설정 | _(Oxios 확장)_ | `<KnowledgeSettings>` | #24, #25 |
| **F25** | 서식 단축키 | `editor.js` → keymap | `<MarkdownEditor>` | #3 |
| **F26** | 제목 강제 (`# `) | `editor.js` → change 핸들러 | `<MarkdownEditor>` | #3 |
| **F27** | MD→HTML 변환 | _(Oxios 확장)_ | `<MarkdownPreview>` | #28 |
| **F28** | 이모지 자동완성 | `lib/emoji.js` | `<MarkdownEditor>` 플러그인 | #29 |

### 2.2 원본에서 제외

| files.md 기능 | 제외 이유 |
|--------------|----------|
| File System Access API (OPFS) | 서버 API로 대체 |
| 서버 동기화 (`syncTextsWithServer`) | Oxios가 단일 소스 |
| PWA manifest / 토큰 인증 | oxios-web auth 통합 |
| Welcome files 시드 | 서버 초기화 담당 |
| `lib/similarity.js` | `oxios-markdown::parser::similar()`이 이미 포팅됨 |

---

## 3. 라우팅

```
/knowledge                      → KnowledgeLayout (자체 3-column 레이아웃)
/knowledge/chat                 → Chat 모드
/knowledge/file/*               → 파일 에디터
/knowledge/graph                → 링크 그래프
/knowledge/journal              → 저널 뷰
/knowledge/habits               → 습관 트래커
/knowledge/settings             → 설정
```

`/knowledge`는 대시보드 AppLayout과 **별개의** 독립 레이아웃.
files.md처럼 사이드바+에디터+정보패널 3-column 구조.

---

## 4. 컴포넌트 설계

### 4.1 레이아웃

```
┌──────────────────────────────────────────────────────────────────┐
│  KnowledgeLayout                                                 │
│                                                                  │
│  ┌──────────────┬──────────────────────────┬──────────────────┐ │
│  │ Knowledge    │                          │ InfoPanel        │ │
│  │ Sidebar      │  EditorPanel             │ (접히는 패널)     │ │
│  │              │                          │                  │ │
│  │ ┌──────────┐ │  ┌────────────────────┐  │ ┌──────────────┐ │ │
│  │ │ FileTree │ │  │ MarkdownEditor     │  │ │ Backlinks    │ │ │
│  │ │          │ │  │ (HyperMD)          │  │ │              │ │ │
│  │ │ brain/   │ │  │                    │  │ │ source_path  │ │ │
│  │ │  Rust.md │ │  │                    │  │ │ link_text    │ │ │
│  │ │  Go.md   │ │  │                    │  │ │              │ │ │
│  │ │ Chat.md  │ │  └────────────────────┘  │ │ LinkGraphMini│ │ │
│  │ │ Later.md │ │  ┌────────────────────┐  │ │              │ │ │
│  │ │ journal/ │ │  │ SplitEditor        │  │ └──────────────┘ │ │
│  │ └──────────┘ │  └────────────────────┘  │                  │ │
│  │              │  또는                     │                  │ │
│  │ [+New File]  │  ┌────────────────────┐  │                  │ │
│  │ [+New Dir]   │  │ KnowledgeChat      │  │                  │ │
│  │              │  │ (Quick Notes)      │  │                  │ │
│  │ ◀ Resize ▶   │  │                    │  │                  │ │
│  └──────────────┴──└────────────────────┘──┴──────────────────┘ │
│                                                                  │
│  <SearchModal />   <MoveModal />   (전역 오버레이)                │
└──────────────────────────────────────────────────────────────────┘
```

### 4.2 컴포넌트별 상세

#### `KnowledgeLayout`

**원본**: `app.js` 전체 구조, 키보드 이벤트, 레이아웃 제어

```
State (Zustand):
  mode: 'editor' | 'chat'
  currentFilePath: string | null
  history: string[]            // 파일 탐색 히스토리
  historyIndex: number
  sidebarOpen: boolean         // 기본 true
  sidebarWidth: number         // 기본 280px
  infoPanelOpen: boolean       // 기본 false
  splitEditorOpen: boolean     // 기본 false
  splitFilePath: string | null

단축키:
  ⌘K / ⌘P    → SearchModal.open()
  ⌘M          → MoveModal.open()
  ⌘N          → newFile()
  ⌘⇧N         → newFolder()
  ⌘D          → deleteCurrentFile()
  ⌘Enter      → openChat()
  ⌘⇧Enter     → toggleChatModal()
  ⌘~ / ⌘§     → toggleSidebar()
  ⌘W          → closeSplitEditor()
  Escape      → closeSplit / focus editor
```

---

#### `KnowledgeSidebar` + `FileTree`

**원본**: `lib/sidebar.js`, `lib/sidebar.css`

```
API: GET /api/knowledge/tree?dir=

기능:
  - GET /tree → 디렉토리 트리 렌더링
  - 디렉토리 확장/축소 (지연 로딩)
  - 파일 클릭 → openFile(path)
  - 우클릭 컨텍스트 메뉴: 이름 변경, 삭제, 새 파일
  - 시스템 디렉토리 숨김 (archive, .config)
  - 새로 생성된 파일 깜박임 (원본 sidebar-blink)
  - 드래그 리사이즈 핸들
  - [+New File] / [+New Dir] 버튼
```

---

#### `MarkdownEditor`

**원본**: `editor.js` + `lib/hypermd.*` + `lib/codemirror.*`

```
API:
  GET  /api/knowledge/file/{path}  → 초기 로드
  PUT  /api/knowledge/file/{path}  → 자동 저장 (debounce 1초)

구현: HyperMD (CodeMirror 5 기반) React 래퍼
  npm 패키지: hypermd, codemirror (@5)

포팅할 기능 (editor.js에서):
  1. HyperMD.fromTextArea() 초기화
     - mode: { name: 'hypermd', math: false }
     - lineNumbers: false
     - dragDrop: false
     - viewportMargin: 10

  2. `[` 입력 → 위키 링크 자동완성 (lib/autocomplete-link.js)
     - 파일 목록: GET /tree 결과를 캐시
     - 정렬: lastModified 순 (최근 수정 우선)
     - API #5 (search)를 사용해도 됨

  3. 링크 클릭 → 파일 열기 (lib/click.js)
     - .md 경로 → 라우팅으로 openFile()
     - 외부 URL → window.open()
     - `cmd:openDir`, `cmd:openChat` 액션 링크

  4. 이미지 북여넣기
     - clipboard image → PUT /api/knowledge/file/media/{filename}
     - `![](media/{filename})` 삽입

  5. 서식 단축키
     - ⌘B → **bold** 토글
     - ⌘I → *italic* 토글
     - ⌘Y → ✅ 삽입

  6. 제목 라인 강제 (첫 줄 항상 `# ` 유지)

  7. ⌘+클릭 → 인라인 코드 복사

  8. 이미지 인라인 프리뷰 (HyperMD 내장)

  9. 코드 블록 하이라이트 (js, python, go, php, shell)
     → lib/codemirror-*.js 언어 모드

자동 저장:
  onChange → debounce(1000ms) → PUT /api/knowledge/file/{path}
  + window blur 시 즉시 저장
```

---

#### `KnowledgeChat`

**원본**: `chat.js` 전체

```
API:
  GET  /api/knowledge/chat/messages   → 메시지 로드 (#14)
  POST /api/knowledge/chat/append     → 메시지 추가 (#13)
  POST /api/knowledge/chat/delete     → 메시지 삭제 (#15)
  POST /api/knowledge/chat/move       → 파일로 이동 (#16)
  POST /api/knowledge/journal/add     → 저널 이동 (#17)
  POST /api/knowledge/checklist/add   → 체크리스트 이동 (#10)

중요: 백엔드 KnowledgeApi.chat_messages()는
  oxios_markdown::read_chat_msgs()를 호출해서
  Chat.md 텍스트를 파싱한 Vec<String>을 반환.
  프론트엔드는 이 문자열 배열을 렌더링.

포팅할 기능 (chat.js에서):
  1. 메시지 입력 (Enter → 전송, Shift+Enter → 줄바꿈)
  2. 메시지 완료 토글 (체크박스 → - [x])
  3. 메시지 선택 (클릭, Shift+클릭, ⌘+클릭, 드래그 범위 선택)
  4. 선택 → 파일 이동 (SearchModal 열기)
  5. 선택 → 저널 이동
  6. 선택 → 체크리스트 이동 (Later/Read/Watch/Shop)
  7. 선택 → 아카이브 이동
  8. "text jj" 단축어 → 저널에 바로 추가
  9. 빈 상태 → "Free your head" 안내
  10. 이미지 북여넣기
  11. 자동 스크롤
  12. textarea 자동 높이 조절
```

---

#### `SearchModal`

**원본**: `modals.js` → `SearchModal` 클래스

```
API:
  POST /api/knowledge/search          → 파일 검색 (#5)
  GET  /api/knowledge/tree            → 디렉토리 목록 (#1)

모드:
  'navigate': 파일 열기 (⌘K 기본)
  'moveMessage': 채팅 메시지를 파일/디렉토리로 이동

포팅할 기능:
  1. 빈 입력 → 최근 파일 15개 (sort by lastModified)
  2. 타이핑 → POST /search
  3. "folder/" → 해당 폴더 파일만
  4. "folder file" → 폴더 내 검색
  5. Arrow Up/Down → 결과 탐색
  6. Enter → 선택 (이동 모드: 파일에 텍스트 추가)
  7. Escape → 닫기
  8. moveMessage 모드 → 디렉토리도 표시

참고: 원본의 similarity matching은 이미
  oxios_markdown::parser::similar()로 포팅됨.
  백엔드 search API가 이를 사용.
```

---

#### `MoveModal`

**원본**: `modals.js` → `MoveModal` 클래스

```
API:
  GET /api/knowledge/tree → 디렉토리 목록 (#1)

기능:
  1. 디렉토리 목록만 표시 (archive, _read_ 등은 하단)
  2. 타이핑 → prefix 필터링
  3. Enter → 파일 이동
     - GET 현재 내용 → PUT 새 경로 → DELETE 기존 경로
```

---

#### `InfoPanel` + `Backlinks` + `LinkGraph`

**Oxios 확장 기능** (원본에 없음)

```
API:
  GET /api/knowledge/backlinks?path=  → 백링크 (#6)
  GET /api/knowledge/graph            → 전체 그래프 (#7)

기능:
  - 현재 파일의 역참조 목록
  - 백링크 클릭 → 파일 열기
  - 미니 그래프 시각화 (nodes + edges)
```

---

## 5. 상태 관리

### 5.1 TanStack Query (서버 상태 — API 캐싱)

```typescript
// hooks/use-knowledge.ts

export function useKnowledgeTree(dir?: string)
// → GET /api/knowledge/tree?dir=

export function useKnowledgeFile(path: string)
// → GET /api/knowledge/file/{path}

export function useWriteFile()
// → PUT /api/knowledge/file/{path} (mutation)

export function useDeleteFile()
// → DELETE /api/knowledge/file/{path} (mutation)

export function useKnowledgeSearch(query: string, enabled: boolean)
// → POST /api/knowledge/search (수동 trigger)

export function useKnowledgeBacklinks(path: string)
// → GET /api/knowledge/backlinks?path=

export function useKnowledgeGraph()
// → GET /api/knowledge/graph

export function useChatMessages()
// → GET /api/knowledge/chat/messages

export function useChatAppend()
// → POST /api/knowledge/chat/append (mutation)

export function useChatDelete()
// → POST /api/knowledge/chat/delete (mutation)

export function useChatMove()
// → POST /api/knowledge/chat/move (mutation)

export function useJournalAdd()
// → POST /api/knowledge/journal/add (mutation)

export function useJournalToday()
// → GET /api/knowledge/journal/today

export function useChecklistAdd()
// → POST /api/knowledge/checklist/add (mutation)

export function useChecklistItems(path: string)
// → POST /api/knowledge/checklist/items

export function useHabits(year?: number)
// → GET /api/knowledge/habits

export function useStatsToday()
// → GET /api/knowledge/stats/today

export function useKnowledgeConfig()
// → GET /api/knowledge/config

export function useKnowledgeConfigUpdate()
// → PUT /api/knowledge/config (mutation)

export function useCopilot()
// → POST /api/knowledge/copilot (mutation)

export function useConvertHtml()
// → POST /api/knowledge/convert/html (mutation)

export function useAutoEmoji(text: string)
// → GET /api/knowledge/emoji?text=
```

### 5.2 Zustand (클라이언트 상태)

```typescript
// stores/knowledge.ts

interface KnowledgeState {
  mode: 'editor' | 'chat'
  currentFilePath: string | null
  history: string[]
  historyIndex: number

  sidebarOpen: boolean
  sidebarWidth: number
  infoPanelOpen: boolean

  splitEditorOpen: boolean
  splitFilePath: string | null

  // actions
  openFile: (path: string) => void
  openChat: () => void
  goBack: () => void
  goForward: () => void
  toggleSidebar: () => void
  setSidebarWidth: (w: number) => void
  toggleInfoPanel: () => void
  openSplit: (path: string) => void
  closeSplit: () => void
}
```

---

## 6. 파일 구조

```
channels/oxios-web/web/src/
├── routes/knowledge/
│   ├── knowledge.tsx                # KnowledgeLayout 래퍼
│   ├── chat.tsx                     # /knowledge/chat
│   ├── file.$path.tsx               # /knowledge/file/$path
│   ├── graph.tsx                    # /knowledge/graph
│   ├── journal.tsx                  # /knowledge/journal
│   ├── habits.tsx                   # /knowledge/habits
│   └── settings.tsx                 # /knowledge/settings
│
├── components/knowledge/
│   ├── knowledge-layout.tsx         # 3-column + 키보드 단축키
│   ├── knowledge-sidebar.tsx        # 사이드바 컨테이너
│   ├── file-tree.tsx                # 재귀적 파일 트리
│   ├── editor-panel.tsx             # 에디터 영역
│   ├── markdown-editor.tsx          # HyperMD React 래퍼
│   ├── split-editor.tsx             # 두 번째 에디터
│   ├── editor-toolbar.tsx           # 뒤로/앞으로/파일명
│   ├── knowledge-chat.tsx           # Quick Notes
│   ├── chat-message.tsx             # 메시지 + 액션 버튼
│   ├── chat-input.tsx               # 입력
│   ├── search-modal.tsx             # ⌘K
│   ├── move-modal.tsx               # ⌘M
│   ├── info-panel.tsx               # 우측 패널
│   ├── backlinks.tsx                # 백링크 목록
│   ├── link-graph.tsx               # 그래프
│   ├── copilot.tsx                  # AI 코파일럿
│   ├── resize-handle.tsx            # 리사이즈 드래그
│   ├── habits.tsx                   # 습관 트래커
│   ├── today-stats.tsx              # 통계
│   └── knowledge-settings.tsx       # 설정
│
├── hooks/
│   ├── use-knowledge.ts             # API hooks (위 5.1 전체)
│   └── use-knowledge-shortcuts.ts   # 키보드 단축키
│
├── stores/
│   └── knowledge.ts                 # Zustand store
│
├── lib/
│   └── hypermd-setup.ts             # HyperMD 초기화 설정
│
└── types/
    └── knowledge.ts                 # 타입 정의
```

---

## 7. 의존성

```json
{
  "dependencies": {
    "hypermd": "^3.0",
    "codemirror": "^5.65"
  }
}
```

HyperMD는 CodeMirror 5에 의존. Knowledge 라우트 내에서만 사용하므로
`React.lazy`로 지연 로드.

---

## 8. 구현 순서

### Phase 1: 뼈대 (MVP)
1. `KnowledgeLayout` 3-column 레이아웃 + Zustand store
2. `KnowledgeSidebar` + `FileTree` → `GET /tree`
3. `MarkdownEditor` (HyperMD 기본) → `GET/PUT /file/{path}`
4. 라우팅 `/knowledge/file/$path`
5. 파일 CRUD hooks

→ **파일 열기/편집/저장 가능**

### Phase 2: 채팅 + 검색
6. `KnowledgeChat` → `/chat/*` API
7. `SearchModal` (⌘K) → `POST /search`
8. `MoveModal` (⌘M)
9. Chat → 파일/저널/체크리스트 이동

→ **files.md 핵심 워크플로우 완성**

### Phase 3: 에디터 고급
10. `[` 위키 링크 자동완성
11. 링크 클릭 → 파일 열기
12. 이미지 북여넣기
13. 서식 단축키 (⌘B/I/Y)
14. `SplitEditor` (⌘W)
15. 사이드바 리사이즈

→ **files.md 에디터 경험 완성**

### Phase 4: Oxios 확장
16. `InfoPanel` + `Backlinks` + `LinkGraph`
17. `Copilot`
18. `Habits` + `TodayStats`
19. `KnowledgeSettings`

→ **files.md 이상의 기능**
