# Search & Browse Panel — Design Document

**Date:** 2026-07-28
**Status:** Draft
**Approach:** B — Dedicated side panel (PortalPanel extension)

## Overview

Web search results and page-content browsing in the oxios Web UI are currently
fragmented: `web_search` renders as pretty cards (snippet only), `browse`
renders as a raw `<pre>` block (truncated to 1000 chars), and there is no
visual connection between a search query and its browsed pages. The user cannot
interactively discover, read, or save web pages from within the UI without
directing the agent.

This design adds a **dedicated Search & Browse Panel** to the chat page by
extending the existing PortalPanel (stack-based right-side panel).

## Architecture

```
Chat Page
┌────────────────────────────┬──────────────────────────────┐
│ Chat Stream                │ PortalPanel (기존)            │
│                            │ ┌──────────────────────────┐ │
│ [web_search tool card] ────┼→│ SearchView (신규)        │ │
│   snippet only             │ │  ┌────────────────────┐  │ │
│                            │ │  │ 🔍 검색 입력란     │  │ │
│ [browse tool card] ────────┼→│  │ 결과 카드 목록     │  │ │
│   raw pre block            │ │  │  ├─ favicon+title   │  │ │
│                            │ │  │  ├─ snippet         │  │ │
│                            │ │  │  └─ [Read] → 본문  │  │ │
│                            │ │  │      MarkdownMessage│  │ │
│                            │ │  └────────────────────┘  │ │
│                            │ └──────────────────────────┘ │
└────────────────────────────┴──────────────────────────────┘
```

**Two data flows:**

1. **Agent-driven (automatic):** During a chat session, when the agent calls
   `web_search` or `browse`, the SearchView opens automatically and displays the
   results extracted from the chat store. If the agent browses multiple URLs,
   each browse result attaches under the corresponding search result card.

2. **User-driven (manual):** The panel has its own search input. The user types
   a query → `POST /api/search` → results appear as cards. Clicking "Read page"
   → `POST /api/browse` → full markdown expands inline within the card.

## 1. PortalView Extension

**File:** `web/src/stores/portal.ts`

Add to the `PortalView` union:

```typescript
| {
    type: 'search'
    /** Search query (auto-set on agent-driven, entered in panel on manual). */
    query?: string
    /** Chat message ID that triggered this view (agent-driven only). */
    messageId?: string
  }
```

The `PortalPanel` view dispatcher (`portal-panel.tsx`) adds a `'search'` case
that renders `SearchView`.

## 2. SearchView Component

**File:** `web/src/components/portal/views/search-view.tsx` (new)

### Structure

```
SearchView
├── SearchInput
│   └── <input /> + Search icon
│       Enter → onSearch(query) → /api/search
├── [Source indicator]
│   "Agent searched: `rust async runtime`" (agent-driven)
│   or blank (manual)
├── ResultList
│   └── SearchResultCard × N
│       ├── favicon (via Google favicon service: sz=32)
│       ├── title (link, opens in new tab)
│       ├── domain badge (truncated URL host)
│       ├── snippet (2–3 lines, clamp)
│       ├── status badges: "Browsed" / "Cached" (if applicable)
│       └── [Read page] toggle
│           └── when expanded:
│               ├── loading skeleton
│               ├─── MarkdownMessage (full page content)
│               └── error: [Retry] button
└── StatusBar
    └── "N results · Xms"
```

### Data Sources

| Source | When | Display |
|---|---|---|
| `chatStore.messages[messageId].blocks` | Agent-driven (`messageId` set) | Derive web_search & browse results from tool blocks |
| `searchPanelStore.manualResults[]` | Manual search | Results from `/api/search` |

### States

| State | Rendering |
|---|---|
| **Empty** | "Search the web or wait for agent results" placeholder |
| **Loading** | Skeleton (3 cards with shimmer) |
| **Agent results** | Pre-populated from chat store, no loading state |
| **Manual results** | Fresh from `/api/search` |
| **Browsing (loading)** | Card expands, skeleton + pulsing indicator |
| **Browsed (content)** | Markdown content via MarkdownMessage |
| **Browse error** | Error message + [Retry] button |
| **Empty results** | "No results found" + suggestion |

