# RFC-016: Frontend 정리 및 패턴 통일

> **상태:** 📝 설계
> **날짜:** 2026-05-27 (개정)
> **우선순위:** P1
> **범위:** `channels/oxios-web/web/src/`
> **선행:** 없음
> **후행:** 없음

---

## 1. 동기

프론트엔드에 패턴 불일치와 데드 코드가 혼재:

| # | 문제 | 심각도 |
|---|------|--------|
| 1 | `use-chat-stream.ts` + `ws-client.ts` — 어느 것도 import 없는 완전 데드 코드 | 🔴 |
| 2 | `SpaceSessionSidebar`가 raw `fetch` 2군데 사용 → auth 헤더 누락 | 🔴 |
| 3 | `getToken()` 4곳 중복 — auth store 무시하고 각자 localStorage 직접 읽기 | 🔴 |
| 4 | Error Boundary 미적용 → 렌더 에러 시 전체 UI 크래시 | 🟡 |
| 5 | 글로벌 에러 핸들링 없음 → TanStack Query 에러가 사용자에게 안 보임 | 🟡 |
| 6 | Persistence 불일치: 4개 store가 수동 localStorage, chat만 persist 미들웨어 | 🟡 |
| 7 | `loadSession`이 api 클라이언트 우회 (auth는 있으나 일관성 없음) | 🟢 |
| 8 | `t()` fallback 사용 불일치 | 🟢 |
| 9 | 하드코딩된 `'ko-KR'` 로케일 (1군데) | 🟢 |

### 발견 배경

코드베이스 교차 검증으로 다음을 확인:

- **#1**: `grep -rn "use-chat-stream"` 결과 없음. `ws-client.ts`의 유일한 consumer가 `use-chat-stream.ts`. 두 파일 모두 안전 삭제.
- **#2**: `routes/chat.tsx` L298, L305 — `/api/spaces`, `/api/sessions` 둘 다 auth 없는 raw fetch.
- **#3**: `getToken()`이 `stores/chat.ts`, `lib/api-client.ts`, `lib/sse-client.ts`, `stores/auth.ts` 각각 자체 구현. auth store가 source of truth인데 무시됨.
- **#10 (이전)**: `SseClient`는 `stores/events.ts`에서 정상 사용 중 — 제거 항목에서 제외.
- **#9 (이전)**: 메시지 key에 array index 사용 — 백엔드에서 ID를 내려주지 않는 구조적 한계. 프론트엔드 단독 해결 불가. 본 RFC 범위 밖.

---

## 2. 설계

### 2.1 데드 코드 제거

**제거 대상:**

```
channels/oxios-web/web/src/
├── hooks/use-chat-stream.ts     ← 삭제 (import 없음)
└── lib/ws-client.ts             ← 삭제 (use-chat-stream.ts만 import)
```

`chat.ts`는 이미 자체 WebSocket 수명주기를 관리 (모듈 레벨 `wsInstance`, `buildWsUrl()`, `chunkHandler`). WsClient의 재연결/대기열 로직이 필요해지면 그때 분리. 현재는 두 구현이 독립적이고 한쪽이 완전 미사용.

### 2.2 Raw Fetch → API 클라이언트 통일

`SpaceSessionSidebar`의 2개 raw fetch를 `api` 클라이언트로 교체:

```typescript
// 변경 전: routes/chat.tsx L298, L305
const { data: spacesData } = useQuery({
  queryKey: ['spaces'],
  queryFn: () =>
    fetch('/api/spaces').then((r) => r.json()) as Promise<{...}>,   // ❌ auth 없음
})

const { data: sessionsData, refetch: refetchSessions } = useQuery({
  queryKey: ['sessions', activeSpaceId],
  queryFn: () =>
    fetch('/api/sessions').then((r) => r.json()) as Promise<{...}>, // ❌ auth 없음
})

// 변경 후
import { api } from '@/lib/api-client';

const { data: spacesData } = useQuery({
  queryKey: ['spaces'],
  queryFn: () => api.get<{ items: Space[]; total: number }>('/api/spaces'),
})

const { data: sessionsData, refetch: refetchSessions } = useQuery({
  queryKey: ['sessions', activeSpaceId],
  queryFn: () => api.get<{ items: Session[]; total: number }>('/api/sessions'),
})
```

