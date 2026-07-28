# Knowledge × Search Integration — Design Document

**Date:** 2026-07-28
**Status:** Draft
**Phase:** 3 (following Search & Browse Panel Phases 1-2)
**Prerequisite:** docs/superpowers/specs/2026-07-28-search-browse-panel-design.md

## Overview

Integrate the Search & Browse Panel (PortalPanel) with the Knowledge Base (Knowledge UI at `/knowledge`). Four integration points:

1. **Search Panel Knowledge tab** — Search/browse knowledge notes from the chat sidebar
2. **Save to Knowledge** — Save web search results and page content as knowledge notes
3. **Knowledge SearchModal Web tab** — Web search from the ⌘K modal in the Knowledge page
4. **Copilot Web integration** — Copilot sidebar shows web results alongside knowledge notes

All backend APIs already exist (29 Knowledge endpoints, 2 Search/Browse endpoints). This phase is **frontend only**.

## Architecture

```
┌─ Chat Page ──────────────────────┬─ Knowledge Page ───────────────┐
│ PortalPanel                      │ KnowledgeSidebar               │
│ ┌─ SearchView ───────────────┐   │ ┌─ SearchModal (⌘K) ──────┐   │
│ │ [Web] [Knowledge] ← tab    │   │ │ [Files] [Web] ← tab    │   │
│ │                            │   │ │                        │   │
│ │ Web tab:                   │   │ │ Files tab (기존):      │   │
│ │  └─ [Save to Knowledge]    │──┼→│  └─ openFile()          │   │
│ │                            │   │ │ Web tab (신규):        │   │
│ │ Knowledge tab:             │   │ │  └─ /api/search        │   │
│ │  ├─ /api/knowledge/search  │   │ │  └─ [Open in Panel]    │──┼→┐
│ │  └─ 미리보기 (read only)   │   │ └────────────────────────┘   │ │
│ └────────────────────────────┘   │ ┌─ Copilot ───────────────┐   │ │
│                                  │ │ [🔍 Include web] ☑️    │   │ │
│                                  │ │ 질문 → /api/copilot     │   │ │
│                                  │ │      + /api/search      │   │ │
│                                  │ │ Knowledge notes + Web  │   │ │
│                                  │ └────────────────────────┘   │ │
└──────────────────────────────────┴───────────────────────────┘   │
                                                                   │
    PortalPanel ← KnowledgeView ───────────────────────────────────┘
    (type: 'knowledge', path: 'notes/foo.md')
```

### PortalView 확장

`web/src/stores/portal.ts`:

```typescript
| {
    type: 'knowledge'
    path: string
    title?: string
  }
```

### 새/변경 컴포넌트 의존 관계

```
search-view.tsx
├── WebTabContent (기존 search/browse 로직)
│   └── 각 카드: [Save to Knowledge] 버튼
├── KnowledgeTabContent (신규: knowledge-browser.tsx)
│   ├── 검색 입력 → /api/knowledge/search
│   ├── 노트 목록
│   └── 미리보기 (MarkdownMessage)
└── TabBar: [Web] [Knowledge]

knowledge-browser.tsx (신규)
├── 검색 입력 (controlled)
├── 노트 목록 (검색 결과)
├── 노트 선택 → MarkdownMessage 미리보기
├── [Open in Knowledge] → router navigate
└── 상태: empty/loading/results/preview/error

search-modal.tsx (수정)
├── TabBar: [Files] [Web]
├── FilesTab (기존)
└── WebTab (신규)
    ├── 검색 입력 → /api/search (debounce 300ms)
    ├── 결과 카드 (favicon + title + snippet)
    └── [Open in Search Panel] → pushView({ type: 'search' })

copilot.tsx (수정)
├── [Include web results] ☑️ toggle
├── 질문 전송 시:
│   ├── include_web=false → 기존처럼 copilot만 호출
│   └── include_web=true → copilot + /api/search 병렬 호출
└── 응답을 Knowledge notes + Web results 섹션으로 분할 표시
```

## 1. Search Panel Knowledge Tab

### 파일: `web/src/components/portal/views/search-view.tsx` (수정)

탭 바 추가 (`[Web] [Knowledge]`).

```tsx
const [activeTab, setActiveTab] = useState<'web' | 'knowledge'>('web')

// 탭 바 렌더링 (기존 검색 입력 아래)
// - agent-driven 결과 수신 시 → Web 탭 자동 선택
// - 사용자가 Knowledge 탭 수동 선택 시 유지
```

### 파일: `web/src/components/portal/views/knowledge-browser.tsx` (신규)

**상태:**

| State | Rendering |
|---|---|
| **Empty** | 책 아이콘 + "Search your knowledge base" |
| **Loading** | 3줄 skeleton shimmer |
| **Results** | 노트 제목 + snippet + 경로 (최대 50개) |
| **Selected** | MarkdownMessage 미리보기 (80vh max, scroll) |
| **Error** | 에러 메시지 + Retry 버튼 |
| **Empty results** | "No notes matching '{query}'" |

