# RFC-016: Frontend 정리 및 패턴 통일

> **상태:** 📝 설계
> **날짜:** 2026-05-26
> **우선순위:** P1
> **범위:** `channels/oxios-web/web/src/`
> **선행:** 없음
> **후행:** 없음

---

## 1. 동기

프론트엔드에 패턴 불일치와 데드 코드가 혼재:

| # | 문제 | 심각도 |
|---|------|--------|
| 1 | `use-chat-stream.ts` (WsClient 사용) vs `stores/chat.ts` (자체 WS) — 전자 데드 코드 | 🔴 |
| 2 | Chat 사이드바가 raw `fetch` 사용 → auth 헤더 누락 | 🔴 |
| 3 | `loadSession`이 raw `fetch` 사용 → api 클라이언트 우회 | 🟡 |
| 4 | Error Boundary 없음 → 렌더 에러 시 전체 UI 크래시 | 🟡 |
| 5 | 글로벌 에러 핸들링 없음 → TanStack Query 에러가 사용자에게 안 보임 | 🟡 |
| 6 | Persistence 불일치: knowledge=수동, chat=persist 미들웨어 | 🟡 |
| 7 | `t()` fallback 사용 불일치 | 🟢 |
| 8 | 하드코딩된 `'ko-KR'` 로케일 | 🟢 |
| 9 | 메시지 key에 array index 사용 (백엔드 ID 없음) | 🟢 |
| 10 | `SseClient` 클래스 미사용 (EventStore가 직접 SSE 관리) | 🟢 |

---

## 2. 설계

### 2.1 데드 코드 제거

**제거 대상:**

```
channels/oxios-web/web/src/
├── hooks/use-chat-stream.ts     ← 삭제 (stores/chat.ts가 실제 사용)
└── lib/ws-client.ts             ← 삭제 또는 chat.ts에 통합
```

**`stores/chat.ts`에 `WsClient` 통합:**

```typescript
// 변경 전: chat.ts가 자체 WebSocket 관리
const wsInstance: WebSocket | null = null; // 모듈 레벨

// 변경 후: WsClient 클래스를 재사용
import { WsClient } from '@/lib/ws-client';

// 또는 (더 나은 방법):
// WsClient의 로직을 chat store에 통합하고 ws-client.ts 삭제
// WsClient의 재연결 로직(지수 백오프, 대기열)은 chat store에 이미 부분적으로 있음
```

**권장:** `ws-client.ts`의 재연결/대기열 패턴을 `chat.ts`에 이미 있다면 `ws-client.ts`와 `use-chat-stream.ts` 모두 삭제. 중복이 없다면 `chat.ts`가 `WsClient`를 사용하도록 리팩토링.

### 2.2 Raw Fetch → API 클라이언트 통일

```typescript
// 변경 전: routes/chat.tsx SpaceSessionSidebar
const res = await fetch('/api/spaces');      // ❌ auth 헤더 없음
const data = (await res.json()) as Space[];  // ❌ 타입 캐스트

// 변경 후
import { api } from '@/lib/api-client';

const { data } = await useQuery({
  queryKey: ['spaces'],
  queryFn: () => api.get<Space[]>('/api/spaces'), // ✅ auth 헤더 자동
});
```

```typescript
// 변경 전: stores/chat.ts loadSession
const res = await fetch(`/api/sessions/${id}`); // ❌ auth 없음
const data = await res.json() as SessionDetail; // ❌ 캐스트

// 변경 후
async loadSession(id: string) {
  const data = await api.get<SessionDetail>(`/api/sessions/${id}`);
  // api 클라이언트가 에러 핸들링 + auth 헤더 처리
}
```

### 2.3 Error Boundary + 글로벌 에러 토스트

**React Error Boundary:**

```typescript
// components/shared/error-boundary.tsx — 이미 존재, 적용만 필요

// routes/__root.tsx에 적용
export const Route = createRootRouteWithContext<RouterContext>()({
  component: RootComponent,
});

function RootComponent() {
  return (
    <QueryClientProvider client={queryClient}>
      <ErrorBoundary
        fallback={({ error, reset }) => (
          <div className="flex h-screen items-center justify-center">
            <div className="text-center">
              <h2 className="text-xl font-bold">문제가 발생했습니다</h2>
              <p className="text-muted-foreground mt-2">{error.message}</p>
              <Button onClick={reset} className="mt-4">다시 시도</Button>
            </div>
          </div>
        )}
      >
        <Outlet />
      </ErrorBoundary>
    </QueryClientProvider>
  );
}
```

**글로벌 Query 에러 핸들링:**

```typescript
// main.tsx 또는 hooks/query-client.ts
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 2,
      staleTime: 30_000,
    },
    mutations: {
      onError: (error) => {
        // 글로벌 에러 토스트
        const message = error instanceof ApiError
          ? error.body?.message ?? error.statusText
          : '요청 처리 중 오류가 발생했습니다.';
        toast.error(message);
      },
    },
  },
});
```