`loadSession`도 api 클라이언트로 통일 (auth는 이미 있으나 일관성 확보):

```typescript
// 변경 전: stores/chat.ts — auth는 있으나 에러 핸들링이 api 클라이언트와 불일치
const res = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}`, {
  headers: { Authorization: `Bearer ${getToken()}`, ... },
})

// 변경 후
const data = await api.get<SessionDetail>(`/api/sessions/${encodeURIComponent(sessionId)}`);
```

### 2.3 `getToken()` 중복 제거

**문제:** `localStorage.getItem('oxios-api-key')`가 4곳에서 각자 구현:

| 위치 | 방식 |
|------|------|
| `stores/auth.ts` | `getItem` → `token`/`isAuthenticated` 상태 |
| `stores/chat.ts` | `getToken()` 함수 |
| `lib/api-client.ts` | 인라인 `getItem` |
| `lib/sse-client.ts` | 인라인 `getItem` |

**해결:** `auth store`를 단일 source of truth로 지정. 다른 곳은 `useAuthStore.getState().token` 또는 getter 함수 사용.

```typescript
// stores/auth.ts에 추가
export function getAuthToken(): string | null {
  return useAuthStore.getState().token
}

// lib/api-client.ts — 교체
const token = getAuthToken()

// lib/sse-client.ts — 교체
const token = getAuthToken()

// stores/chat.ts — getToken() 제거 후 getAuthToken() 사용
```

**주의:** `auth.ts`는 persist 미들웨어를 적용하지 않음 (2.4 참조). 대신 `useAuthStore.getState()`로 어디서든 동기 접근 가능.

### 2.4 Error Boundary + 글로벌 에러 토스트

**Error Boundary 전략:** Root 1개 + 주요 라우트 개별 적용.

Root-level EB는 마지막 안전망. 하지만 모든 에러가 전체 앱을 크래시 화면으로 만들면 안 됨 — chat에서 에러 나도 sidebar, knowledge는 멀쩡해야 함. 따라서:

```typescript
// routes/__root.tsx — 마지막 안전망
<QueryClientProvider client={queryClient}>
  <ErrorBoundary fallback={<GlobalErrorFallback />}>
    <AppLayout />
  </ErrorBoundary>
