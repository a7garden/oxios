# Oxios Web UI — TypeScript 마이그레이션 설계

> **상태**: 초안 · **날짜**: 2026-05-20
> **배경**: oxios-markdown을 JS 기반으로 결정하면서, Dioxus(WASM) 프론트엔드를 TS로 전환.

---

## 1. 현재 상태

### 기존 프론트엔드 (Dioxus/WASM)

```
channels/oxios-web/frontend/src/
├── main.rs                          # 앱 진입점 (Dioxus launch)
├── api/mod.rs                       # API 클라이언트 (fetch 래퍼)
├── components/
│   ├── layout.rs                    # 전체 레이아웃
│   ├── sidebar.rs                   # 네비게이션 사이드바 (Panel enum)
│   ├── chat.rs                      # 채팅 컴포넌트
│   ├── icons.rs                     # 아이콘 모음
│   └── settings/                    # 설정 폼 컴포넌트 (13개)
└── views/
    ├── dashboard.rs                 # 대시보드
    ├── chat.rs                      # 채팅 뷰
    ├── agents.rs                    # 에이전트 목록
    ├── sessions.rs                  # 세션 관리
    ├── seeds.rs                     # Ouroboros 시드
    ├── spaces.rs                    # 스페이스 관리
    ├── programs.rs                  # 프로그램 관리
    ├── skills.rs                    # 스킬 관리
    ├── memory.rs                    # 메모리 브라우저
    ├── scheduler.rs                 # 스케줄러
    ├── security.rs                  # 보안/권한
    ├── budget.rs                    # 예산 관리
    ├── resources.rs                 # 리소스 모니터
    ├── events.rs                    # 이벤트 스트림
    ├── approvals.rs                 # HitL 승인
    ├── cron_jobs.rs                 # 크론 작업
    ├── git.rs                       # Git 내역
    ├── personas.rs                  # 페르소나
    ├── agent_groups.rs              # 에이전트 그룹
    ├── host_tools.rs                # 호스트 도구
    ├── workspace.rs                 # 워크스페이스 파일 탐색기
    └── settings/                    # 설정 탭 (7개)
```

### 기존 백엔드 (Axum REST API)

50+ 엔드포인트 — `/api/chat`, `/api/agents`, `/api/seeds`, `/api/spaces`, `/api/sessions`,
`/api/memory`, `/api/config`, `/api/programs`, `/api/scheduler`, `/api/audit`, `/api/budget`,
`/api/cron-jobs`, `/api/approvals`, `/api/git`, `/api/personas`, `/api/resources` 등.

WebSocket: `/api/chat/stream` (실시간 채팅)
SSE: `/api/events` (이벤트 스트림)

---

## 2. 기술 스택 결정

### 결정 원칙

| 원칙 | 설명 |
|------|------|
| **SPA** | Oxios에 Axum 백엔드가 이미 있음. SSR/풀스택 프레임워크 불필요 |
| **최신 버전** | 모든 의존성은 npm에 배포된 최신 안정 버전 사용 |
| **생태계** | 커뮤니티 크기, npm 패키지 호환성, 채용 시장 |
| **DX** | 타입 안전성, 빠른 HMR, 직관적인 API |
| **기존 경험** | React + Mantine 경험 활용, 학습 곡선 최소화 |

### npm에서 확인된 최신 버전 (2026-05-20)

```
Vite                8.0.13     ← Rolldown 통합, Rust 기반 단일 번들러
React               19.2.6     ← Compiler, Actions, use()
@vitejs/plugin-react 6.0.2     ← Oxc 기반, Babel 불필요
TypeScript          6.0.3      ← import defer, expandable hovers
TanStack Router     1.170.4    ← 파일 기반 타입 안전 라우팅
TanStack Query      5.100.11   ← 비동기 상태 관리 표준
Zustand             5.0.13     ← 클라이언트 상태 관리
Tailwind CSS        4.3.0      ← CSS-native 설정
Biome               2.4.15     ← Rust 기반 린트/포맷
Zod                 4.4.3      ← 스키마 검증
```

### 선정 스택

