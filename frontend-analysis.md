# Oxios Frontend Architecture Analysis

> Analyzed 14 core files across routes, components, stores, hooks, lib, i18n, and types.
> Date: 2026-05-26

---

## 1. File-by-File Summary

### 1.1 `routes/__root.tsx` — Root Layout

- **Purpose**: TanStack Router root route with `QueryClientProvider` wrapper.
- **Pattern**: `createRootRouteWithContext<RouterContext>()` — passes `QueryClient` via route context.
- **Observations**: Minimal and clean. Single responsibility: provide React Query to the tree. No error boundary or suspense at this level.

### 1.2 `components/layout/app-layout.tsx` — Main Layout

- **Purpose**: Unified shell that switches between Dashboard mode (sidebar + header + outlet) and Knowledge mode (Knowledge sidebar + info panel).
- **Pattern**: Reads `pathname` from `useRouterState()` to determine mode; conditionally renders completely different sidebar/content structures.
- **State**: Zustand stores (`useSidebarStore`, `useKnowledgeStore`), hooks (`useKnowledgeShortcuts`, `useGlobalEvents`, `useApprovalWatcher`), and `useEventStore` for SSE bootstrap.
- **Observations**:
  - Mode detection via pathname prefix (`/knowledge`) is simple but fragile — any future route starting with `/knowledge` would trigger Knowledge mode.
  - SSE bootstrap uses `React.useState(() => { connectEvents() })` which is a clever init-once pattern (the initializer runs only on first render).
  - Mobile overlays use `role="dialog"` and `onKeyDown` for Escape — good a11y, but no focus trap.

### 1.3 `components/layout/sidebar.tsx` — Navigation

- **Purpose**: Main nav sidebar with grouped items, dynamic "Approvals" badge, theme toggle, settings link.
- **Pattern**: Declarative `navGroups` array with `labelKey`, `href`, `icon`. Dynamic items injected via `useDynamicItems()` hook.
- **State**: `useSidebarStore` (collapsed toggle), `useThemeStore` (theme cycling), TanStack Query for pending approvals count.
- **Observations**:
  - Approvals badge refetches every 10s — reasonable polling.
  - Active state logic: `currentPath === item.href || (item.href !== '/' && currentPath.startsWith(item.href))` — standard prefix matching, works well.
  - Theme cycles dark → light → system (3-way toggle), which is clean UX.
  - Uses `Tooltip` wrapper when collapsed — good desktop UX.

### 1.4 `components/layout/header.tsx` — Header

- **Purpose**: Top bar with mobile hamburger, Knowledge breadcrumb, notification bell, language selector, brand.
- **Pattern**: Conditionally renders `KnowledgeBreadcrumb` (reads knowledge store) vs. dashboard separator.
- **Observations**:
  - KnowledgeBreadcrumb is extracted into its own component specifically to scope `useKnowledgeStore` subscriptions — **intentional optimization** noted in comment.
  - Uses `fallback` in `t()` calls: `t('common.toggleSidebar', 'Toggle sidebar')` — inconsistent with other places that use `t()` without fallback.

### 1.5 `hooks/use-knowledge.ts` — API Hooks (29 hooks)

- **Purpose**: TanStack Query hooks for all Knowledge API operations.
- **Pattern**: Consistent `useQuery` for reads, `useMutation` with `qc.invalidateQueries()` for writes. Each mutation invalidates relevant query keys on success.
- **Observations**:
  - **Consistent structure**: Every query has a typed response (`<KnowledgeTreeEntry[]>`, `<KnowledgeBacklink[]>`, etc.) — strong type safety.
  - **Cache invalidation is granular**: `useWriteFile` invalidates `tree`, `file`, and `backlinks` — correctly broad.
  - **`enabled: !!path`** guard pattern used consistently for path-dependent queries — prevents unnecessary fetches.
  - **No error handling in hooks**: Errors are delegated to TanStack Query's default behavior (no `onError` callbacks). Consumers must handle `error` from the returned query object.
  - **29 hooks** in one file is large but manageable — each is small and uniform.