</QueryClientProvider>
```

```typescript
// routes/chat.tsx, routes/knowledge/index.tsx 등 — 라우트 레벨
// TanStack Router의 errorComponent 옵션 활용
export const Route = createFileRoute('/chat')({
  component: ChatPage,
  errorComponent: RouteErrorFallback,  // ← 각 라우트 격리
})
```

```typescript
// components/shared/error-boundary.tsx — 개선
// reset이 setState({ hasError: false })인데, 원인 미해결 시 무한 루프 위험.
// 대신 페이지 새로고침 유도:
interface Props {
  children: ReactNode
  fallback?: (error: Error, reset: () => void) => ReactNode
}
```

**글로벌 Query 에러 핸들링:**

```typescript
// main.tsx
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 30_000,
    },
    mutations: {
      onError: (error) => {
        const message = error instanceof ApiError
          ? (error.body as Record<string, string>)?.message ?? error.statusText
          : '요청 처리 중 오류가 발생했습니다.';
        toast.error(message);
      },
    },
  },
});
```

### 2.5 Persistence 패턴 통일

**기준:** Zustand `persist` 미들웨어. `stores/chat.ts`가 이미 사용 중 — 본 패턴의 참조 구현.

**마이그레이션 대상 (3개 store):**

| Store | 수동 localStorage 키 | persist name | partialize |
|-------|----------------------|--------------|------------|
| `knowledge.ts` | `oxios-knowledge-sidebar-width`, `oxios-knowledge-sidebar-open` | `oxios-knowledge` | `sidebarOpen`, `sidebarWidth` |
| `sidebar.ts` | `oxios-sidebar-collapsed` | `oxios-sidebar` | `collapsed` |
| `theme.ts` | `oxios-theme` | `oxios-theme` | `theme` |

**예외: `auth.ts`는 수동 유지.** 이유:
- token 같은 민감 정보의 자동 직렬화는 의도치 않은 노출 위험
- persist의 `partialize`로 제외해도 디버깅 시 실수 가능
- 현재 수동 방식이 단순하고 충분

**`theme.ts` 특이사항:** `applyTheme()`가 DOM 조작 (class toggle). persist 적용 시 `onRehydrateStorage`에서 호출:

```typescript
// stores/theme.ts — persist 적용
export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      theme: 'dark',
      resolved: 'dark',
      setTheme: (theme) => {
        const r = resolveTheme(theme)
        applyTheme(r)
        set({ theme, resolved: r })
      },
    }),
    {
      name: 'oxios-theme',
      partialize: (state) => ({ theme: state.theme }),
      onRehydrateStorage: () => (state) => {
        if (state) {
          const r = resolveTheme(state.theme)
          applyTheme(r)
          state.resolved = r
        }
      },
    },
  ),
)
```

**localStorage 키 마이그레이션:** 기존 수동 키와 persist가 생성하는 키가 다르므로, 초기 로드 시 기존 값을 읽어 persist 스토리지로 이관:

```typescript
// stores/knowledge.ts — 마이그레이션 헬퍼
function migrateFromLegacy(): Partial<KnowledgeState> | undefined {
  const legacyWidth = localStorage.getItem('oxios-knowledge-sidebar-width')
  const legacyOpen = localStorage.getItem('oxios-knowledge-sidebar-open')
  if (legacyWidth || legacyOpen) {
    localStorage.removeItem('oxios-knowledge-sidebar-width')
    localStorage.removeItem('oxios-knowledge-sidebar-open')
    return {
      sidebarWidth: legacyWidth ? Number(legacyWidth) : 280,
      sidebarOpen: legacyOpen !== 'false',
    }
  }
  return undefined
}
```

### 2.6 하드코딩 로케일 수정

`'ko-KR'`이 사용되는 곳은 `routes/chat.tsx` L310 **단 한 군데**. 유틸 함수는 오버엔지니어링 — 직접 치환:

```typescript
// 변경 전
new Date(s.created_at).toLocaleString('ko-KR', {
  month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
})

// 변경 후
import i18n from '@/i18n';
new Date(s.created_at).toLocaleString(i18n.language, {
  month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
})
```

나머지 15+ 곳은 `toLocaleString()` (인자 없음)으로 이미 브라우저 로케일을 따르고 있음 — 변경 불필요.

### 2.7 `t()` Fallback 표준화

현재 `i18n/index.ts`에 `fallbackLng: 'en'`이 이미 설정되어 있어, 번역 키 누락 시 영어로 대체됨. 그러나 코드 레벨 fallback도 보장:

```typescript
// ❌ 일관성 없음
t('common.toggleSidebar', 'Toggle sidebar')  // header.tsx
t('common.toggleSidebar')                     // 다른 곳