| 영역 | 선택 | 이유 | 대안 검토 |
|------|------|------|-----------|
| **프레임워크** | **React 19.2** | 생태계 #1, 기존 경험, Concurrent Features, React Compiler | Vue 3, Svelte 5, Solid — 생태계/경험 부족 |
| **빌드 도구** | **Vite 8** | Rolldown 통합 (10-30x 빌드 속도). Rust 기반 단일 번들러 | Turbopack (Next.js 종속), Rsbuild (성장 중) |
| **React 플러그인** | **@vitejs/plugin-react v6** | Oxc 기반 React Refresh. Babel 의존성 제거 | — |
| **언어** | **TypeScript 6.0** | 비협상 불가. strict mode. `import defer` 지원 | — |
| **UI 컴포넌트** | **shadcn/ui (CLI v4)** | 2026년 React UI 표준. Radix 프리미티브 기반. 복사-붙여넣기 모델로 완전 제어. Presets/Skills 지원 | Mantine (과거 경험 있으나 shadcn이 더 주류), MUI (무거움), Ant Design (엔터프라이즈) |
| **스타일링** | **Tailwind CSS v4.3** | shadcn/ui의 기반. v4에서 CSS-native 설정 (`@theme`). 유틸리티 퍼스트 | Panda CSS (타입 안전하지만 생태계 작음), CSS Modules (구식) |
| **클라이언트 상태** | **Zustand v5** | 가볍고 단순. 보일러플레이트 최소. 2026년 클라이언트 상태 표준 | Redux Toolkit (과도), Jotai (원자형, 이 프로젝트에 과함) |
| **서버 상태** | **TanStack Query v5** | API 캐싱/동기화/낙관적 업데이트의 업계 표준. WebSocket 연동 | SWR (기능 부족), 수동 fetch |
| **라우팅** | **TanStack Router v1** | 파일 기반 + 타입 안전 라우팅. search params까지 타입 안전 | React Router (타입 안전성 부족), wouter (과도 경량) |
| **폼 처리** | **React Hook Form + Zod v4** | 표준 조합. 런타임 + 컴파일타임 검증 | Formiz, conform (생태계 작음) |
| **아이콘** | **Lucide React** | shadcn/ui 기본 아이콘. Tree-shakable | Heroicons, Phosphor |
| **차트** | **Recharts** | React-native 차트. 대시보드 메트릭 시각화 | Tremor (shadcn 기반이나 제한적), Nivo (과함) |
| **테스트** | **Vitest + Testing Library** | Vite 통합, Jest 호환 API | Playwright (E2E 전용) |
| **패키지 매니저** | **Bun** | 런타임 + 패키지 매니저 + 빌드 도구 통합. npm보다 빠름. `bun --version` 1.3.14 | pnpm, npm, yarn |
| **린트/포맷** | **Biome v2** | Rust 기반 ESLint+Prettier 대체. 매우 빠름 | ESLint + Prettier (느림, 설정 복잡) |

### 왜 Next.js/TanStack Start가 아닌가?

Oxios는 **자체 Axum 백엔드**를 가진 독립적인 에이전트 OS다. 프론트엔드는 이 백엔드에 연결하는 **SPA 클라이언트**일 뿐이다.

- **Next.js**: API Routes, SSR, ISR, Image Optimization 등의 기능이 전혀 필요 없음. 불필요한 복잡성 추가.
- **TanStack Start**: 풀스택 서버 함수 모델. Oxios 백엔드와 중복.
- **Vite SPA**: 필요한 것만. 빌드 결과물은 정적 파일 → Axum의 `tower_http::services::ServeDir`로 서빙.

---

## 3. 프로젝트 구조