### 1.6 `stores/knowledge.ts` — Knowledge State

- **Purpose**: Zustand store for Knowledge UI state (mode, current file, history, layout).
- **Pattern**: Plain `create()` without middleware. Some values initialized from `localStorage` (sidebar width, sidebar open).
- **Observations**:
  - **No persistence middleware**: Only `sidebarOpen` and `sidebarWidth` are manually saved to `localStorage` via action code. Other state (current file, history) resets on page reload.
  - **History management**: Manual forward/back with `history` array + `historyIndex` — correctly trims forward history on new navigation.
  - **No middleware**: Unlike `chat.ts`, this store doesn't use `persist()` — inconsistency.

### 1.7 `stores/chat.ts` — Chat State

- **Purpose**: Zustand store with `persist` middleware for chat. Manages WebSocket lifecycle, message streaming, session management.
- **Pattern**: `persist()` middleware with `partialize` to only save `activeSessionId` and `activeSpaceId`. Auto-reconnects WS on rehydration.
- **Observations**:
  - **WebSocket singleton** at module level (`wsInstance`) — not ideal for SSR but fine for SPA.
  - **`chunkHandler`** is a module-level variable — potential stale closure issue, but the store re-assigns it on each `connect()`.
  - **`loadSession`** uses raw `fetch` instead of `api` client — **inconsistency**. Could use `api.get<SessionDetail>(...)` for consistency and automatic auth header injection.
  - **`_sendQueue`** dedup logic uses `includes(content)` — could fail if same message is sent twice legitimately.
  - **Error handling in streaming**: Appends `[Error: ...]` to the last assistant message — visible to user but not structured.

### 1.8 `routes/chat.tsx` — Chat Page

- **Purpose**: Full chat UI with Space/Session sidebar, message list, input area.
- **Pattern**: `createFileRoute('/chat')` with component. Uses `useChatStore` for all chat state. `SpaceSessionSidebar` is a local component.
- **Observations**:
  - **`SpaceSessionSidebar` fetches spaces/sessions using raw `fetch`** — doesn't use `api` client. Missing auth header (`Authorization: Bearer`) that the `api` client provides.
  - **`groupSessionsByDate`** helper is clean but dates are hardcoded to `'ko-KR'` locale — should use i18n locale.
  - **Message keys**: Uses `${msg.role}-${i}` array index keys with biome-ignore comment — messages lack unique IDs from backend.
  - **Empty state** properly handles both "not connected" and "connected, no messages" states.
  - **Markdown rendering** with `ReactMarkdown` + `remarkGfm` — good for agent responses.
  - **Auto-scroll** via `useEffect` on `messages` and `isStreaming` — works but could be improved with scroll anchoring.

### 1.9 `lib/api-client.ts` — API Client

- **Purpose**: Type-safe HTTP client wrapping `fetch` with auth, error handling, content negotiation.
- **Pattern**: `ApiError` class, generic `apiClient<T>()`, convenience `api.get/post/put/delete` methods.
- **Observations**:
  - **Clean design**: Generic, typed, supports raw body for markdown uploads, JSON default.
  - **Auth**: Reads token from `localStorage.getItem('oxios-api-key')` on every request — simple, no refresh mechanism.
  - **Error class**: `ApiError` with `status`, `statusText`, `body` — sufficient for consumers.
  - **Content-type fallback**: If not JSON or text, falls back to `res.json()` — could throw on non-JSON responses.
  - **`VITE_API_BASE`** env var for base URL — good for dev/proxy scenarios.

### 1.10 `lib/sse-client.ts` — SSE Client

- **Purpose**: Server-Sent Events client using `fetch` + `ReadableStream` (not `EventSource`).
- **Pattern**: Class with `connect()`/`disconnect()`, manual SSE parsing (event/data lines), `AbortController` for cancellation.
- **Observations**:
  - **Uses fetch + ReadableStream** instead of native `EventSource` — allows custom headers (Authorization).
  - **Line parsing** is correct but basic: no support for `id:`, `retry:`, or multi-line `data:` fields.
  - **No auto-reconnect** — once the stream ends, the caller must reconnect manually.