## 3. SearchPanel Store

**File:** `web/src/stores/search-panel.ts` (new)

```typescript
interface SearchPanelState {
  // Manual search state
  manualQuery: string
  manualResults: SearchResult[]
  manualLoading: boolean
  manualError: string | null

  // Browse cache (URL → content, survives panel close)
  browseCache: Record<string, BrowseResult>
  browseLoading: Record<string, boolean>
  browseError: Record<string, string | null>

  // UI state
  expandedUrls: Set<string>

  // Actions
  search: (query: string) => Promise<void>
  browse: (url: string) => Promise<void>
  toggleExpand: (url: string) => void
  saveToKnowledge: (url: string, title: string, content: string) => Promise<void>
  reset: () => void
}
```

### Why a separate store?

The portal store owns stack navigation. The search panel store owns search
results, browse cache, and expansion state. Separation keeps both stores
focused. The portal store just stores `type: 'search'` and delegates rendering
to SearchView, which reads its own data source.

## 4. New Backend Endpoints

### `POST /api/search`

**File:** `src/api/routes/search.rs` (new)

```
Request:  { "query": "...", "engines": ["ddg","wiki"], "limit": 10 }
Response: {
  "results": [{ "title": "...", "url": "...", "snippet": "...", "engine": "ddg" }],
  "elapsed_ms": 1234
}
```

Implementation calls `oxibrowser::search::dispatch()` (same function the
`web_search` tool uses). The engine parameter defaults to the same
`--engines ddg,wiki` used by the tool. No agent loop involved.

**Dependency:** `oxibrowser` must be added as a direct dependency of the
binary crate (`Cargo.toml`). It is already a transitive dep via `oxi-sdk →
oxi-agent → oxibrowser`, so this adds no new compiled code.

**Security:** This is read-only, no side effects. The endpoint is open to the
current session (authenticated via the existing session token).

### `POST /api/browse`

**File:** `src/api/routes/search.rs` (same file)

```
Request:  { "url": "https://...", "format": "markdown" }
Response: {
  "url": "...",
  "title": "...",
  "markdown": "...",
  "status": 200,
  "elapsed_ms": 456
}
```

Implementation calls `state.kernel.browser.engine().await?.new_tab()?.goto(url)`
via the already-wired `BrowserApi` (KernelHandle). The `format` field is
reserved for future `"text"` or `"html"` variants; initially only `"markdown"`
is supported.

**Security:** Same as search — read-only, session-scoped. The `robots.txt`
obeisance is configurable via `BrowseConfig` (already wired).

### Route Registration

**File:** `src/api/routes/mod.rs`

Add two lines:
```rust
.route("/api/search", post(handle_search))
.route("/api/browse", post(handle_browse))
```

## 5. Chat ↔ Panel Auto-Open

### Tool Chunk Detection

**File:** `web/src/stores/chat.ts`, inside `handleChunk` for `tool.result` /
`tool.start`:

```typescript
if (
  chunk.type === 'tool.result' &&
  (chunk.tool_name === 'web_search' ||
   chunk.tool_name === 'browse')
) {
  // Auto-open the search panel if not already open
  const portalState = usePortalStore.getState()
  const stack = portalState.stack
  const top = stack[stack.length - 1]
  if (!top || top.type !== 'search') {
    portalState.pushView({ type: 'search', messageId: assistantMsgId })
  }
  // Update the agent-driven results (the SearchView will re-derive
  // from the chat store when messageId is set — no explicit update needed)
}
```

### Manual Open

A 🔍 (search) icon button in the chat header or input area:

```typescript
onClick: () => usePortalStore.getState().pushView({ type: 'search' })
```

## 6. Browse Tool Render Improvements

These are companion changes that fix existing broken rendering in chat.

### Register browse tools

**File:** `web/src/components/chat/tool-renders/index.ts`

Add registrations:
```typescript
registerToolRender('browse', BrowseRender)
registerToolRender('browse_extract', BrowseRender)
registerToolRender('browse_session', BrowseRender)
registerToolRender('browse_script', BrowseRender)
```