```
channels/oxios-web/
├── Cargo.toml                  # Rust 백엔드 (Axum 서버, 기존 코드 유지)
├── src/                        # Rust 백엔드 소스 (기존 routes/, server.rs 등)
│
└── web/                        # ← 새 TypeScript 프론트엔드
    ├── package.json
    ├── bun.lockb                     # ← Bun 네이티브 잠금 파일
    ├── tsconfig.json
    ├── vite.config.ts
    ├── biome.json
    ├── index.html
    │
    ├── public/
    │   └── favicon.svg
    │
    └── src/
        ├── main.tsx                    # 앱 진입점
        ├── app.tsx                     # 루트 컴포넌트 (라우터 프로바이더)
        ├── routes/                     # TanStack Router 파일 기반 라우팅
        │   ├── __root.tsx              # 루트 레이아웃 (사이드바 + 헤더)
        │   ├── dashboard.tsx           # / → 대시보드
        │   ├── chat.tsx                # /chat
        │   ├── agents/
        │   │   ├── index.tsx           # /agents
        │   │   └── $agentId.tsx        # /agents/:id
        │   ├── sessions/
        │   │   ├── index.tsx           # /sessions
        │   │   └── $sessionId.tsx      # /sessions/:id
        │   ├── seeds/
        │   │   ├── index.tsx           # /seeds
        │   │   └── $seedId.tsx         # /seeds/:id
        │   ├── spaces/
        │   │   ├── index.tsx           # /spaces
        │   │   └── $spaceId.tsx        # /spaces/:id
        │   ├── programs.tsx            # /programs
        │   ├── skills.tsx              # /skills
        │   ├── memory.tsx              # /memory
        │   ├── scheduler.tsx           # /scheduler
        │   ├── security.tsx            # /security
        │   ├── budget.tsx              # /budget
        │   ├── resources.tsx           # /resources
        │   ├── events.tsx              # /events
        │   ├── approvals.tsx           # /approvals
        │   ├── cron-jobs.tsx           # /cron-jobs
        │   ├── git.tsx                 # /git
        │   ├── personas.tsx            # /personas
        │   ├── agent-groups.tsx        # /agent-groups
        │   ├── host-tools.tsx          # /host-tools
        │   ├── workspace/
        │   │   ├── index.tsx           # /workspace (파일 트리)
        │   │   └── $filePath.tsx       # /workspace/:path (파일 뷰어/에디터)
        │   └── settings.tsx            # /settings
        │
        ├── components/
        │   ├── ui/                     # shadcn/ui 컴포넌트 (자동 생성)
        │   │   ├── button.tsx
        │   │   ├── card.tsx
        │   │   ├── dialog.tsx
        │   │   ├── input.tsx
        │   │   ├── table.tsx
        │   │   ├── badge.tsx
        │   │   ├── tabs.tsx
        │   │   ├── select.tsx
        │   │   ├── toast.tsx
        │   │   ├── dropdown-menu.tsx
        │   │   ├── sheet.tsx           # 모바일 사이드바
        │   │   ├── skeleton.tsx        # 로딩 스켈레톤
        │   │   └── ...                 # 필요에 따라 추가
        │   │
        │   ├── layout/
        │   │   ├── app-layout.tsx      # 전체 레이아웃
        │   │   ├── sidebar.tsx         # 네비게이션 사이드바
        │   │   ├── header.tsx          # 상단 헤더 (테마 토글, 상태)
        │   │   └── mobile-nav.tsx      # 모바일 네비게이션
        │   │
        │   ├── chat/
        │   │   ├── chat-panel.tsx      # 채팅 메인 패널
        │   │   ├── message-list.tsx    # 메시지 목록
        │   │   ├── message-bubble.tsx  # 개별 메시지
        │   │   ├── chat-input.tsx      # 입력 영역
        │   │   └── streaming-indicator.tsx
        │   │
        │   ├── agents/
        │   │   ├── agent-card.tsx
        │   │   ├── agent-status-badge.tsx
        │   │   └── agent-detail.tsx
        │   │
        │   ├── seeds/
        │   │   ├── seed-card.tsx
        │   │   ├── evolution-timeline.tsx
        │   │   └── spec-viewer.tsx
        │   │
        │   ├── workspace/
        │   │   ├── file-tree.tsx
        │   │   ├── file-viewer.tsx
        │   │   └── code-editor.tsx     # Monaco 또는 CodeMirror
        │   │
        │   ├── memory/
        │   │   ├── memory-table.tsx
        │   │   └── memory-search.tsx
        │   │
        │   ├── settings/
        │   │   ├── settings-tabs.tsx
        │   │   ├── config-form.tsx
        │   │   └── sections/           # 설정 섹션별 폼
        │   │
        │   └── shared/
        │       ├── loading.tsx         # 로딩 상태
        │       ├── error-boundary.tsx  # 에러 바운더리
        │       ├── empty-state.tsx     # 빈 상태
        │       ├── status-indicator.tsx
        │       └── data-table.tsx      # 재사용 데이터 테이블
        │
        ├── lib/
        │   ├── api-client.ts           # API 클라이언트 (fetch 래퍼)
        │   ├── ws-client.ts            # WebSocket 클라이언트 (채팅 스트리밍)
        │   ├── sse-client.ts           # SSE 클라이언트 (이벤트 스트림)
        │   └── utils.ts                # shadcn cn() 유틸 등
        │
        ├── hooks/
        │   ├── use-auth.ts             # 인증 훅
        │   ├── use-theme.ts            # 테마 훅
        │   ├── use-chat-stream.ts      # 채팅 스트리밍 훅
        │   └── use-events.ts           # SSE 이벤트 훅
        │
        ├── stores/
        │   ├── auth.ts                 # Zustand: 인증 상태
        │   ├── theme.ts                # Zustand: 테마 상태
        │   └── sidebar.ts              # Zustand: 사이드바 상태
        │
        └── types/
            ├── api.ts                  # API 응답 타입
            ├── agent.ts                # 에이전트 관련 타입
            ├── seed.ts                 # 시드 관련 타입
            ├── space.ts                # 스페이스 타입
            ├── session.ts              # 세션 타입
            ├── config.ts               # 설정 타입
            └── index.ts                # 리익스포트
```

