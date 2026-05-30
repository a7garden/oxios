# Web UI Phase 2: Chat · Workspace · Search/Filter · MCP 설계

> **날짜:** 2026-05-30
> **범위:** Chat 개편, Workspace CRUD, 전역 검색/필터, MCP 관리 페이지
> **목표:** 사용자-에이전트 상호작용과 파일 관리의 핵심 UX 완성

---

## 0. PI 세션 컨텍스트

이 문서는 독립 PI 세션에서 그대로 사용합니다. 아래 정보만으로 구현에 필요한 모든 컨텍스트를 제공합니다.

### 프로젝트 스택

| 영역 | 기술 |
|------|------|
| Framework | React 19 + TypeScript 6 |
| 라우팅 | TanStack Router (파일 기반, `autoCodeSplitting`)
| 데이터 | TanStack Query (`useQuery` / `useMutation` + 캐시 무효화)
| 상태 | Zustand 5 (persist 미들웨어로 localStorage)
| 스타일 | Tailwind CSS v4 + shadcn/ui 패턴 (`components/ui/`)
| 차트 | Recharts 3 |
| i18n | i18next + react-i18next (EN/KO, 618키)
| 에디터 | HyperMD (CM5, Knowledge 전용) |
| 빌드 | Vite 8 + Bun |
| 백엔드 | Axum (Rust), `surface/oxios-web/src/routes/` |

### 디렉토리 구조

```
surface/oxios-web/
├── src/                          # Rust 백엔드
│   ├── plugin.rs                 # AppState, 서버 시작
│   ├── server.rs                 # AppState 정의 (kernel, channel, config, ...)
│   ├── middleware.rs              # Auth + Rate Limit
│   ├── channel.rs                # WebChannel (Gateway 브릿지)
│   ├── routes/
│   │   ├── mod.rs                # build_routes() — 모든 라우트 등록
│   │   ├── system.rs             # /api/agents, /api/status, /api/config
│   │   ├── workspace.rs          # /api/seeds, /api/skills, /api/memory, /api/workspace
│   │   ├── chat.rs               # /api/chat, /api/chat/stream (WS)
│   │   ├── knowledge_routes.rs   # /api/knowledge/* (30개)
│   │   ├── engine_routes.rs      # /api/engine/*
│   │   ├── events.rs             # /api/events (SSE), /api/sessions, /api/approvals
│   │   ├── infra.rs              # /api/metrics, /api/scheduler, /api/audit
│   │   │                         # /api/mcp/* — 핸들러 존재하나 라우트 미등록!
│   │   ├── budget_routes.rs      # /api/budget
│   │   ├── cron_jobs.rs          # /api/cron-jobs
│   │   ├── space_routes.rs       # /api/spaces
│   │   ├── git_routes.rs         # /api/git
│   │   ├── marketplace.rs        # /api/marketplace
│   │   ├── audit_routes.rs       # /api/audit
│   │   ├── resource_routes.rs    # /api/resources
│   │   └── agent_groups.rs       # /api/agent-groups
│   └── persona_routes.rs         # /api/personas (routes/ 밖에 위치)
│
└── web/                         # React 프론트엔드
    ├── src/
    │   ├── main.tsx
    │   ├── routes/               # TanStack Router 파일 기반 라우트
    │   ├── components/
    │   │   ├── ui/               # shadcn/ui 기본 (button, card, tabs, badge...)
    │   │   ├── shared/           # 공유 (data-table, loading, error-state, empty-state)
    │   │   ├── layout/           # app-layout, sidebar, header
    │   │   ├── knowledge/        # Knowledge 앱 전용
    │   │   └── engine/           # Engine 설정
    │   ├── hooks/                # TanStack Query 훅
    │   ├── stores/               # Zustand 스토어 (auth, chat, events, knowledge, notifications, sidebar, theme)
    │   ├── types/                # TypeScript 타입
    │   ├── lib/                  # api-client, utils, sse-client, ws-client
    │   └── i18n/                 # 번역
    └── package.json
```

### 코딩 패턴 (반드시 따를 것)

**1. 라우트:** `createFileRoute('/path')({ component: ... })`. 로딩/에러/빈 상태는 항상 처리.