### 1.11 `lib/ws-client.ts` — WebSocket Client

- **Purpose**: Reusable WebSocket client with reconnect logic and pending queue.
- **Pattern**: Class with `connect()`/`send()`/`close()`, exponential backoff reconnect (max 10 attempts), message queueing.
- **Observations**:
  - **Reconnect**: Exponential backoff with 30s cap, max 10 attempts — solid pattern.
  - **Pending queue**: Messages sent before WS open are queued and flushed on connect.
  - **`_disposed` flag**: Prevents reconnect after intentional `close()` — correct.
  - **This is a well-designed utility class** — but the chat store (`chat.ts`) doesn't use it! Instead, `chat.ts` manages its own WebSocket directly.

### 1.12 `hooks/use-chat-stream.ts` — Chat Streaming Hook

- **Purpose**: Alternative chat streaming implementation using `WsClient` class.
- **Pattern**: Custom hook with `useState` for messages, `useRef` for WsClient, `useCallback` for send/disconnect.
- **Observations**:
  - **Seems to be an older or alternative implementation** — the chat page uses `useChatStore` instead.
  - **Uses `WsClient`** from `lib/ws-client.ts` (unlike `chat.ts` store which manages its own WS).
  - **Duplicate streaming logic**: Token accumulation logic (`setMessages` with last-message check) is identical to `chat.ts` store's `handleChunk`.
  - **No session management**: Doesn't send `session_id` or `space_id` — less capable than the store version.

### 1.13 `i18n/index.ts` — Internationalization

- **Purpose**: i18next initialization with HTTP backend, browser language detection.
- **Pattern**: `HttpBackend` loads from `/locales/{{lng}}/{{ns}}.json`. Two languages: `en`, `ko`.
- **Observations**:
  - **Minimal setup**: Single namespace `common`, two languages.
  - **Detection order**: `localStorage` → `navigator` — good, respects user preference.
  - **No fallback namespace or pluralization config** — fine for current scope.

### 1.14 `types/index.ts` — Type Definitions

- **Purpose**: Central type definitions for all backend API shapes.
- **Pattern**: Interfaces for each API entity (Agent, Session, Seed, Space, Skill, etc.). `StreamChunk` union type for WS events.
- **Observations**:
  - **Comprehensive**: Covers all major API entities with optional fields matching backend flexibility.
  - **`StreamChunk.type`** uses string union: `'token' | 'tool_call' | 'tool_result' | 'done' | 'error'` — good discriminated union.
  - **`Skill` type is detailed**: Matches RFC-009 with `SkillSource`, `SkillStatus`, `SkillFormat`, `SkillRequirements`, `SkillInstallSpec`.
  - **`PaginatedResponse<T>`** generic — available but not consistently used (chat page manually types the response).
  - **`OxiosEvent`** uses index signature `[key: string]: unknown` — loose typing for ad-hoc SSE fields.

---

## 2. Cross-Cutting Analysis

### 2.1 State Management Strategy

| Approach | Where Used | Notes |
|----------|-----------|-------|
| **Zustand** | `sidebar.ts`, `knowledge.ts`, `chat.ts`, `theme.ts`, `events.ts` | Primary state manager. Some stores use `persist` middleware, others don't. |
| **TanStack Query** | `use-knowledge.ts` (29 hooks), sidebar (approvals), chat page (spaces/sessions) | All API data fetching. Consistent query key structure (`['domain', 'sub', ...params]`). |
| **Local state** | `use-chat-stream.ts` (messages), chat page (input, showHistory) | Component-scoped state. |
| **localStorage** | API key, knowledge sidebar prefs, chat persist, i18n lang | Manual reads in some places, `persist` middleware in others. |