### Props

```typescript
interface KnowledgeBrowserProps {
  /** 초기에 로드할 파일 경로 (선택). 설정 시 자동으로 해당 파일 로드 + 미리보기 표시. */
  initialPath?: string
}
```

**initialPath 동작:**
- `initialPath`가 설정되면 → 해당 파일을 자동으로 읽어서 미리보기 표시
- `initialPath`가 없으면 → 빈 상태에서 검색 대기
- KnowledgeView (PortalPanel)에서 사용 시: `path`를 `initialPath`로 전달

```
검색 입력 (debounce 300ms)
  → POST /api/knowledge/search { query, limit: 50 }
  → 결과 목록 표시
  → 항목 클릭
    → GET /api/knowledge/file/{path}
    → MarkdownMessage 렌더링
  → [Open in Knowledge] 버튼
    → router.navigate('/knowledge/file/$path')
```

### 파일: `web/src/stores/search-panel.ts` (수정)

```typescript
interface SearchPanelState {
  // 기존 fields...
  activeTab: 'web' | 'knowledge'

  // Knowledge tab state
  knowledgeQuery: string
  knowledgeResults: KnowledgeSearchResult[]
  knowledgeLoading: boolean
  knowledgeError: string | null
  selectedKnowledgePath: string | null
  selectedKnowledgeContent: string | null
  selectedKnowledgeLoading: boolean

  // Save to Knowledge modal
  saveModalOpen: boolean
  saveUrl: string
  saveTitle: string
  saveContent: string
  savePath: string
  saveLoading: boolean
  saveError: string | null

  // Actions
  searchKnowledge: (query: string) => Promise<void>
  selectKnowledge: (path: string) => Promise<void>
  openInKnowledge: (path: string) => void
  openSaveModal: (url: string, title: string, content: string) => void
  closeSaveModal: () => void
  saveToKnowledge: () => Promise<void>
  setActiveTab: (tab: 'web' | 'knowledge') => void
}
```

## 2. Save to Knowledge

### 흐름

```
Web tab 결과 카드 또는 Browse 확장 카드
  → [Save to Knowledge] 버튼
  → SaveToKnowledgeModal 열림
    ├─ Title: 검색 결과 title (편집 가능)
    ├─ Path: web-clippings/{domain}/{date}-{slug}.md (편집 가능)
    ├─ Content 미리보기 (읽기 전용)
→ [Save] 버튼
    → PUT /api/knowledge/file/{path} (with frontmatter)
    → 모달 auto-close
    → Toast "Saved to Knowledge"
    → Toast action: [View in Knowledge] 버튼 (/knowledge/file/$path로 이동)
```

### 저장 포맷

```markdown
# {title}
> **Source:** [{url}]({url})
> **Saved:** {YYYY-MM-DD}

{content}
```

### 자동 경로 생성

```
web-clippings/{domain}/{YYYY-MM-DD}-{slugify(title)}.md

예: web-clippings/rust-lang.org/2026-07-28-announcing-rust-1-85.md
```

### 변경 파일

- `web/src/stores/search-panel.ts` — save modal state + actions
- `web/src/components/portal/views/search-view.tsx` — Save 버튼 + modal

## 3. Knowledge SearchModal Web Tab

### 파일: `web/src/components/knowledge/search-modal.tsx` (수정)

```typescript
// Tab bar 추가
const [searchTab, setSearchTab] = useState<'files' | 'web'>('files')

// Web tab:
//   검색 입력 (300ms debounce) → POST /api/search
//   결과 카드: favicon(Google favicon service) + title + snippet
//   [Open in Search Panel] → usePortalStore.pushView({ type: 'search', query })
//
// Files tab (기존 유지):
//   기존 knowledge 검색 로직
```

**주의사항:**
- SearchModal은 `web/src/components/knowledge/`에 있음. 여기서 Portal store import 필요
- SearchModal이 `/knowledge` 페이지에서만 열리므로 cross-module import는 문제 없음

## 4. Copilot Web Integration

### 파일: `web/src/components/knowledge/copilot.tsx` (수정)

```typescript
const [includeWeb, setIncludeWeb] = useState(false)

const handleAsk = useCallback(async () => {
  if (!question.trim()) return

  // 항상 copilot 호출
  const copilotPromise = copilot.mutateAsync({
    question: question.trim(),
    contextPath: currentFilePath ?? undefined,
  })

  if (includeWeb) {
    // 병렬로 웹 검색
    const webPromise = api.post('/api/search', { query: question.trim(), limit: 5 })
    const [copilotResult, webResult] = await Promise.all([copilotPromise, webPromise])

    setResponse({
      copilotResponse: copilotResult,
      webResults: webResult.results,
    })
  } else {
    const copilotResult = await copilotPromise
    setResponse({
      copilotResponse: copilotResult,
      webResults: [],
    })
  }
}, [question, currentFilePath, copilot, includeWeb])
```

### 응답 표시