**2. API 훅:** `hooks/use-*.ts` 파일에 TanStack Query 훅. `useQuery`로 읽기, `useMutation`으로 쓰기. mutation 성공 시 `queryClient.invalidateQueries()`.

**3. API 클라이언트:** `import { api } from '@/lib/api-client'` — `api.get<T>(path, params?)`, `api.post<T>(path, body?)`, `api.put<T>(path, body?, raw?)`, `api.delete<T>(path)`.

**4. i18n:** `useTranslation()` → `t('key')`. 키는 `public/locales/en/common.json`과 `ko/common.json`에 추가. 현재 618키.

**5. shadcn/ui:** `@/components/ui/tabs`, `badge`, `card`, `button`, `progress`, `dialog`, `select`, `tooltip`, `input`, `textarea`, `separator`, `skeleton`, `scroll-area`, `switch`, `dropdown-menu`.

### 참고: 이 Phase에서 수정/확장할 기존 파일

반드시 구현 전에 먼저 읽을 것:
- `routes/chat.tsx` — Chat 페이지 전체 (405줄)
- `stores/chat.ts` — Chat Zustand 스토어 (356줄). WebSocket, chunk handling, session management
- `types/index.ts` — `StreamChunk`, `ChatMessage`, `ChatResponse` 타입
- `routes/workspace/index.tsx` — 현재 Workspace 페이지 (파일 트리만)
- `components/shared/data-table.tsx` — 현재 DataTable (검색/필터 없음)
- `surface/oxios-web/src/routes/chat.rs` — 백엔드 WS 핸들러
- `surface/oxios-web/src/routes/workspace.rs` — 백엔드 Workspace + Memory + Seeds + Skills API
- `surface/oxios-web/src/routes/infra.rs` — MCP 핸들러 (존재하나 라우트 미등록, `#[allow(dead_code)]`)
- `components/layout/sidebar.tsx` — 사이드바 내비게이션

### MCP 백엔드 (이미 구현됨, 라우트만 등록하면 됨)

`infra.rs`에 이미 4개 핸들러가 구현되어 있습니다:
- `handle_mcp_servers_list` → `GET /api/mcp/servers`
- `handle_mcp_server_register` → `POST /api/mcp/servers`
- `handle_mcp_tools_list` → `GET /api/mcp/tools`
- `handle_mcp_tool_call` → `POST /api/mcp/tools`

`routes/mod.rs::build_routes()`에 `.route()`만 추가하면 활성화됩니다.

### 병렬 Worktree 전략

```
main 브랜치에서 출발:
  git worktree add ../oxios-p2 -b feature/web-ui-phase2
  cd ../oxios-p2
  bun install   # CM6 의존성 설치 필요
```

**이 Phase(P2)의 충돌 파일:**

| 파일 | 수정 내용 | 충돌 Phase |
|------|----------|----------|
| `src/routes/mod.rs` | MCP 라우트 등록 | P1, P3 도 라우트 추가 |
| `src/routes/workspace.rs` | File CRUD 엔드포인트 추가 | P1 도 Memory 엔드포인트 추가 |
| `web/src/components/layout/sidebar.tsx` | Monitor 그룹에 MCP 항목 추가 | P3 도 같은 그룹에 A2A 추가 |
| `web/src/types/index.ts` | ChatMessage/StreamChunk 확장 | P1 도 타입 보강 |
| `web/src/components/shared/data-table.tsx` | 재작성 (검색/필터/정렬) | P3 에서 사용만, 충돌 없음 |

**해결:** P2는 다른 함수/컴포넌트를 추가하므로 P1과 충돌해도 쉽게 병합. P1 머지 후 `main` 리베이스.

### 사이드바 변경 (이 Phase)

`components/layout/sidebar.tsx`의 `navGroups` 배열에서 `common.monitor` 그룹에 MCP Servers 항목 추가:

```tsx
// Monitor 그룹에 추가:
{ labelKey: 'common.mcpServers', href: '/mcp', icon: <Zap className="h-4 w-4" /> },
```

참고: Phase 3이 이어서 A2A Monitor를 같은 그룹에, Agent Groups를 Agents 그룹에 추가합니다.

---

## 1. 개요

Phase 2는 사용자가 Oxios와 직접 상호작용하는 4개 영역을 다룹니다:

```
Chat ←→ Workspace ←→ Search/Filter ←→ MCP
(대화)     (파일)      (탐색)         (도구)
```