**Verdict**: Clean separation — Zustand for UI state, TanStack Query for server state. However, `chat.ts` store mixes both (manages WS messages as runtime state alongside persisted session IDs), which is pragmatic but blurs the line.

### 2.2 Component Patterns

| Pattern | Consistency | Notes |
|---------|-------------|-------|
| Route components | ✅ Consistent | All use `createFileRoute('/path')({ component: ... })` |
| i18n | ✅ Consistent | All user-facing strings use `t('key')` |
| Shadcn/UI components | ✅ Consistent | `Button`, `Card`, `ScrollArea`, `Textarea`, `Separator`, `Tooltip` |
| Tailwind classes | ✅ Consistent | Utility-first, `cn()` for conditional classes |
| TypeScript | ✅ Strong | All files typed, generics on API calls |

### 2.3 Error Handling

| Layer | Approach | Quality |
|-------|----------|---------|
| API client | `ApiError` class thrown on non-2xx | ✅ Good — structured, typed |
| TanStack Query hooks | No `onError`, default behavior | ⚠️ Consumer must handle; no global error toast |
| Chat streaming | Appends `[Error: ...]` to message | ⚠️ Visible but not actionable |
| SSE client | Optional `onError` callback | ✅ Flexible |
| WS client | Closes on error, reconnects | ✅ Resilient |
| Chat `loadSession` | Silent `catch {}` | ❌ Network errors silently swallowed |
| Chat sidebar fetches | No error handling | ❌ Raw `fetch` with no `.ok` check |

### 2.4 Loading & UX States

| Component | Loading | Empty | Error |
|-----------|---------|-------|-------|
| Chat messages | ✅ Spinner (`Loader2 animate-spin`) + "Thinking" text | ✅ "Send a message" prompt | ⚠️ Inline `[Error: ...]` text |
| Chat connection | ✅ "Connecting..." header text | — | ❌ No connection failed state |
| Chat input | ✅ Disabled when not connected or streaming | — | — |
| Knowledge hooks | Delegated to consumers | Delegated to consumers | Delegated to consumers |
| Spaces sidebar | ❌ No loading spinner | ✅ "Loading..." text | ❌ No error state |
| Sessions sidebar | ❌ No loading spinner | ❌ No empty state | ❌ No error state |

### 2.5 Type Safety

| Aspect | Rating | Notes |
|--------|--------|-------|
| API responses | ✅ Strong | All `api.get<T>()` calls are generically typed |
| Zustand stores | ✅ Strong | Interfaces defined for all state + actions |
| Component props | ✅ Strong | Inline types for local components, named types for shared |
| WS/SSE payloads | ⚠️ Mixed | `StreamChunk` is well-typed but `WsMessageHandler` is `(data: unknown)` |
| Backend fetches | ❌ Weak | `chat.ts` `loadSession` and sidebar use raw `fetch` with `as Promise<...>` casts |

---

## 3. Inconsistencies & Issues

### 🔴 Critical

1. **Duplicate WebSocket implementations**: `use-chat-stream.ts` hook uses `WsClient` class, while `stores/chat.ts` manages its own WebSocket directly. Only the store is used in the chat page. The hook appears to be dead code or an earlier iteration.

2. **Missing auth headers in chat sidebar**: `SpaceSessionSidebar` uses raw `fetch('/api/spaces')` and `fetch('/api/sessions')` without `Authorization` header. The `api` client adds this automatically. If the API requires auth, these calls will fail silently.

3. **`loadSession` uses raw fetch**: `stores/chat.ts` `loadSession` bypasses the `api` client, losing consistent error handling and auth.

### 🟡 Moderate

4. **Inconsistent persistence**: `knowledge.ts` store manually reads/writes `localStorage` for 2 fields. `chat.ts` store uses `persist` middleware. Other stores (sidebar, theme) have their own patterns. No unified approach.

5. **Inconsistent `t()` fallback usage**: Header uses `t('key', 'fallback')` pattern; most other components just use `t('key')`. Should be consistent.