---

## 4. 아키텍처

### 4.1 전체 구조

```
┌─────────────────────────────────────────────────────────┐
│                    Browser (SPA)                         │
│                                                          │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ TanStack │  │ TanStack     │  │ Zustand          │  │
│  │ Router   │  │ Query v5     │  │ (클라이언트 상태)│  │
│  │          │  │ (서버 상태)  │  │                  │  │
│  └────┬─────┘  └──────┬───────┘  └────────┬─────────┘  │
│       │               │                    │             │
│  ┌────┴───────────────┴────────────────────┴─────────┐  │
│  │              React 19 Components                  │  │
│  │  ┌─────────────────────────────────────────────┐ │  │
│  │  │        shadcn/ui + Tailwind CSS v4          │ │  │
│  │  └─────────────────────────────────────────────┘ │  │
│  └────────────────────┬─────────────────────────────┘  │
│                       │                                  │
│  ┌────────────────────┴─────────────────────────────┐  │
│  │  API Client  │  WS Client  │  SSE Client          │  │
│  └───────┬──────┴──────┬──────┴───────┬─────────────┘  │
└──────────┼─────────────┼──────────────┼─────────────────┘
           │ HTTP        │ WebSocket    │ SSE
┌──────────┴─────────────┴──────────────┴─────────────────┐
│              Axum Backend (Rust)                         │
│  REST API (/api/*)  │  /api/chat/stream  │  /api/events │
└─────────────────────────────────────────────────────────┘
```

### 4.2 데이터 흐름

```
사용자 액션
    │
    ▼
TanStack Router (라우팅 + loader)
    │
    ▼
TanStack Query (캐싱 + 재검증)
    │
    ▼
API Client (fetch + interceptors)
    │  ← 인증 헤더 자동 주입
    │  ← 에러 응답 자동 처리
    ▼
Axum Backend (/api/*)
```

### 4.3 실시간 통신

```
채팅:  Browser ←WebSocket→ /api/chat/stream  → Kernel
이벤트: Browser ←SSE────→ /api/events        → Event Bus
```

---

## 5. 핵심 구현 패턴

### 5.1 API 클라이언트

```typescript
// lib/api-client.ts
const API_BASE = import.meta.env.VITE_API_BASE || '';

interface ApiOptions {
  method?: string;
  body?: unknown;
  params?: Record<string, string>;
}

async function apiClient<T>(path: string, options?: ApiOptions): Promise<T> {
  const url = new URL(`${API_BASE}${path}`, window.location.origin);
  if (options?.params) {
    Object.entries(options.params).forEach(([k, v]) => url.searchParams.set(k, v));
  }

  const token = useAuthStore.getState().token;
  const res = await fetch(url.toString(), {
    method: options?.method ?? 'GET',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: options?.body ? JSON.stringify(options.body) : undefined,
  });

  if (!res.ok) {
    throw new ApiError(res.status, await res.text());
  }
  return res.json() as Promise<T>;
}
```

### 5.2 TanStack Query 연동

```typescript
// hooks/use-agents.ts
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';

export function useAgents() {
  return useQuery({
    queryKey: ['agents'],
    queryFn: () => apiClient<AgentListResponse>('/api/agents'),
    refetchInterval: 5000, // 에이전트 상태 자동 갱신
  });
}

export function useKillAgent() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (agentId: string) =>
      apiClient(`/api/agents/${agentId}/kill`, { method: 'POST' }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['agents'] }),
  });
}
```

### 5.3 채팅 스트리밍

```typescript
// hooks/use-chat-stream.ts
export function useChatStream() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);

  const sendMessage = useCallback(async (content: string) => {
    const ws = new WebSocket(
      `${wsBase}/api/chat/stream?token=${token}`
    );

    ws.onopen = () => {
      ws.send(JSON.stringify({ type: 'message', content }));
      setIsStreaming(true);
    };

    ws.onmessage = (event) => {
      const chunk = JSON.parse(event.data);
      setMessages(prev => appendChunk(prev, chunk));
    };

    ws.onclose = () => setIsStreaming(false);
  }, [token]);

  return { messages, isStreaming, sendMessage };
}
```