### 기존 대비 변화

| 영역 | 현재 | 목표 |
|------|------|------|
| Chat | 토큰 스트리밍만, 툴콜 무시 (85%) | 툴콜 인라인, 에이전트 상태, 단계 표시 (95%) |
| Workspace | 파일 트리 탐색만 (70%) | 파일 뷰어/에디터, 생성/삭제/업로드 (95%) |
| 검색/필터 | DataTable에 없음 (0%) | 모든 리스트 페이지에 통합 검색/필터/정렬 (90%) |
| MCP | 백엔드 구현됨, 라우트/프론트엔드 없음 (0%) | 서버/도구 관리, 호출 테스트 (90%) |

---

## 2. Chat 개편

### 2.1 현재 문제점

1. **툴콜 무시**: `StreamChunk`에 `tool_call`/`tool_result` 타입이 정의되어 있으나, 백엔드가 이를 전송하지 않고 프론트엔드도 처리하지 않음
2. **배치 스트리밍**: 백엔드가 전체 응답을 하나의 `token` 청크로 전송. 진정한 토큰 단위 스트리밍 아님
3. **완료 메타데이터 미표시**: `done` 청크의 `phase`, `evaluation_passed`, `seed_id`, `duration_ms`가 무시됨
4. **메시지 ID 없음**: React key로 배열 인덱스 사용 — 불안정
5. **에러 처리**: 에러를 텍스트에 인라인 표시, 재시도 버튼 없음

### 2.2 백엔드 변경

#### Chat 백엔드 WebSocket 개선 (`routes/chat.rs`)

**현재 흐름:**
```
Client → WS message → Gateway send_and_wait() → 전체 응답 대기 → 1개 token 청크 + done 청크
```

**목표 흐름:**
```
Client → WS message → Gateway → SSE/스트림 구독 → 토큰/툴콜 이벤트를 개별 청크로 전달
```

**구현 방안:**

Gateway의 `send_and_wait()`는 전체 응답을 기다리는 패턴이므로, 완전한 토큰 단위 스트리밍을 위해선 Gateway 레벨 변경이 필요합니다. **현실적 대안**으로 두 단계로 나눕니다:

**Step A (Phase 2): 툴콜 메타데이터 전송**
- `send_and_wait()` 결과에서 도구 호출 정보를 추출하여 `tool_call`/`tool_result` 청크 전송
- AgentRuntime이 이미 `trajectory_steps`를 수집하므로, 이를 세션에 저장하고 WS done 청크에 포함

```rust
// routes/chat.rs - WS handler 수정
// done 청크에 tool_calls 배열 추가
let done_chunk = json!({
    "type": "done",
    "session_id": ...,
    "space_id": ...,
    "phase": ...,
    "evaluation_passed": ...,
    "seed_id": ...,
    "duration_ms": ...,
    "tool_calls": [
        {
            "tool_name": "exec",
            "input": "cargo test",
            "output": "142 tests passed",
            "duration_ms": 3500
        }
    ]
});
```

**Step B (향후): 진정한 토큰 스트리밍**
- Gateway에 `send_and_stream()` 메서드 추가
- `BroadcastStream<OutgoingMessage>`을 WS로 직접 전달
- Phase 2에서는 Step A만 구현

#### Chat 세션에 툴콜 데이터 저장

**신규 엔드포인트:**

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/sessions/{id}/tool-calls` | 세션의 툴콜 타임라인 |

**응답:**
```json
{
  "session_id": "uuid",
  "tool_calls": [
    {
      "index": 0,
      "tool_name": "exec",
      "input": "cargo test --workspace",
      "output": "running 142 tests... all passed",
      "duration_ms": 3500,
      "timestamp": "2026-05-30T10:00:15Z"
    }
  ]
}
```

### 2.3 프론트엔드 변경

#### Chat Store (`stores/chat.ts`) 확장

```typescript
// 새 메시지 타입
interface ChatMessage {
  id: string              // ← 신규: crypto.randomUUID()
  role: 'user' | 'assistant' | 'system' | 'tool'
  content: string
  timestamp?: string
  // 툴콜 전용 (role === 'tool')
  toolName?: string
  toolArgs?: Record<string, unknown>
  toolResult?: unknown
  toolDurationMs?: number
  // 완료 메타데이터 (assistant 마지막 메시지)
  metadata?: {
    phase?: string
    evaluation_passed?: boolean
    seed_id?: string
    duration_ms?: number
    tool_calls?: ToolCallSummary[]
  }
}