6. **Hardcoded locale `'ko-KR'`**: Chat sidebar date formatting uses `toLocaleString('ko-KR', ...)` regardless of i18n language setting. Should use detected locale.

7. **`use-chat-stream.ts` is likely dead code**: This hook duplicates chat store functionality with fewer features (no session management). Should be removed or consolidated.

8. **SSE client is basic**: No auto-reconnect, no `id:`/`retry:` field support. The `SseClient` class exists but there's also an `EventStore` that manages SSE connections — unclear which is canonical.

### 🟢 Minor

9. **Mode detection via pathname**: `pathname.startsWith('/knowledge')` is simple but fragile. A route metadata approach (e.g., `Route.useMatch`) would be more robust.

10. **No error boundaries**: No React error boundary wraps the app or individual routes. A render error in any component crashes the entire UI.

11. **Message keys use array index**: Chat messages lack backend-assigned IDs, forcing array-index keys. This can cause React reconciliation issues when messages are modified.

12. **`SseClient` unused in analyzed files**: The SSE client class exists but wasn't found being used in the analyzed set. The app uses `EventStore` for SSE instead.

---

## 4. Architecture Summary

```
┌─────────────────────────────────────────────────────────────┐
│                    TanStack Router                           │
│  __root.tsx → QueryClientProvider → AppLayout               │
├─────────────────────────────────────────────────────────────┤
│  AppLayout                                                  │
│  ├── Dashboard mode: Sidebar + Header + <Outlet />          │
│  └── Knowledge mode: KnowledgeSidebar + Header + <Outlet /> │
│      + InfoPanel + SearchModal + MoveModal                  │
├─────────────────────────────────────────────────────────────┤
│  State Management                                           │
│  ├── Zustand stores: sidebar, knowledge, chat, theme, events│
│  ├── TanStack Query: 29 knowledge hooks + misc queries      │
│  └── localStorage: API key, prefs, persisted chat state     │
├─────────────────────────────────────────────────────────────┤
│  Real-time                                                  │
│  ├── WebSocket (chat): inline in chat store OR WsClient     │
│  ├── SSE (events): EventStore → notification pipeline       │
│  └── Polling: TanStack Query refetchInterval                │
├─────────────────────────────────────────────────────────────┤
│  API Layer                                                  │
│  ├── api-client.ts: Typed fetch wrapper with auth           │
│  ├── sse-client.ts: Manual SSE over fetch                   │
│  └── ws-client.ts: Reconnect-capable WebSocket class        │
├─────────────────────────────────────────────────────────────┤
│  i18n: i18next (en, ko) + HTTP backend                      │
│  Types: Centralized in types/index.ts + types/knowledge.ts  │
└─────────────────────────────────────────────────────────────┘
```

### Strengths

- **Clean separation** between UI state (Zustand) and server state (TanStack Query)
- **Type-safe API layer** with generic `api.get<T>()` pattern
- **Consistent TanStack Query patterns** in knowledge hooks — model for other domains
- **Good i18n coverage** — all user-facing strings go through `t()`
- **Resilient WebSocket** with reconnect and message queueing in `WsClient`
- **Thoughtful component extraction** (e.g., `KnowledgeBreadcrumb` separated to scope subscriptions)

### Areas for Improvement

1. **Consolidate chat streaming**: Remove `use-chat-stream.ts` dead code; have `chat.ts` store use `WsClient` class instead of managing its own WebSocket
2. **Use `api` client everywhere**: Replace raw `fetch` calls in `chat.ts` and `SpaceSessionSidebar` with the typed `api` client
3. **Add error boundaries**: At minimum, wrap `<Outlet />` in AppLayout with an error boundary
4. **Standardize persistence**: Pick one approach (Zustand `persist` middleware or manual `localStorage`) and apply consistently
5. **Add global error handling**: TanStack Query `QueryClient` config with global `onError` for toast notifications
6. **Fix locale hardcoding**: Use i18n language for date formatting throughout
