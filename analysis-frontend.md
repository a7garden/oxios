# Frontend Analysis Report

**Project:** oxios-web (`channels/oxios-web/web`)
**Date:** 2026-05-28
**Scope:** Dependency audit, code quality, type safety, security, state management

---

## 1. Dependency Issues

### 1.1 Unused Dependencies (no imports found in `src/`)

| Package | Type | Evidence |
|---------|------|----------|
| `react-hook-form` | dependency | No `from 'react-hook-form'` found in any source file |
| `@hookform/resolvers` | dependency | No `from '@hookform/resolvers'` found in any source file |
| `zod` | dependency | No `from 'zod'` found in any source file |
| `shadcn` | dependency | CLI tool installed as runtime dependency; should be `devDependencies` |

**Impact:** Increases bundle size unnecessarily. `shadcn` is a scaffolding CLI — it has no runtime purpose once components are generated.

### 1.2 ESLint Configuration Missing

ESLint packages are declared in `devDependencies` (`eslint`, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`) but **no ESLint config file exists** (no `.eslintrc.*` or `eslint.config.*`). The project uses **Biome** instead (`biome.json`).

**Impact:** The `eslint`-related devDependencies are dead weight. The `lint` script correctly uses `biome check .`, so this is just package bloat.

### 1.3 Version Inconsistencies

- `@tanstack/router-devtools` (`^1.167.0`) and `@tanstack/router-plugin` (`^1.168.6`) are slightly behind `@tanstack/react-router` (`^1.170.4`). Minor version drift within a monorepo can cause subtle type mismatches.

---

## 2. TODO/FIXME/HACK/XXX Markers

**Result: None found.** The codebase is clean of these markers.

---

## 3. Type Safety Issues (`as any`)

### 3.1 `src/components/knowledge/markdown-editor.tsx` (6 occurrences)

| Line | Code | Risk |
|------|------|------|
| 156 | `} as any)` | Casting editor config — bypasses type checking on HyperMD options |
| 160 | `(cm as any).hmdResolveURL?.bind(cm)` | Accessing undocumented HyperMD internals |
| 187 | `(cm as any).hmdResolveURL = resolveURL` | Monkey-patching HyperMD internal method |
| 189 | `(cm as any).hmdReadLink = readLink` | Monkey-patching HyperMD internal method |
| 199 | `(cm as any).showHint({` | Calling CM5 addon method without types |

**Assessment:** These are **unavoidable** — HyperMD (CM5 ecosystem) lacks TypeScript declarations for its plugin APIs. The `eslint-disable` comments are present. Low risk but should be tracked.

### 3.2 `src/routeTree.gen.ts` (~30 occurrences)

All `as any` casts are in the **auto-generated** route tree file from TanStack Router. Not actionable.

### 3.3 `src/lib/hypermd-setup.ts` (6 occurrences)

| Line | Code | Risk |
|------|------|------|
| 14-15 | `(window as any).CodeMirror` | Checking/reading global CM5 instance |
| 20-24 | `(CodeMirrorNS as any).default/fromTextArea` | Module resolution fallback chain |
| 33 | `(window as any).CodeMirror = CodeMirror` | Setting global CM5 instance |
| 103 | `(window as any).CodeMirror as typeof import('codemirror')` | Re-reading global after setup |

**Assessment:** Required due to HyperMD's UMD/CJS module format conflicting with Vite 8's ESM processing. The fallback chain is defensive. Low risk.

### 3.4 `src/hooks/use-engine.ts` — Unsafe Config Type Casting

```typescript
// useSetModel, useSetApiKey, useSetProviderOptions
const config = await api.get<Record<string, unknown>>('/api/config')
;(config.engine as Record<string, unknown>).default_model = model
```

Multiple places cast `config.engine` to `Record<string, unknown>` for mutation. This bypasses type safety on config structure and could silently corrupt config if the API shape changes.

**Recommendation:** Define a typed `EngineConfig` interface and use type guards.

---

## 4. Unused Code

### 4.1 Dead Hook: `useChatStream` (`src/hooks/use-chat-stream.ts`)

The entire `useChatStream` hook (standalone WebSocket chat) is **never imported** anywhere in the codebase. Chat functionality uses `useChatStore` from `stores/chat.ts` instead.

**Action:** Delete `src/hooks/use-chat-stream.ts`.

### 4.2 Unused Imports in Routes

`src/routes/chat.tsx` imports from `@tanstack/react-query` and uses raw `fetch()` instead of the `api` client:
```typescript
// Line 239, 248 — direct fetch without auth headers
fetch('/api/spaces').then((r) => r.json())
fetch('/api/sessions').then((r) => r.json())
```
**Issue:** Missing `Authorization: Bearer <token>` header — these calls will fail if API auth is enforced.

---

## 5. State Management Patterns

### 5.1 Overview

| Store | Pattern | Persisted |
|-------|---------|-----------|
| `knowledge.ts` | Zustand + manual localStorage | Partial (sidebar width/open) |
| `chat.ts` | Zustand + `persist` middleware | Partial (session/space IDs) |
| `events.ts` | Zustand (singleton SSE) | No |
| `auth.ts` | Zustand + manual localStorage | Yes (token) |
| `notifications.ts` | Zustand (ephemeral) | No |
| `sidebar.ts` | Zustand + manual localStorage | Yes (collapsed) |
| `theme.ts` | Zustand + manual localStorage | Yes (theme) |

### 5.2 Inconsistency: Two Persistence Patterns

The codebase uses **two different persistence strategies**:

1. **Manual localStorage** (`knowledge.ts`, `auth.ts`, `sidebar.ts`, `theme.ts`) — reads/writes localStorage directly in actions
2. **Zustand `persist` middleware** (`chat.ts`) — uses the built-in middleware

**Recommendation:** Standardize on `persist` middleware across all stores that need persistence. The manual approach is error-prone (e.g., `knowledge.ts` reads from localStorage at module initialization time, which can fail in SSR contexts).

### 5.3 Dual Chat Implementation

Two chat systems coexist:
- `stores/chat.ts` — Full-featured chat store with WS, persistence, session management
- `hooks/use-chat-stream.ts` — Standalone hook that creates its own WS connection (**unused**)

The store (`chat.ts`) manages its own WebSocket singleton outside the store's reactive boundary (module-level `wsInstance` and `chunkHandler` variables). This is a known pattern for zustand but creates tight coupling.

### 5.4 SSE Singleton Leak in `stores/events.ts`

```typescript
let client: SseClient | null = null
```

The `SseClient` instance lives as a module-level variable. If `connect()` is called multiple times in strict mode (React 18 double-mount), the idempotent guard (`if (client) return`) prevents duplicates, but the client reference is never cleaned up on unmount — only on explicit `reconnect()`.

---

## 6. Security Issues

### 6.1 API Token in WebSocket URL (Medium)

**File:** `src/stores/chat.ts:74`
```typescript
const sep = token ? `?token=${encodeURIComponent(token)}` : ''
return `${protocol}//${window.location.host}/api/chat/stream${sep}`
```
The API key is passed as a URL query parameter for WebSocket connections. This is standard practice for browser WebSocket APIs (which don't support custom headers), but the token will appear in:
- Server access logs
- Browser history (minor)
- Proxy logs

**Mitigation:** Use a short-lived one-time ticket endpoint (e.g., `POST /api/ws-ticket` → get a temporary token for WS connection).

### 6.2 Missing Auth on Sidebar API Calls in `chat.tsx` (High)

**File:** `src/routes/chat.tsx:239-248`
```typescript
fetch('/api/spaces').then((r) => r.json())
fetch('/api/sessions').then((r) => r.json())
```

These two `fetch` calls in `SpaceSessionSidebar` do **not** include the `Authorization` header. Every other API call uses `api.get()` from `lib/api-client.ts` which attaches the Bearer token from localStorage.

**Fix:**
```typescript
import { api } from '@/lib/api-client'
// Replace raw fetch with:
const { data: spacesData } = useQuery({
  queryKey: ['spaces'],
  queryFn: () => api.get<{items: Space[]; total: number}>('/api/spaces'),
})
```

### 6.3 No Content Security Policy

No CSP headers or meta tags are configured. Combined with `react-markdown` rendering user-generated content, this could allow injected scripts if the backend doesn't sanitize markdown.

**Note:** `react-markdown` itself is safe (renders to React elements, not raw HTML), so the risk is limited. However, if `remarkRehype` plugins or custom components are added later that render HTML, this becomes a vector.

### 6.4 `innerHTML` Usage (Low Risk)

**File:** `src/components/knowledge/markdown-editor.tsx:50`
```typescript
container.innerHTML = ''
```
Used solely to clear the CodeMirror container div before re-creating the editor. Not exploitable since it's always set to an empty string.

---

## 7. API Hook Error Handling (`hooks/use-knowledge.ts`)

### 7.1 Summary

All 29 hooks use TanStack Query's `useQuery`/`useMutation` which provide built-in error states (`error`, `isError`, `failureCount`). However:

| Issue | Details |
|-------|---------|
| **No `onError` callbacks** | None of the 29 hooks define `onError` handlers. Errors are only surfaced via TanStack's `error` state property. |
| **No global error boundary** | The `QueryClient` in `main.tsx` does not set global `onError` for queries or mutations. |
| **No toast notifications on failure** | Mutations like `useWriteFile`, `useDeleteFile` silently fail — the user only sees stale data. |

### 7.2 Specific Hooks Missing Error Handling

| Hook | Impact of Silent Failure |
|------|--------------------------|
| `useWriteFile` | User's edits are lost on save failure |
| `useDeleteFile` | File appears deleted in UI but still exists on server |
| `useKnowledgeCopilot` | Copilot panel shows loading forever |
| `useChecklistComplete` | Checkmark appears but isn't persisted |
| `useKnowledgeFileRestore` | User thinks file is restored but it isn't |

### 7.3 Inconsistent Error Handling Across Routes

Some routes handle errors manually:
- `src/routes/marketplace.tsx:52` — has `onError` with toast
- `src/routes/skills.tsx:148` — has `onError` with toast
- `src/components/knowledge/search-modal.tsx:135` — has `onError` callback

But the majority of mutation consumers don't check `isError` or `error`.

### 7.4 Recommendation

Add a global mutation error handler to `QueryClient`:

```typescript
const queryClient = new QueryClient({
  defaultOptions: {
    mutations: {
      onError: (error) => {
        // Show toast notification
        console.error('Mutation failed:', error)
      },
    },
  },
})
```

---

## 8. Additional Findings

### 8.1 `ToastProvider` Never Mounted

**File:** `src/components/ui/sonner.tsx` exports `ToastProvider` and `useToast`, but `ToastProvider` is **never rendered** in the component tree (`__root.tsx` doesn't include it).

Despite this, `useToast` is called in `marketplace.tsx` and `skills.tsx`. Since `useToast` returns from `useContext(ToastContext)` with a default `{ toast: () => {} }`, **all toast calls are silently swallowed**. Toast notifications in marketplace and skills pages do nothing.

**Fix:** Wrap the app in `<ToastProvider>` in `__root.tsx`.

### 8.2 Knowledge Store: `localStorage` Read at Module Scope

**File:** `src/stores/knowledge.ts:35-36`
```typescript
const savedWidth = Number(localStorage.getItem('oxios-knowledge-sidebar-width')) || 280
const savedSidebarOpen = localStorage.getItem('oxios-knowledge-sidebar-open') !== 'false'
```

This will throw `ReferenceError: localStorage is not defined` in SSR/Node environments (e.g., tests). The `theme.ts` store has the same issue.

### 8.3 `useChatStore` Auto-Connects on Rehydration

**File:** `src/stores/chat.ts` — `onRehydrateStorage` callback calls `state.connect()` and `state.loadSession()`. This means:
1. Every page load opens a WebSocket connection
2. If a session was active, its history is fetched immediately
3. This happens even on non-chat pages

### 8.4 `SseClient` Reconnection Gap

**File:** `src/lib/sse-client.ts` — The SSE client has **no automatic reconnection**. When the connection drops (network hiccup, server restart), events stop flowing until the user navigates to the Events page and triggers a manual reconnect. Compare with `WsClient` which has exponential backoff reconnection.

### 8.5 Missing TypeScript Strict Checks on API Responses

Many types use `[key: string]: unknown` index signatures:
- `TodayReport` — all fields are `unknown`
- `NightlyReport` — all fields are `unknown`
- `HabitsData` — all fields are `unknown`
- `OxiosConfig` — nested `unknown` index signatures

This defeats TypeScript's value in catching API contract violations.

---

## Priority Summary

| Priority | Issue | Location |
|----------|-------|----------|
| 🔴 **High** | Missing auth headers on `fetch()` calls | `src/routes/chat.tsx:239,248` |
| 🔴 **High** | `ToastProvider` never mounted — toasts silently fail | `src/routes/__root.tsx`, `src/components/ui/sonner.tsx` |
| 🟡 **Medium** | No mutation error handling — silent failures on save/delete | `src/hooks/use-knowledge.ts` (all mutations) |
| 🟡 **Medium** | Unused dependencies: `react-hook-form`, `zod`, `@hookform/resolvers` | `package.json` |
| 🟡 **Medium** | Dead code: `useChatStream` hook never used | `src/hooks/use-chat-stream.ts` |
| 🟡 **Medium** | SSE client has no auto-reconnect | `src/lib/sse-client.ts` |
| 🟡 **Medium** | API token exposed in WebSocket URL query params | `src/stores/chat.ts:74` |
| 🟢 **Low** | Inconsistent state persistence patterns (manual vs middleware) | All stores |
| 🟢 **Low** | `as any` casts in HyperMD integration (unavoidable) | `markdown-editor.tsx`, `hypermd-setup.ts` |
| 🟢 **Low** | `localStorage` access at module scope (SSR-hostile) | `stores/knowledge.ts`, `stores/theme.ts` |
| 🟢 **Low** | Weakly typed API responses (index signatures) | `types/knowledge.ts` |
| 🟢 **Low** | ESLint packages declared but no config file | `package.json` |