```
┌─ Knowledge Notes ──────────────┐
│ Copilot 응답 내용               │
│ Referenced notes: [...links]   │
└────────────────────────────────┘
┌─ Web Results ──────────────────┐  ← (includeWeb이 true일 때만 표시)
│ Result 1: title + url          │
│ Result 2: title + url          │
│ [Search in Panel] 버튼         │
└────────────────────────────────┘
```

### 변경 파일
- `web/src/components/knowledge/copilot.tsx` — 토글 + 병렬 요청 + 분할 표시
- `web/src/hooks/use-knowledge.ts` — (변경 없음, api 직접 호출)

## 5. PortalPanel KnowledgeView

### 파일: `web/src/stores/portal.ts` (수정)

```typescript
export type PortalView =
  | { type: 'artifact'; artifactId: string }
  | { type: 'preview'; messageId: string }
  | { type: 'search'; query?: string; messageId?: string }
  | { type: 'knowledge'; path: string; title?: string }
```

### 파일: `web/src/components/portal/portal-panel.tsx` (수정)

```typescript
// viewTitle:
'knowledge' → t('knowledge.notes')

// ViewBody:
case 'knowledge': {
  const path = (view as Extract<PortalView, { type: 'knowledge' }>).path
  return <KnowledgeBrowser initialPath={path} />
}
```

**용도:** 채팅에서 Knowledge 링크 클릭 시 PortalPanel에서 바로 열림 (전체 페이지 이동 없이).

## 6. i18n Translation Keys

### en.json

```json
{
  "search": {
    "panel": {
      "placeholder": "Search the web…",
      "agentResults": "Agent search results",
      "empty": "Search the web or wait for agent results",
      "results": "results",
      "saveToKnowledge": "Save to Knowledge",
      "savedToKnowledge": "Saved to Knowledge",
      "viewInKnowledge": "View in Knowledge",
      "knowledgeTab": "Knowledge",
      "webTab": "Web",
      "knowledgePlaceholder": "Search your knowledge base…",
      "knowledgeEmpty": "Search your knowledge base",
      "openInKnowledge": "Open in Knowledge",
      "includeWeb": "Include web results"
    }
  }
}
```

## 7. Files Changed

### New files

| File | Purpose |
|---|---|
| `web/src/components/portal/views/knowledge-browser.tsx` | Knowledge 노트 검색 + 읽기 전용 브라우저 (~150 lines) |

### Modified files

| File | Change |
|---|---|
| `web/src/stores/portal.ts` | Add `type: 'knowledge'` to PortalView union |
| `web/src/components/portal/portal-panel.tsx` | Add KnowledgeView case to dispatcher |
| `web/src/stores/search-panel.ts` | Add activeTab, knowledgeResults, save modal state/actions |
| `web/src/components/portal/views/search-view.tsx` | Tab bar, Save button, modal, Knowledge tab delegation |
| `web/src/components/knowledge/search-modal.tsx` | Web tab + tab bar |
| `web/src/components/knowledge/copilot.tsx` | "Include web" toggle + 병렬 표시 |
| `web/src/i18n/locales/en.json` | Search panel + integration keys |
| `web/src/i18n/locales/ko.json` | Same, Korean translations |

### Unchanged

- All backend files (Knowledge API, Search/Browse API)
- Knowledge filetree, editor, info panel, etc.
- Chat store (auto-open logic already implemented in Phase 2)

## 8. Implementation Order

| # | Task | Depends on |
|---|------|-----------|
| 1 | KnowledgeBrowser component (신규) | — |
| 2 | SearchView tab bar + Knowledge tab + Save to Knowledge | 1 |
| 3 | SearchPanel store 확장 (tab state, knowledge state, save state) | 1 |
| 4 | i18n 번역 키 | 2 |
| 5 | SearchModal Web 탭 (⌘K) | — |
| 6 | Copilot Web 토글 | — |
| 7 | PortalPanel KnowledgeView 등록 | 1 |
| 8 | 전체 검증 (tsc + test) | 1-7 |

Tasks 1, 5, 6, 7 are independent (parallelizable).

## 9. Open Questions / Edge Cases

- **SearchModal import cycle:** SearchModal이 `/knowledge` 페이지에 있고 Portal store를 import. Portal store는 knowledge를 import하지 않으므로 circular dep는 없음.
- **KnowledgeBrowser 재사용:** KnowledgeBrowser는 PortalPanel 안에서와 `/knowledge` 페이지에서 모두 사용될 수 있음. `initialPath` prop으로 초기 파일 지정.
- **Save 충돌:** 같은 URL을 여러 번 저장하면? → 파일명에 timestamp 추가로 방지.
- **Copilot + Web 검색 지연:** copilot 응답이 느릴 수 있음. web 결과가 먼저 도착하면 바로 표시, copilot 결과는 도착 시 추가.
- **Large browse content:** 1MB 넘는 페이지는 save 전에 경고 또는 truncation.
- **KnowledgeView navigation:** type: 'knowledge' view에서 다른 노트로 이동 시 push 또는 replace? → push (stack에 쌓임).