### 2.4 Persistence 패턴 통일

**기준:** Zustand `persist` 미들웨어를 표준으로 채택.

```typescript
// 변경 전: stores/knowledge.ts
// 수동 localStorage 읽기/쓰기
sidebarOpen: localStorage.getItem('kb-sidebar-open') !== 'false',
setSidebarOpen: (open) => {
  localStorage.setItem('kb-sidebar-open', String(open));
  set({ sidebarOpen: open });
},

// 변경 후: persist 미들웨어 사용
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export const useKnowledgeStore = create<KnowledgeState>()(
  persist(
    (set, get) => ({
      sidebarOpen: true,
      sidebarWidth: 240,
      // ...다른 상태
      setSidebarOpen: (open) => set({ sidebarOpen: open }),
    }),
    {
      name: 'oxios-knowledge',
      partialize: (state) => ({
        sidebarOpen: state.sidebarOpen,
        sidebarWidth: state.sidebarWidth,
      }),
    }
  )
);
```

### 2.5 i18n 로케일 일관성

```typescript
// 변경 전: routes/chat.tsx
const dateStr = date.toLocaleString('ko-KR', { ... }); // ❌ 하드코딩

// 변경 후: i18n 언어 설정 사용
import i18n from '@/i18n';

const dateStr = date.toLocaleString(i18n.language, { ... });

// 또는 유틸 함수로 추출
// lib/date-format.ts
export function formatDate(date: Date, options?: Intl.DateTimeFormatOptions): string {
  return date.toLocaleString(i18n.language, options);
}
```

### 2.6 `t()` Fallback 표준화

```typescript
// 규칙: fallback은 항상 제공
// 이유: 번역 키가 누락되어도 UI가 깨지지 않도록

// ❌ 변경 전 (일관성 없음)
t('common.toggleSidebar', 'Toggle sidebar')  // header.tsx
t('common.toggleSidebar')                     // sidebar.tsx

// ✅ 변경 후: 항상 fallback 제공
t('common.toggleSidebar', 'Toggle sidebar')

// 또는 i18next 설정에서 fallback 언어 보장
i18n.init({
  fallbackLng: 'en',  // 번역 누락 시 영어로 대체
});
```

---

## 3. 마이그레이션 계획

### Phase 1: 데드 코드 + Critical 수정 (0.5일)

| 작업 | 비고 |
|------|------|
| `use-chat-stream.ts` 삭제 | import 경로 확인 후 제거 |
| `ws-client.ts` 정리 | chat.ts에 통합하거나 반대 |
| Chat 사이드바 `fetch` → `api` 교체 | `SpaceSessionSidebar` 컴포넌트 |
| `loadSession` → `api` 교체 | `stores/chat.ts` |

### Phase 2: 안정성 인프라 (0.5일)

| 작업 | 비고 |
|------|------|
| Error Boundary를 `__root.tsx`에 적용 | 기존 `error-boundary.tsx` 활용 |
| 글로벌 QueryClient 에러 핸들링 | `toast.error` 연동 |
| QueryClient 기본 옵션 설정 | retry, staleTime |

### Phase 3: 패턴 통일 (0.5일)

| 작업 | 비고 |
|------|------|
| Knowledge store `persist` 미들웨어 적용 | 수동 localStorage 제거 |
| 날짜 포맷 유틸 추출 | `lib/date-format.ts` |
| `t()` fallback 감사 | 누락된 곳에 fallback 추가 |

---

## 4. 영향 범위

| 파일 | 변경 유형 |
|------|-----------|
| `hooks/use-chat-stream.ts` | **삭제** |
| `lib/ws-client.ts` | 삭제 또는 chat.ts 통합 |
| `stores/chat.ts` | raw fetch → api 교체 |
| `routes/chat.tsx` | SpaceSessionSidebar fetch → useQuery 교체 |
| `routes/__root.tsx` | ErrorBoundary 추가 |
| `main.tsx` | QueryClient 기본 옵션 |
| `stores/knowledge.ts` | persist 미들웨어 적용 |

---

## 5. 위험 및 완화

| 위험 | 완화 |
|------|------|
| `ws-client.ts` 제거 후 다른 곳에서 import | `grep -r "ws-client"` 로 사용처 먼저 확인 |
| Error Boundary가 에러를 삼켜서 디버깅 어려움 | `console.error` + 에러 리포팅 유지 |
| persist 마이그레이션 시 기존 localStorage 키 불일치 | storage version 관리 또는 마이그레이션 함수 |

---

## 6. 성공 기준

- [ ] `use-chat-stream.ts` 파일 삭제
- [ ] 모든 API 호출이 `api` 클라이언트 통과 (raw fetch 제거)
- [ ] 렌더 에러가 전체 UI 크래시 대신 Error Boundary 표시
- [ ] TanStack Query 에러가 자동으로 토스트 표시
- [ ] 모든 store의 persistence 패턴이 persist 미들웨어로 통일
- [ ] 날짜 포맷이 i18n 언어 설정 반영