### 5.4 설정 폼 (React Hook Form + Zod)

```typescript
// 기존 settings/ 의 폼들이 Zod 스키마로 변환
const configSchema = z.object({
  general: z.object({
    default_model: z.string().min(1),
    max_concurrent_agents: z.number().min(1).max(100),
    workspace_path: z.string().min(1),
  }),
  engine: z.object({
    provider: z.enum(['openai', 'anthropic', 'google', 'ollama']),
    // ...
  }),
});

type Config = z.infer<typeof configSchema>;

function SettingsForm() {
  const { data: config } = useConfig();
  const form = useForm<Config>({ resolver: zodResolver(configSchema), values: config });
  // ...
}
```

---

## 6. 빌드 & 배포

### 6.1 개발

```bash
# 설치
curl -fsSL https://bun.sh/install | bash
# 또는 brew install bun

# 개발 서버 (Vite 번들러 사용, Bun 런타임으로 실행)
bun run dev

# 프로덕션 빌드
bun run build

# 테스트
bun test

# lint
bun run lint
```

### 6.2 프로덕션 빌드

```bash
bun run build        # → web/dist/ (정적 파일)
```

Bun의 빠른 번들링으로 빌드 시간이 크게 단축된다. Vite 8의 Rolldown과 결합하면 기존 Rust 백엔드의 컴파일 시간보다 프론트엔드 빌드가 더 빠를 수 있다.

Axum 서버는 `web/dist/`를 정적 파일로 서빙:

```rust
// 기존과 동일 — tower_http ServeDir로 서빙
// 경로만 channels/oxios-web/web/dist 로 변경
```

### 6.3 Vite 개발 서버 프록시

```typescript
// vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      '/api': 'http://localhost:3000',
      '/health': 'http://localhost:3000',
    },
  },
})
```

---

## 7. 마이그레이션 전략

### Phase 1: 기반 구축
- [ ] `web/` 디렉토리 생성, Vite + React + TS 프로젝트 초기화
- [ ] shadcn/ui (CLI v4), Tailwind CSS v4 설정
- [ ] TanStack Router + Query 설정
- [ ] Zustand 스토어 (auth, theme, sidebar)
- [ ] API 클라이언트, WS 클라이언트
- [ ] 레이아웃 (사이드바 + 헤더 + 라우팅)

### Phase 2: 핵심 뷰
- [ ] 대시보드 (상태 카드 + 메트릭 차트)
- [ ] 채팅 (스트리밍 포함)
- [ ] 에이전트 목록/상세
- [ ] 세션 관리

### Phase 3: 관리 뷰
- [ ] Seeds, Spaces, Programs, Skills
- [ ] Memory, Scheduler, Security
- [ ] Settings (모든 탭)
- [ ] Git, Budget, Resources

### Phase 4: 고급 기능
- [ ] Workspace 파일 탐색기 + 코드 에디터
- [ ] SSE 이벤트 실시간 스트림
- [ ] Approvals (HitL) 인터랙션
- [ ] Cron Jobs 관리
- [ ] 모바일 반응형

### Phase 5: 정리
- [ ] Dioxus 프론트엔드 제거 (`frontend/` 디렉토리 삭제)
- [ ] Axum 정적 파일 서빙 경로 변경
- [ ] CI 파이프라인에 `bun run build` 추가

---

## 8. 의존성 요약

```json
{
  "name": "oxios-web-ui",
  "version": "0.1.0",
  "private": true,
  "license": "MIT",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "test": "vitest",
    "lint": "biome check .",
    "lint:fix": "biome check --write .",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "react": "^19.2",
    "react-dom": "^19.2",
    "@tanstack/react-router": "^1.170",
    "@tanstack/react-query": "^5.100",
    "zustand": "^5.0",
    "react-hook-form": "^7",
    "@hookform/resolvers": "^3",
    "zod": "^4.4",
    "recharts": "^2",
    "lucide-react": "^0.511",
    "class-variance-authority": "^0.7",
    "clsx": "^2",
    "tailwind-merge": "^3"
  },
  "devDependencies": {
    "typescript": "^6.0",
    "vite": "^8.0",
    "@vitejs/plugin-react": "^6.0",
    "tailwindcss": "^4.3",
    "@tailwindcss/vite": "^4.3",
    "@biomejs/biome": "^2.4",
    "vitest": "^3",
    "@testing-library/react": "^16",
    "@tanstack/router-devtools": "^1.170",
    "@tanstack/router-plugin": "^1.170"
  },
  "trustedDependencies": ["@biomejs/biome"]
}
```