// ✅ 항상 fallback 제공
t('common.toggleSidebar', 'Toggle sidebar')
```

`grep -rn "t('" --include="*.tsx" | grep -v "fallback"` 으로 누락 감사 후 일괄 추가.

---

## 3. 마이그레이션 계획

### Phase 1: 데드 코드 + Critical (0.5일)

| 작업 | 비고 |
|------|------|
| `use-chat-stream.ts` 삭제 | `grep -rn "use-chat-stream"` → 0건 확인 후 삭제 |
| `ws-client.ts` 삭제 | `grep -rn "ws-client"` → use-chat-stream.ts만 확인 후 삭제 |
| `SpaceSessionSidebar` fetch 2건 → `api.get` 교체 | `/api/spaces`, `/api/sessions` |
| `loadSession` fetch → `api.get` 교체 | `stores/chat.ts` (일관성) |
| `getToken()` → `getAuthToken()` 통일 | `chat.ts`, `api-client.ts`, `sse-client.ts` |

### Phase 2: 안정성 인프라 (0.5일)

| 작업 | 비고 |
|------|------|
| Root ErrorBoundary 적용 | `__root.tsx` — 마지막 안전망 |
| 주요 라우트 ErrorBoundary 적용 | chat, knowledge 등 — TanStack Router `errorComponent` |
| `error-boundary.tsx` reset 로직 개선 | 무한 루프 방지 |
| 글로벌 QueryClient 에러 토스트 | `mutations.onError` + `toast.error` |

### Phase 3: 패턴 통일 (0.5일)

| 작업 | 비고 |
|------|------|
| `knowledge.ts` → persist 마이그레이션 | 기존 localStorage 키 이관 |
| `sidebar.ts` → persist 마이그레이션 | 동일 |
| `theme.ts` → persist 마이그레이션 | `applyTheme` onRehydrate 처리 |
| `ko-KR` → `i18n.language` 치환 | 단 1군데 |
| `t()` fallback 감사 | 누락된 곳에 fallback 추가 |

---

## 4. 영향 범위

| 파일 | 변경 유형 |
|------|-----------|
| `hooks/use-chat-stream.ts` | **삭제** |
| `lib/ws-client.ts` | **삭제** |
| `routes/chat.tsx` | raw fetch 3건 → api.get 교체 |
| `stores/chat.ts` | loadSession → api.get, getToken → getAuthToken |
| `lib/api-client.ts` | localStorage → getAuthToken |
| `lib/sse-client.ts` | localStorage → getAuthToken |
| `stores/auth.ts` | getAuthToken() export 추가 |
| `routes/__root.tsx` | ErrorBoundary 래핑 |
| `components/shared/error-boundary.tsx` | reset 로직 개선 |
| `main.tsx` | QueryClient mutations.onError 추가 |
| `stores/knowledge.ts` | persist 미들웨어 적용 + 마이그레이션 |
| `stores/sidebar.ts` | persist 미들웨어 적용 |
| `stores/theme.ts` | persist 미들웨어 적용 |

---

## 5. 위험 및 완화

| 위험 | 완화 |
|------|------|
| `ws-client.ts` 제거 후 숨은 import | `grep -rn "ws-client"` 선행 확인 — 유일 consumer가 use-chat-stream.ts |
| Error Boundary가 에러를 삼켜 디버깅 어려움 | `console.error` 유지 + 에러 리포팅 서비스 연동 |
| persist 마이그레이션 시 기존 localStorage 키 불일치 | `migrateFromLegacy()` 로 기존 키 읽고 제거 |
| `getAuthToken()` 순환 의존 | auth.ts는 zustand store이므로 `.getState()`는 동기 — 순환 없음 |
| theme persist onRehydrate 시점에 DOM 미반영 | `onRehydrateStorage` 콜백에서 `applyTheme()` 직접 호출 |

---

## 6. 성공 기준

- [ ] `use-chat-stream.ts`, `ws-client.ts` 삭제
- [ ] `grep -rn "fetch(" --include="*.ts" --include="*.tsx"` 결과가 `api-client.ts`, `sse-client.ts`만 남음
- [ ] `localStorage.getItem('oxios-api-key')` 직접 호출이 `stores/auth.ts` 1곳만 남음
- [ ] 렌더 에러 시 해당 라우트만 ErrorBoundary 표시, 전체 앱 크래시 없음
- [ ] TanStack Query mutation 에러가 자동으로 토스트 표시
- [ ] knowledge, sidebar, theme store가 persist 미들웨어 사용
- [ ] `ko-KR` 하드코딩 제거