interface ToolCallSummary {
  tool_name: string
  input: string
  output: string
  duration_ms: number
}
```

**handleChunk 확장:**
```typescript
handleChunk(chunk) {
  if (chunk.type === 'tool_call') {
    // tool_call 청크 → tool 메시지로 인라인 삽입
  } else if (chunk.type === 'tool_result') {
    // 기존 tool 메시지 업데이트
  } else if (chunk.type === 'done') {
    // phase, evaluation_passed, duration_ms, tool_calls[] 처리
    // 마지막 assistant 메시지에 metadata 추가
  }
}
```

#### Chat UI 컴포넌트

| 컴포넌트 | 파일 | 설명 |
|----------|------|------|
| `ChatMessageBubble` | `components/chat/message-bubble.tsx` | 사용자/어시스턴트 메시지 렌더링 + 타임스탬프 |
| `ToolCallCard` | `components/chat/tool-call-card.tsx` | 툴콜 인라인 카드 (접기/펼치기, 도구명, 입출력) |
| `ChatMetadata` | `components/chat/chat-metadata.tsx` | 완료 메타: Phase, 평가 결과, 소요시간 |
| `ChatInput` | `components/chat/chat-input.tsx` | 입력창 + 전송 + 취소 버튼 |
| `ConnectionStatus` | `components/chat/connection-status.tsx` | WS 연결 상태 인디케이터 |

**ToolCallCard 레이아웃:**
```
┌─────────────────────────────────────────────────┐
│ 🔧 exec                                    3.5s │
├─────────────────────────────────────────────────┤
│ Input:  cargo test --workspace                  │
│─────────────────────────────────────────────────│
│ Output: running 142 tests... all passed         │
│         [펼치기/접기]                            │
└─────────────────────────────────────────────────┘
```

**ChatMetadata (완료 후 마지막 메시지 아래):**
```
┌─────────────────────────────────────────────────┐
│ ✅ Evaluate · Passed · 2m 30s · Seed #abc →    │
└─────────────────────────────────────────────────┘
```

#### 채팅 페이지 개선

| 개선 | 설명 |
|------|------|
| 메시지 타임스탬프 표시 | 각 메시지 상대 시간 (방금, 2분 전) |
| 에러 토스트 | 인라인 에러 대신 Sonner 토스트 + 재시도 버튼 |
| 연결 상태 배지 | "연결됨" / "연결 중..." / "연결 끊김" 상태 배지 |
| 취소 버튼 | 스트리밍 중 응답 취소 (WS close/reconnect) |
| 자동 스크롤 개선 | 사용자가 수동 스크롤 시 자동 스크롤 일시정지 |

### 2.4 신규/수정 파일

| 파일 | 변경 | 설명 |
|------|------|------|
| `stores/chat.ts` | 수정 | 메시지 ID, tool_call 처리, metadata 확장 |
| `routes/chat.tsx` | 수정 | ToolCallCard, ChatMetadata, 연결 상태 통합 |
| `components/chat/tool-call-card.tsx` | 신규 | 툴콜 인라인 카드 |
| `components/chat/chat-metadata.tsx` | 신규 | 완료 메타데이터 표시 |
| `components/chat/message-bubble.tsx` | 신규 | 메시지 버블 (타임스탬프, 마크다운) |
| `components/chat/chat-input.tsx` | 신규 | 입력 컴포넌트 분리 |
| `components/chat/connection-status.tsx` | 신규 | WS 연결 상태 |
| `types/index.ts` | 수정 | ChatMessage, StreamChunk 확장 |

---

## 3. Workspace 개편

### 3.1 백엔드 변경

기존 API는 충분합니다. 파일 생성/삭제 엔드포인트 추가:

| Method | Path | 설명 |
|--------|------|------|
| `POST` | `/api/workspace/file/*path` | 빈 파일 생성 (body: `{ "is_dir": false }`) |
| `POST` | `/api/workspace/dir/*path` | 디렉토리 생성 |
| `DELETE` | `/api/workspace/file/*path` | 파일/디렉토리 삭제 |

**`DELETE` 구현 노트:**
- 파일은 직접 삭제
- 디렉토리는 비어있을 때만 삭제 (`is_empty` 체크)
- 경로 순회 방지 (기존 `sanitize_path` 재사용)

### 3.2 프론트엔드 변경

#### 레이아웃: 트리 + 뷰어 분할

```
┌─────────────────────────────────────────────────────────────┐
│  Workspace                              [Upload] [+New ▼]  │
├────────────────────┬────────────────────────────────────────┤
│ 📁 src/            │  📄 src/main.rs                        │
│   📁 kernel/       │  ┌────────────────────────────────┐   │
│     📄 mod.rs      │  │ 1  fn main() {                 │   │
│   📄 main.rs  ←    │  │ 2      println!("hello");       │   │
│   📄 lib.rs        │  │ 3  }                            │   │
│ 📁 config/         │  │                                 │   │
│ 📄 Cargo.toml      │  │                                 │   │
│                    │  └────────────────────────────────┘   │
│                    │  [Save] [Delete]                       │
├────────────────────┴────────────────────────────────────────┤
│  Breadcrumb: src / main.rs                                   │
└─────────────────────────────────────────────────────────────┘
```

#### 컴포넌트

| 컴포넌트 | 파일 | 설명 |
|----------|------|------|
| `FileTree` | `components/workspace/file-tree.tsx` | 기존 트리 개선 (확장형 아이콘, 우클릭 메뉴) |
| `FileViewer` | `components/workspace/file-viewer.tsx` | 파일 내용 뷰어 (CodeMirror 읽기 전용) |
| `FileEditor` | `components/workspace/file-editor.tsx` | 파일 편집기 (CodeMirror 편집 모드) |
| `FileBreadcrumb` | `components/workspace/file-breadcrumb.tsx` | 경로 브레드크럼 |
| `FileActions` | `components/workspace/file-actions.tsx` | 새 파일/폴더 생성, 업로드, 삭제 |
| `CreateFileDialog` | `components/workspace/create-file-dialog.tsx` | 파일/폴더 생성 다이얼로그 |
| `UploadDropZone` | `components/workspace/upload-drop-zone.tsx` | 드래그앤드롭 업로드 |

#### 뷰어 모드

| 파일 타입 | 렌더링 | 편집 가능 |
|----------|--------|----------|
| `.rs`, `.ts`, `.tsx`, `.js`, `.py`, `.go`, `.toml`, `.json`, `.yaml` | CodeMirror (구문 강조) | ✅ |
| `.md` | 마크다운 미리보기 (전환 가능) | ✅ |
| `.txt`, `.log`, `.env` | 일반 텍스트 | ✅ |
| 이미지 (`.png`, `.jpg`, `.svg`) | `<img>` 태그 | ❌ |
| 바이너리 | "바이너리 파일, 다운로드만 가능" | ❌ |

#### 편집 흐름

1. 파일 클릭 → `FileViewer` (읽기 전용)
2. "편집" 버튼 또는 더블클릭 → `FileEditor` (편집 모드)
3. 내용 수정 → `PUT /api/workspace/file/*path`로 저장
4. `Ctrl+S` / `⌘S` 단축키로 저장

### 3.3 신규/수정 파일

| 파일 | 변경 | 설명 |
|------|------|------|
| `routes/workspace/index.tsx` | 재작성 | 분할 레이아웃, 뷰어/에디터 통합 |
| `hooks/use-workspace.ts` | 신규 | 파일 CRUD, 업로드 훅 |
| `types/workspace.ts` | 신규 | FileContent, CreateFileRequest 등 |
| `components/workspace/file-tree.tsx` | 신규 | 개선된 파일 트리 |
| `components/workspace/file-viewer.tsx` | 신규 | CodeMirror 파일 뷰어 |
| `components/workspace/file-editor.tsx` | 신규 | CodeMirror 파일 에디터 |
| `components/workspace/file-breadcrumb.tsx` | 신규 | 경로 브레드크럼 |
| `components/workspace/create-file-dialog.tsx` | 신규 | 파일/폴더 생성 다이얼로그 |
| `components/workspace/upload-drop-zone.tsx` | 신규 | 드래그앤드롭 업로드 |

### 3.4 의존성

CodeMirror 6를 파일 뷰어/에디터로 사용. Knowledge의 HyperMD(CM5)와는 별개:
- `@codemirror/view`, `@codemirror/state`, `@codemirror/language`
- 개별 언어 팩: `@codemirror/lang-rust`, `@codemirror/lang-json`, `@codemirror/lang-markdown`, `@codemirror/lang-python`, `@codemirror/lang-yaml`, `@codemirror/lang-toml`
- 현재 `package.json`에 CodeMirror 5(hypermd용)만 있음. CM6를 별도로 설치

---

## 4. 전역 검색/필터/정렬

### 4.1 DataTable 개선 (`components/shared/data-table.tsx`)

**기존:** 정적 테이블, 정렬/필터/페이지네이션 없음

**목표:** 범용 DataTable에 검색, 필터, 정렬, 페이지네이션 내장

#### DataTableProps 확장

```typescript
interface DataTableProps<T> {
  data: T[]
  columns: ColumnDef<T>[]
  // 신규
  searchable?: boolean                    // 검색 바 표시
  searchPlaceholder?: string              // 검색 placeholder
  searchKeys?: (keyof T)[]                // 검색 대상 필드
  filterable?: {                          // 컬럼 필터
    key: keyof T
    options: { label: string; value: string }[]
  }[]
  sortable?: (keyof T)[]                  // 정렬 가능 컬럼
  pagination?: { pageSize: number }       // 페이지네이션
  onRowClick?: (row: T) => void
  emptyMessage?: string
  loading?: boolean
}
```

#### 컴포넌트 구조

```
┌─────────────────────────────────────────────────────────────┐
│ [🔍 검색어 입력...]     [상태 ▼ All]    [정렬 ▼ 최신순]      │
├─────┬──────────┬──────────┬──────────┬──────────────────────┤
│     │ Name     │ Status   │ Seed     │ Created              │
├─────┼──────────┼──────────┼──────────┼──────────────────────┤
│ ... │ ...      │ ...      │ ...      │ ...                  │
├─────┴──────────┴──────────┴──────────┴──────────────────────┤
│                    ◀ 1 2 3 ... 5 ▶    25개 중 1-10          │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 적용 대상

| 페이지 | 검색 | 필터 | 정렬 | 페이지네이션 |
|--------|:---:|:---:|:---:|:---:|
| Agents | 이름, ID | 상태 | 생성일, 이름 | 20 |
| Seeds | Goal | 세대 | 생성일 | 20 |
| Sessions | ID | Space | 업데이트일 | 20 |
| Audit | 액션, 에이전트 | 허용/거부 | 타임스탬프 | 20 |
| Memory | 내용 | 티어, 타입 | 중요도, 생성일 | 20 |
| Cron Jobs | 이름 | 활성화 | 다음 실행 | 20 |

**구현 방식:** 모든 필터링/정렬은 클라이언트 사이드. 데이터가 충분히 작음 (수백 개 이하).

### 4.3 신규/수정 파일

| 파일 | 변경 | 설명 |
|------|------|------|
| `components/shared/data-table.tsx` | 재작성 | 검색/필터/정렬/페이지네이션 내장 |
| `components/shared/search-bar.tsx` | 신규 | 재사용 검색 바 컴포넌트 |
| `components/shared/column-filter.tsx` | 신규 | 컬럼 필터 드롭다운 |
| `components/shared/pagination.tsx` | 신규 | 페이지네이션 컴포넌트 |
| `routes/agents/index.tsx` | 수정 | 검색/필터 활성화 |
| `routes/seeds/index.tsx` | 수정 | 검색 활성화 |
| `routes/sessions/index.tsx` | 수정 | 검색/필터 활성화 |
| `routes/security.tsx` | 수정 | 감사 트레일 필터/정렬 |
| `routes/events.tsx` | 수정 | 이벤트 타입 필터 |

---

## 5. MCP 관리 페이지

### 5.1 백엔드 변경

**기존 핸들러를 `routes/mod.rs`에 등록:**

```rust
// routes/mod.rs::build_routes()에 추가
.route("/api/mcp/servers", get(handle_mcp_servers_list).post(handle_mcp_server_register))
.route("/api/mcp/tools", get(handle_mcp_tools_list).post(handle_mcp_tool_call))
```

**신규 엔드포인트:**

| Method | Path | 설명 |
|--------|------|------|
| `DELETE` | `/api/mcp/servers/{name}` | MCP 서버 삭제/연결 해제 |
| `POST` | `/api/mcp/servers/{name}/toggle` | MCP 서버 활성/비활성 토글 |
| `POST` | `/api/mcp/servers/{name}/refresh` | 도구 목록 새로고침 |

**서버 삭제 구현:**
- `McpBridge`에 `remove_server()` 메서드 추가 필요
- 해당 서버의 stdio 프로세스 종료
- 캐시에서 해당 서버의 도구 제거

### 5.2 프론트엔드

#### 레이아웃: 3탭

```
┌─────────────────────────────────────────────────────────────┐
│  MCP Servers                              [+ Add Server]     │
├──────────┬──────────┬──────────────────────────────────────┤
│ Servers  │ Tools    │ Test                                  │
├──────────┴──────────┴──────────────────────────────────────┤
│  [Tab Content]                                              │
└─────────────────────────────────────────────────────────────┘
```

#### Tab 1: Servers

```
┌──────────────────────────────────────────────────────────────┐
│  Server Name      Command          Status      Actions       │
│  ─────────────────────────────────────────────────────────── │
│  🟢 filesystem    npx @model...   Connected   [Refresh][✕]  │
│  🟢 github        npx @model...   Connected   [Refresh][✕]  │
│  🔴 database      npx @model...   Error       [Retry] [✕]   │
│  ⚪ custom        /usr/bin/my...  Disabled    [Enable][✕]   │
└──────────────────────────────────────────────────────────────┘
```

**서버 등록 다이얼로그:**
```
┌──────────────────────────────────────────┐
│ Add MCP Server                           │
│──────────────────────────────────────────│
│ Name:    [                            ]  │
│ Command: [npx @modelcontextprotocol/]    │
│ Args:    [server-filesystem /path   ]  │
│ Env:     KEY=value (한 줄에 하나씩)       │
│                                          │
│        [Cancel]  [Add Server]            │
└──────────────────────────────────────────┘
```

#### Tab 2: Tools

```
┌──────────────────────────────────────────────────────────────┐
│  [🔍 도구 검색...]                                          │
│──────────────────────────────────────────────────────────────│
│  📦 filesystem                                               │
│    ├── read_file        파일 내용 읽기                       │
│    ├── write_file       파일 내용 쓰기                       │
│    ├── list_directory   디렉토리 목록                        │
│    └── search_files     파일 검색                            │
│  📦 github                                                   │
│    ├── create_issue     이슈 생성                            │
│    └── list_prs         PR 목록                              │
└──────────────────────────────────────────────────────────────┘
```

도구 클릭 → 상세 패널: 이름, 설명, 인자 스키마(JSON Schema), 소속 서버

#### Tab 3: Test

```
┌──────────────────────────────────────────────────────────────┐
│  Tool Call Tester                                            │
│──────────────────────────────────────────────────────────────│
│  Server:  [filesystem ▼]                                     │
│  Tool:    [read_file ▼]                                      │
│  Arguments (JSON):                                           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ { "path": "/tmp/test.txt" }                          │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  [Execute]                                                   │
│                                                              │
│  Result:                                                     │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ "Hello, World!"                                      │   │
│  └──────────────────────────────────────────────────────┘   │
│  Duration: 120ms                                             │
└──────────────────────────────────────────────────────────────┘
```

### 5.3 사이드바 내비게이션 변경

```
common.monitor
  ├── ...
  ├── Git
  ├── MCP Servers        ← 신규 (Zap 아이콘)
```

### 5.4 신규 파일

| 파일 | 설명 |
|------|------|
| `routes/mcp.tsx` | MCP 관리 페이지 (3탭) |
| `components/mcp/server-list.tsx` | 서버 리스트 테이블 |
| `components/mcp/server-card.tsx` | 서버 카드 (상태, 명령어, 액션) |
| `components/mcp/add-server-dialog.tsx` | 서버 등록 다이얼로그 |
| `components/mcp/tool-list.tsx` | 도구 리스트 (서버별 그룹) |
| `components/mcp/tool-detail.tsx` | 도구 상세 (스키마) |
| `components/mcp/tool-tester.tsx` | 도구 호출 테스터 |
| `hooks/use-mcp.ts` | MCP API 훅 |
| `types/mcp.ts` | McpServer, McpTool, McpToolCallResult |

---

## 6. 데이터 흐름

### 6.1 Chat 데이터 흐름 (개선)

```
User → Input → WS send → Gateway → AgentRuntime (trajectory_steps 수집)
                                              ↓
                                         send_and_wait() 완료
                                              ↓
Client ← WS chunks:
  - token: 응답 텍스트
  - done: { session_id, phase, evaluation_passed, seed_id,
            duration_ms, tool_calls[] }
  - error: 에러 메시지

Chat Store:
  handleChunk:
    token → assistant 메시지에 텍스트 추가
    done → metadata 저장, tool_calls[] → 개별 tool 메시지로 삽입
    error → 에러 토스트
```

### 6.2 Workspace 데이터 흐름

```
FileTree → GET /api/workspace/tree?dir= → TreeEntry[]
  └── 클릭 → GET /api/workspace/file/{path} → string (파일 내용)
       └── 편집 → PUT /api/workspace/file/{path} (raw body)
       └── 생성 → POST /api/workspace/file/{path} (빈 파일)
       └── 삭제 → DELETE /api/workspace/file/{path}
       └── 업로드 → PUT /api/workspace/file/{path} (multipart → raw)
```

---

## 7. 구현 순서

### Step 1: 공유 컴포넌트 (검색/필터/정렬)
1. `search-bar.tsx`, `column-filter.tsx`, `pagination.tsx` 작성
2. `data-table.tsx` 재작성 (검색/필터/정렬/페이지네이션 통합)
3. Agents, Seeds, Sessions, Security 페이지에 적용

### Step 2: Chat 백엔드
1. `routes/chat.rs` 수정 — done 청크에 `tool_calls[]` 포함
2. `GET /api/sessions/{id}/tool-calls` 엔드포인트
3. trajectory_steps를 세션에 저장하는 로직 (Phase 1과 공유)

### Step 3: Chat 프론트엔드
1. ChatMessage 타입 확장 (id, tool, metadata)
2. `stores/chat.ts` 수정 (handleChunk 확장)
3. `tool-call-card.tsx`, `chat-metadata.tsx` 컴포넌트
4. `routes/chat.tsx` 수정 (새 컴포넌트 통합)

### Step 4: Workspace 백엔드
1. `POST /api/workspace/file/*path`, `POST /api/workspace/dir/*path`
2. `DELETE /api/workspace/file/*path`

### Step 5: Workspace 프론트엔드
1. CodeMirror 6 패키지 설치
2. `file-viewer.tsx`, `file-editor.tsx`
3. `file-breadcrumb.tsx`, `create-file-dialog.tsx`
4. `upload-drop-zone.tsx`
5. `routes/workspace/index.tsx` 재작성

### Step 6: MCP 백엔드
1. `routes/mod.rs`에 MCP 라우트 등록
2. `DELETE /api/mcp/servers/{name}`, 토글, 리프레시 추가
3. McpBridge에 `remove_server()` 메서드

### Step 7: MCP 프론트엔드
1. `types/mcp.ts`, `hooks/use-mcp.ts`
2. `server-list.tsx`, `add-server-dialog.tsx`
3. `tool-list.tsx`, `tool-detail.tsx`
4. `tool-tester.tsx`
5. `routes/mcp.tsx`
6. 사이드바에 MCP 항목 추가

---

## 8. 의존성

| 의존 | 설명 | 설치 필요 |
|------|------|----------|
| CodeMirror 6 (`@codemirror/view`, `@codemirror/state`, 언어 팩) | Workspace 파일 뷰어/에디터 | ✅ `bun add` |
| 기존 패키지 | Recharts, TanStack, Zustand, i18next | ❌ |

---

## 9. i18n 키 추가분

- Chat: toolCall, phase, evaluation, duration, connected, disconnected, cancel
- Workspace: viewer, editor, save, createFile, createFolder, upload, delete, binaryFile
- DataTable: search, filter, sort, ascending, descending, previous, next, page, of
- MCP: servers, tools, test, addServer, serverName, command, args, env, connected, disconnected, execute, result, duration