### New BrowseRender component

**File:** `web/src/components/chat/tool-renders/Browse.tsx` (new)

- Replaces the existing `WebFetchRender` (which renders `<pre>` blocks).
- Header: Globe icon → URL link (open in new tab) + HTTP status badge + title.
- Body: Markdown content rendered via `MarkdownMessage` component (full-length,
  not truncated to 1000 chars).
- Footer: "Open in Panel" button → opens PortalPanel SearchView scoped to this
  page's URL.

### Update WebSearchRender

**File:** `web/src/components/chat/tool-renders/WebSearch.tsx`

- Each search result card gets a small "Open in Panel" link/button.
- Clicking it calls `usePortalStore.getState().pushView({ type: 'search', messageId })`.

## 7. Visual Design

| Element | Detail |
|---|---|
| **Panel width** | Resizable via existing PortalPanel handle. Default 480px. |
| **Result card** | Rounded border, bg-muted/30, hover:bg-muted/50, transition |
| **Favicon** | `https://www.google.com/s2/favicons?domain=X&sz=32` |
| **Snippet clamp** | 3 lines via `line-clamp-3` |
| **Expand animation** | Smooth height (Tailwind transition-all) |
| **Browse loading** | Skeleton block (shimmer) inside expanded card |
| **Browse content** | `MarkdownMessage` with `prose-sm`, capped at 80vh scroll |
| **Empty state** | Centered icon + "Search the web or wait for agent results" |
| **Search input** | Full-width, rounded, placeholder "Search the web..." |
| **Badges** | "Browsed" (green), "Cached" (amber) pill inside result cards |

## 8. Phase 2: Knowledge UI Integration (future)

Not implemented in this phase. Design sketch:

- **Knowledge SearchModal** gets a "Web" tab alongside the existing file search.
- Tab switches between `/api/knowledge/search` (local notes) and `/api/search`
  (web).
- Clicking a web search result opens the PortalPanel SearchView with that
  query pre-populated.
- "Save to Knowledge" button in SearchView → `POST /api/knowledge/file` with
  the markdown content as a new note.

## 9. Files Changed / Created

### New files

| File | Purpose |
|---|---|
| `web/src/components/portal/views/search-view.tsx` | Search panel view component (~200 lines) |
| `web/src/stores/search-panel.ts` | Search panel state (~100 lines) |
| `web/src/components/chat/tool-renders/Browse.tsx` | Browse tool markdown renderer (~60 lines) |
| `src/api/routes/search.rs` | POST /api/search & /api/browse handlers (~80 lines) |

### Modified files

| File | Change |
|---|---|
| `web/src/stores/portal.ts` | Add `search` variant to PortalView union |
| `web/src/components/portal/portal-panel.tsx` | Add `search` case to view dispatcher |
| `web/src/components/chat/tool-renders/index.ts` | Register browse tool renders |
| `web/src/components/chat/tool-renders/WebSearch.tsx` | Add "Open in Panel" button |
| `web/src/stores/chat.ts` | Auto-open portal on web_search/browse tool results |
| `src/api/routes/mod.rs` | Register /api/search, /api/browse routes |
| `web/src/i18n/locales/en.json` | New translation keys |
| `web/src/i18n/locales/ko.json` | New translation keys |

## 10. Open Questions / Edge Cases

- **Multiple simultaneous searches**: The panel shows only the latest search
  results. Previous searches are in the chat history but not in the panel.
  Future: push a new SearchView for each distinct query (stack navigation
  handles this).

- **PortalPanel already open (artifact/filePreview)**: Auto-open for search
  pushes onto the stack. The user can pop back to the previous view.

- **Agent browses a URL not in search results**: The browse result still appears
  in the panel as a standalone entry (no parent search card). The panel groups
  browse results under the nearest preceding search query.

- **Rate limiting**: `/api/search` and `/api/browse` are per-session. The
  oxibrowser engine has its own rate limiting via DuckDuckGo's limits. No
  additional rate limiting needed for this MVP.

- **Session expiry**: If the websocket session expires, manual search/browse
  still works (independent REST endpoints).
