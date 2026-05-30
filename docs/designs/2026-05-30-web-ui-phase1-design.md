# Web UI Phase 1: Memory · Seed · Agent · Trace 설계

> **날짜:** 2026-05-30  
> **범위:** Memory UI, Seed Detail, Agent Detail, Agent Execution Trace  
> **목표:** Agent OS의 핵심 데이터 파이프라인(기억→설계→실행→추적)을 Web UI에서 완전히 가시화

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
│   ├── server.rs                 # AppState 정의
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
│   │   ├── infra.rs              # /api/metrics, /api/scheduler, /api/audit, /api/mcp (예약)
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
    │   │   ├── __root.tsx        # 루트 레이아웃
    │   │   ├── index.tsx         # 대시보드
    │   │   ├── chat.tsx
    │   │   ├── agents/$agentId.tsx
    │   │   └── ...
    │   ├── components/
    │   │   ├── ui/               # shadcn/ui 기본 (button, card, tabs, badge...)
    │   │   ├── shared/           # 공유 (data-table, loading, error-state, empty-state)
    │   │   ├── layout/           # app-layout, sidebar, header
    │   │   ├── knowledge/        # Knowledge 앱 전용
    │   │   ├── engine/           # Engine 설정
    │   │   └── {도메인}/          # ← 각 Phase가 여기에 새 디렉토리 생성
    │   ├── hooks/                # TanStack Query 훅
    │   ├── stores/               # Zustand 스토어
    │   ├── types/                # TypeScript 타입
    │   ├── lib/                  # api-client, utils, sse-client, ws-client
    │   └── i18n/                 # 번역 (src/i18n/locales/, public/locales/)
    └── package.json
```

### 코딩 패턴 (반드시 따를 것)

**1. 라우트 패턴:**
```tsx
// routes/memory.tsx
import { createFileRoute } from '@tanstack/react-router'
export const Route = createFileRoute('/memory')({
  component: function MemoryPage() {
    const { data, isLoading, error } = useQuery(...)
    if (isLoading) return <LoadingCards />
    if (error) return <ErrorState error={error} />
    if (!data?.length) return <EmptyState message={t('memory.noData')} />
    return <Content />
  },
})
```

**2. API 훅 패턴:**
```tsx
// hooks/use-memory.ts
export function useMemoryStats() {
  return useQuery({
    queryKey: ['memory', 'stats'],
    queryFn: () => api.get<MemoryStats>('/api/memory/stats'),
    staleTime: 30_000,
  })
}
export function useMemoryDelete() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.delete(`/api/memory/${id}`),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['memory'] }) },
  })
}
```

**3. API 클라이언트:** `import { api } from '@/lib/api-client'` — `api.get`, `api.post`, `api.put`, `api.delete`

**4. i18n:** 모든 UI 텍스트는 `useTranslation()` → `t('key')`. 키는 `public/locales/en/common.json`과 `ko/common.json`에 추가.

**5. 타입:** `types/` 디렉토리에 별도 파일로 분리. `types/index.ts`에 기존 타입 있음.

**6. shadcn/ui 컴포넌트:** 이미 설치됨 — `@/components/ui/tabs`, `badge`, `card`, `button`, `progress`, `dialog`, `select`, `tooltip` 등.

### 참고: 기존 라우트 예시 (agents/index.tsx)

구현 시 기존 라우트 파일을 읽어 패턴을 따를 것. 특히:
- `routes/agents/index.tsx` — 리스트 페이지 패턴 (DataTable, polling, empty/error/loading)
- `routes/cron-jobs.tsx` — CRUD 페이지 패턴 (폼 + 리스트 + mutation)
- `routes/settings.tsx` — 탭 구조 페이지 패턴
- `hooks/use-knowledge.ts` — 29개 TanStack Query 훅 (가장 완성도 높은 참고)

### 병렬 Worktree 전략

```
main 브랜치에서 출발:
  git worktree add ../oxios-p1 -b feature/web-ui-phase1
  git worktree add ../oxios-p2 -b feature/web-ui-phase2
  git worktree add ../oxios-p3 -b feature/web-ui-phase3

각 worktree에서 독립 구현 후:
  1. P1 머지 → main
  2. P2 리베이스(main) → 머지 → main
  3. P3 리베이스(main) → 머지 → main
```

**이 Phase(P1)의 충돌 파일:**
- `surface/oxios-web/src/routes/mod.rs` — P2, P3도 라우트 추가
- `surface/oxios-web/src/routes/workspace.rs` — P2도 엔드포인트 추가
- `surface/oxios-web/web/src/types/index.ts` — P2도 타입 확장

**해결:** 각자 다른 함수/인터페이스를 추가하므로 병합 쉬움. P1을 먼저 머지하면 P2/P3는 그 아래에 추가.

### 사이드바 (Phase 1은 변경 없음)

Phase 1은 기존 사이드바 항목을 변경하지 않습니다. Phase 2(MCP), Phase 3(Agent Groups, A2A)가 항목을 추가합니다.

---

---

## 1. 개요

Phase 1은 Oxios를 "Agent Operating System"으로서 시각적으로 증명하는 4개 영역을 다룹니다:

```
Memory ──→ Seed ──→ Agent ──→ Trace
(기억)      (설계)    (실행)    (추적)
```

현재 이 파이프라인의 각 단계가 UI에서 단절되어 있거나 최소한의 표시만 됩니다.
Phase 1은 이 연결을 완성하고 각 단계의 풍부한 커널 데이터를 UI에 온전히 반영합니다.

### 기존 대비 변화

| 영역 | 현재 | 목표 |
|------|------|------|
| Memory | 검색+리스트만 (75%) | 티어 시각화, Dream 상태, 타입 분포, CRUD (95%) |
| Seed Detail | `<pre>` JSON dump (70%) | Ouroboros 페이즈 뷰, 제약/기준 카드, 진화 체인 (95%) |
| Agent Detail | 키-값 카드 + Kill (80%) | 개별 GET, 상태 타임라인, Seed↔Agent↔Session 링크 (95%) |
| Agent Trace | 없음 (0%) | 툴콜 타임라인, 단계별 입출력, 소요시간, 신뢰도 (95%) |

---

## 2. 백엔드 API 변경

### 2.1 신규 엔드포인트

#### Memory API (`surface/oxios-web/src/routes/workspace.rs`에 추가)

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/memory/stats` | 티어별/타입별 카운트, 총 엔트리 수, Dream 상태 |
| `GET` | `/api/memory/tiers` | 티어별 엔트리 리스트 (쿼리: `?tier=hot\|warm\|cold`) |
| `GET` | `/api/memory/{id}` | 단일 엔트리 상세 (전체 필드) |
| `PUT` | `/api/memory/{id}/pin` | 핀 토글 (`{ pinned: bool }`) |
| `PUT` | `/api/memory/{id}/tier` | 티어 수동 변경 (`{ tier: "hot" }`) |
| `DELETE` | `/api/memory/{id}` | 엔트리 삭제 |
| `GET` | `/api/memory/dream/reports` | Dream 리포트 히스토리 (최근 20개) |
| `GET` | `/api/memory/dream/status` | 현재 Dream 상태 (checkpoint 유무, 마지막 실행 시간) |

**`GET /api/memory/stats` 응답 형태:**
```json
{
  "total": 342,
  "by_tier": { "hot": 48, "warm": 289, "cold": 5 },
  "by_type": {
    "fact": 120, "episode": 85, "knowledge": 45,
    "decision": 32, "skill": 28, "preference": 15,
    "conversation": 12, "session": 5
  },
  "by_protection": { "none": 200, "low": 80, "medium": 45, "high": 15, "permanent": 2 },
  "vector_index_size": 280,
  "dream": {
    "status": "idle",
    "last_run": "2026-05-30T03:00:00Z",
    "last_report_id": "dream-abc123"
  }
}
```

#### Agent API (`surface/oxios-web/src/routes/system.rs`에 추가)

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/agents/{id}` | 단일 에이전트 상세 |
| `GET` | `/api/agents/{id}/trace` | 실행 트레이스 (툴콜 타임라인) |
| `GET` | `/api/agents/{id}/logs` | 에이전트 실행 로그 |

**`GET /api/agents/{id}` 응답 형태:**
```json
{
  "id": "uuid",
  "name": "agent-uuid",
  "status": "running",
  "created_at": "2026-05-30T10:00:00Z",
  "seed_id": "uuid",
  "seed_goal": "Review the codebase...",
  "session_id": "uuid",
  "space_id": "uuid",
  "steps_completed": 12,
  "budget": {
    "tokens_used": 15000,
    "tokens_limit": 50000,
    "cost_used": 0.45,
    "cost_limit": 5.0
  },
  "runtime_config": {
    "model": "anthropic/claude-sonnet-4",
    "max_steps": 50
  }
}
```

**`GET /api/agents/{id}/trace` 응답 형태:**
```json
{
  "agent_id": "uuid",
  "seed_goal": "Review the codebase...",
  "started_at": "2026-05-30T10:00:00Z",
  "completed_at": "2026-05-30T10:02:30Z",
  "total_steps": 12,
  "success": true,
  "steps": [
    {
      "index": 0,
      "tool_call_id": "call_abc",
      "tool_name": "exec",
      "input": "cargo test --workspace",
      "output": "running 142 tests... all passed",
      "duration_ms": 3500,
      "confidence": 0.8,
      "timestamp": "2026-05-30T10:00:15Z"
    }
  ]
}
```

**`GET /api/agents/{id}/logs` 응답 형태:**
```json
{
  "agent_id": "uuid",
  "entries": [
    {
      "timestamp": "2026-05-30T10:00:00Z",
      "level": "info",
      "message": "Agent started, seed goal: Review the codebase"
    },
    {
      "timestamp": "2026-05-30T10:00:01Z",
      "level": "info",
      "message": "Tools registered: 18 tools available"
    }
  ]
}
```

#### Seed API (기존 보강)

| Method | Path | 설명 | 변경 |
|--------|------|------|------|
| `GET` | `/api/seeds/{id}` | Seed 상세 | 응답에 `evaluation`, `phase_reached`, `execution_result` 필드 추가 |
| `GET` | `/api/seeds/{id}/evolution` | 진화 체인 | 이미 존재, 프론트엔드에서 활용 |
| `GET` | `/api/seeds/{id}/agents` | 이 Seed에서 생성된 에이전트 목록 | **신규** |

### 2.2 백엔드 구현 노트

**`system.rs` — 개별 Agent GET:**
```rust
// supervisor.list()에서 필터 대신 supervisor.get(id) 필요
// BasicSupervisor에 get 메서드 추가
async fn get(&self, id: AgentId) -> Result<Option<AgentInfo>>
```

**`agent_runtime.rs` — Agent Trace 영속화:**
```rust
// 현재: trajectory_steps가 SONA 학습용으로만 메모리에 저장됨
// 추가 필요: AgentRuntime 실행 완료 후 trace.json으로 StateStore에 저장
// 경로: sessions/{session_id}/trace.json
// 커널 변경: AgentRuntime::run() 완료 후 save_trace() 호출
```

**`workspace.rs` — Memory API:**
```rust
// 이미 존재하는 MemoryManager 메서드 (그대로 사용):
//   get_by_id(id), forget(id, type), list(type, limit),
//   list_by_tier(tier, limit), search(), semantic_search(),
//   total_entries(), vector_index_size()
//
// MemoryManager에 추가 필요:
//   stats() → 집계 (by_tier, by_type, by_protection, dream 상태)
//     구현: list_by_tier() + list()로 전체 로드 후 집계
//     또는 StateStore에 metadata/stats.json 캐시
//   update_pin(id, pinned) → MemoryEntry.pinned 필드 업데이트 후 저장
//   update_tier(id, tier) → shift_tier() 활용
//
// DreamReport 목록:
//   space_dir/memory/dream_reports/ 디렉토리에서 .json 파일 목록
//   DreamCheckpoint: space_dir/memory/.dream_checkpoint.json 읽기
```

---

## 3. 프론트엔드 설계

### 3.1 Memory Page (`routes/memory.tsx`)

**레이아웃:** 3탭 구조

```
┌─────────────────────────────────────────────────────────────┐
│  Memory                                    [Dream Status]    │
├──────────┬──────────┬──────────┬──────────┬────────────────┤
│ Overview │ Browse   │ Dream    │ Search                     │
├──────────┴──────────┴──────────┴──────────┴────────────────┤
│                                                             │
│  [Tab Content]                                              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### Tab 1: Overview (대시보드)

| 컴포넌트 | 설명 |
|----------|------|
| **티어 도넛 차트** | Hot(빨강) / Warm(노랑) / Cold(파랑) 분포. Recharts PieChart |
| **타입별 바 차트** | MemoryType 9종 분포. Recharts BarChart |
| **보호 레벨 프로그레스** | None→Low→Medium→High→Permanent 스택 바 |
| **Dream 상태 카드** | 마지막 실행 시간, 상태(idle/running), 리포트 링크 |
| **통계 숫자 카드** | 총 엔트리, 벡터 인덱스 크기, 핀된 엔트리 수 |

#### Tab 2: Browse (티어별/타입별 브라우저)

| 컴포넌트 | 설명 |
|----------|------|
| **필터 바** | 티어 드롭다운 (All/Hot/Warm/Cold) + 타입 드롭다운 (All/Fact/Episode/...) |
| **메모리 카드 그리드** | 각 카드: 제목(처음 60자), 타입 배지, 티어 배지, 중요도 바, 보호 레벨 아이콘, 생성일 |
| **카드 클릭** → 메모리 상세 사이드 패널 (또는 모달) |
| **페이지네이션** | 20개씩, 커서 기반 |

**메모리 상세 패널:**
```
┌──────────────────────────────────────┐
│ [Type Badge]  [Tier Badge]  📌 Pin   │
│──────────────────────────────────────│
│ ID: abc-123                          │
│ Source: agent-review                 │
│ Session: sess-456                    │
│ Space: space-789                     │
│ Created: 2026-05-30                  │
│ Importance: ████████░░ 0.82         │
│ Protection: Medium                   │
│ Appearances: 5 sessions              │
│──────────────────────────────────────│
│ ## Content (Markdown)                │
│ The codebase uses...                 │
│                                      │
│ Tags: rust, kernel, memory           │
│──────────────────────────────────────│
│ [Delete]  [Change Tier ▼]            │
└──────────────────────────────────────┘
```

#### Tab 3: Dream

| 컴포넌트 | 설명 |
|----------|------|
| **현재 상태 카드** | idle / running / checkpoint 존재 여부 |
| **Dream 리포트 리스트** | 최근 20개. 각 항목: 날짜, 지속시간, compated/promoted/demoted 수 |
| **리포트 클릭** → 상세 뷰 | entries_before→after, protection 변화, 모순 해결 수, LLM 사용 여부 |

#### Tab 4: Search (기존 기능 개선)

| 변경 | 설명 |
|------|------|
| **검색 모드 전환** | 키워드 / 시맨틱 (HNSW) 토글 |
| **결과에 relevance score** | 시맨틱 검색 시 유사도 점수 표시 |
| **결과 카드 개선** | Overview 탭과 동일한 카드 디자인, 상세 패널 연결 |

### 3.2 Seed Detail Page (`routes/seeds/$seedId.tsx`)

**레이아웃:** 상단-하단 분할

```
┌─────────────────────────────────────────────────────────────┐
│ ← Back to Seeds    Seed #abc-123     [Generation 2]         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ 🎯 Goal                                              │   │
│  │ Build a REST API for user management with auth       │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─── Ouroboros Phase Progress ────────────────────────┐   │
│  │ ● Interview → ● Seed → ● Execute → ◐ Evaluate → ○  │   │
│  │                                              Evolve  │   │
│  │ Phase Reached: Evaluate                            │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─── Constraints ──┐  ┌─── Acceptance Criteria ────┐      │
│  │ • Must use JWT    │  │ ✓ API responds in <100ms   │      │
│  │ • PostgreSQL only │  │ ✓ All endpoints have tests │      │
│  │ • UTF-8 support   │  │ ✗ OpenAPI spec generated   │      │
│  └──────────────────┘  └───────────────────────────┘      │
│                                                             │
│  ┌─── Ontology (Entities) ─────────────────────────────┐   │
│  │ User [data] - Core user entity with auth fields     │   │
│  │ AuthService [service] - JWT token management        │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─── Evaluation Result ───────────────────────────────┐   │
│  │ Mechanical: ✅ Pass   Semantic: ✅ Pass              │   │
│  │ Consensus: ○ N/A     Score: 0.85 / 1.0             │   │
│  │ Notes:                                               │   │
│  │  • All acceptance criteria satisfied                 │   │
│  │  • Test coverage at 92%                              │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─── Evolution Chain ─────────────────────────────────┐   │
│  │ Gen 0 ──→ Gen 1 ──→ Gen 2 (current)                │   │
│  │ [View Gen 0]  [View Gen 1]                          │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─── Linked Agents ───────────────────────────────────┐   │
│  │ 🟢 agent-def456  Running  12 steps  0.45s ago      │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─── Raw Data ────────────────────────────────────────┐   │
│  │ [Expand JSON] (접힌 상태, 클릭 시 펼침)             │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

**주요 컴포넌트:**

| 컴포넌트 | 파일 | 설명 |
|----------|------|------|
| `PhaseProgress` | `components/seed/phase-progress.tsx` | 5단계 인터뷰→시드→실행→평가→발전 프로그레스 바 |
| `ConstraintList` | `components/seed/constraint-list.tsx` | 제약 조건 리스트 |
| `CriteriaList` | `components/seed/criteria-list.tsx` | 수용 기준 + 패스/실패 표시 |
| `EvaluationCard` | `components/seed/evaluation-card.tsx` | 3단계 평가 결과 (Mechanical/Semantic/Consensus) |
| `EvolutionChain` | `components/seed/evolution-chain.tsx` | 세대 체인 내비게이션 |
| `OntologyGrid` | `components/seed/ontology-grid.tsx` | Entity 카드 그리드 |
| `LinkedAgents` | `components/seed/linked-agents.tsx` | 이 Seed에서 생성된 에이전트 목록 |

### 3.3 Agent Detail Page (`routes/agents/$agentId.tsx`)

**레이아웃:** 상단 개요 + 탭

```
┌─────────────────────────────────────────────────────────────┐
│ ← Back to Agents                                           │
├─────────────────────────────────────────────────────────────┤
│  🟢 agent-abc123              [Running]                     │
│  Created: 2026-05-30 10:00   Seed: #def456 →               │
│  Session: sess-xyz          Space: [🔧 Project] →          │
│  Model: claude-sonnet-4     Steps: 12/50                   │
│  Budget: ██████░░░░ 30% (15K/50K tokens, $0.45/$5.00)     │
├──────────┬──────────┬──────────┬───────────────────────────┤
│ Overview │ Trace    │ Logs     │ Sessions                  │
├──────────┴──────────┴──────────┴───────────────────────────┤
│  [Tab Content]                                              │
└─────────────────────────────────────────────────────────────┘
```

#### Tab 1: Overview
- 상태 히스토리 (Starting → Running → ...)
- Seed 정보 카드 (클릭 → Seed Detail)
- Session 정보 카드 (클릭 → Session Detail)
- Budget 사용량 바
- [Kill] 버튼 (확인 다이얼로그 포함)

#### Tab 2: Trace (실행 트레이스)
```
┌─────────────────────────────────────────────────────────────┐
│  Execution Trace                          12 steps · 2m 30s │
├─────────────────────────────────────────────────────────────┤
│  ● Step 1  exec                      3.5s  ████████░░ 0.8  │
│  │   Input:  cargo test --workspace                         │
│  │   Output: running 142 tests... all passed                │
│  │                                                          │
│  ● Step 2  read_file                 0.3s  █████████░ 0.9  │
│  │   Input:  src/main.rs                                    │
│  │   Output: fn main() { ... } (324 lines)                  │
│  │                                                          │
│  ● Step 3  write_file                0.1s  ████████░░ 0.8  │
│  │   Input:  src/lib.rs                                     │
│  │   Output: Successfully wrote 156 lines                   │
│  │                                                          │
│  ○ Step 4  (pending...)                                    │
└─────────────────────────────────────────────────────────────┘
```

#### Tab 3: Logs
- `entries[]`를 순차적으로 표시
- 레벨별 색상 (info=blue, warn=yellow, error=red)
- 타임스탬프 상대 시간

#### Tab 4: Sessions
- 이 에이전트가 참여한 세션 목록
- 클릭 → Session Detail

### 3.4 Agent Execution Trace Page (`routes/agents/$agentId/trace.tsx`)

Trace 탭의 전체 화면 버전. 동일한 컴포넌트 사용, 전체 너비.

---

## 4. 신규 프론트엔드 파일

### 4.1 라우트

| 파일 | 변경 | 설명 |
|------|------|------|
| `routes/memory.tsx` | **재작성** | 3탭 구조로 전면 개편 |
| `routes/seeds/$seedId.tsx` | **재작성** | 구조화된 Ouroboros 뷰 |
| `routes/agents/$agentId.tsx` | **재작성** | 탭 구조 + 상세 정보 |
| `routes/agents/$agentId/trace.tsx` | **신규** | 전체 화면 트레이스 뷰 |

### 4.2 컴포넌트

| 파일 | 설명 |
|------|------|
| `components/memory/memory-overview.tsx` | 통계 대시보드 (도넛/바 차트) |
| `components/memory/memory-browser.tsx` | 티어/타입 필터 + 카드 그리드 |
| `components/memory/memory-card.tsx` | 단일 메모리 카드 |
| `components/memory/memory-detail.tsx` | 메모리 상세 사이드 패널 |
| `components/memory/dream-panel.tsx` | Dream 상태 + 리포트 리스트 |
| `components/memory/dream-report-detail.tsx` | Dream 리포트 상세 뷰 |
| `components/memory/memory-search.tsx` | 키워드/시맨틱 검색 (기존 개선) |
| `components/memory/tier-badge.tsx` | Hot/Warm/Cold 색상 배지 |
| `components/memory/protection-badge.tsx` | 보호 레벨 배지 |
| `components/memory/type-badge.tsx` | 메모리 타입 배지 |
| `components/seed/phase-progress.tsx` | Ouroboros 5단계 프로그레스 |
| `components/seed/constraint-list.tsx` | 제약 조건 리스트 |
| `components/seed/criteria-list.tsx` | 수용 기준 + 패스/실패 |
| `components/seed/evaluation-card.tsx` | 3단계 평가 결과 카드 |
| `components/seed/evolution-chain.tsx` | 세대 체인 내비게이션 |
| `components/seed/ontology-grid.tsx` | Entity 카드 그리드 |
| `components/seed/linked-agents.tsx` | 연결된 에이전트 목록 |
| `components/agent/agent-header.tsx` | 상태 + 메타정보 헤더 |
| `components/agent/agent-budget-bar.tsx` | Budget 사용량 바 |
| `components/agent/execution-trace.tsx` | 툴콜 타임라인 |
| `components/agent/trace-step.tsx` | 개별 트레이스 스텝 |
| `components/agent/agent-logs.tsx` | 로그 뷰어 |

### 4.3 훅

| 파일 | 설명 |
|------|------|
| `hooks/use-memory.ts` | Memory API 훅 (stats, tiers, CRUD, search) |
| `hooks/use-agent-trace.ts` | Agent trace API 훅 |

### 4.4 타입

| 파일 | 설명 |
|------|------|
| `types/memory.ts` | MemoryStats, DreamReport, MemoryDetail 등 |
| `types/seed.ts` | SeedDetail, EvaluationResult, EvolutionEntry 등 |
| `types/agent.ts` | AgentDetail, TraceStep, AgentLog 등 |

### 4.5 i18n 키 (추가분)

`public/locales/en/common.json` 및 `ko/common.json`에 추가:
- Memory 탭 라벨 (overview, browse, dream, search)
- Tier 이름 (hot, warm, cold)
- Protection 레벨 (none, low, medium, high, permanent)
- MemoryType 9종 라벨
- Dream 상태 (idle, running, checkpoint)
- Ouroboros Phase 5종 라벨
- Evaluation 단계 (mechanical, semantic, consensus)
- Trace 관련 (step, tool_name, duration, confidence)

---

## 5. 사이드바 내비게이션 변경

```
common.main
  ├── Dashboard
  ├── Chat

common.agents
  ├── Agents        ← 개선
  ├── Seeds         ← 개선
  ├── Personas
  ├── Skills

common.storage
  ├── Knowledge
  ├── Memory        ← 개편
  ├── Workspace

common.monitor
  ├── Resources
  ├── Scheduler
  ├── Cron Jobs
  ├── Budget
  ├── Security
  ├── Events
  ├── Git
```

변경 없음. 기존 사이드바 항목 그대로. Agent Detail의 Trace는 Agent Detail 페이지 내 탭으로 진입.

---

## 6. 데이터 흐름

### 6.1 Memory 데이터 흐름

```
Kernel (MemoryManager)
  ├── remember() → StateStore → memory/{type}/{id}.json
  ├── list_by_tier() → 필터링된 MemoryEntry[]
  ├── stats() → 집계 (by_tier, by_type, by_protection)
  ├── dream → DreamReport → memory/dream_reports/{id}.json
  └── HNSW index → semantic_search_memory()

Backend API
  GET /api/memory/stats → stats()
  GET /api/memory?tier=hot&type=fact → list_by_tier() + 타입 필터
  GET /api/memory/{id} → get()
  PUT /api/memory/{id}/pin → update()
  DELETE /api/memory/{id} → delete
  GET /api/memory/dream/reports → 리포트 디렉토리 읽기
  POST /api/memory/search → search_memory()
  POST /api/memory/semantic → semantic_search_memory()

Frontend
  useMemoryStats() → Overview 탭 차트
  useMemoryList({ tier, type }) → Browse 탭 카드 그리드
  useMemoryDetail(id) → 상세 패널
  useDreamReports() → Dream 탭 리포트 리스트
  useKnowledgeSearch() (기존) → Search 탭
```

### 6.2 Agent→Seed→Trace 데이터 흐름

```
User 클릭: Agent List → Agent Detail
  → GET /api/agents/{id}
  → seed_id 있으면 Seed 카드 표시 (클릭 → Seed Detail)
  → session_id 있으면 Session 카드 표시 (클릭 → Session Detail)

User 클릭: Seed Detail
  → GET /api/seeds/{id} → 구조화된 Seed 데이터
  → GET /api/seeds/{id}/evolution → 진화 체인
  → GET /api/seeds/{id}/agents → 연결된 에이전트

User 클릭: Agent Detail → Trace 탭
  → GET /api/agents/{id}/trace → 툴콜 타임라인
  → 각 스텝 펼치기/접기
```

---

## 7. 상태 관리

Phase 1에서는 새로운 전역 스토어가 필요하지 않습니다. 모든 데이터는 TanStack Query로 관리:

| 훅 | Query Key | 데이터 |
|----|-----------|--------|
| `useMemoryStats()` | `['memory', 'stats']` | 30s 자동 갱신 |
| `useMemoryList({tier, type})` | `['memory', 'list', tier, type]` | 수동 갱신 |
| `useMemoryDetail(id)` | `['memory', 'detail', id]` | 수동 갱신 |
| `useDreamReports()` | `['memory', 'dream', 'reports']` | 60s |
| `useDreamStatus()` | `['memory', 'dream', 'status']` | 30s |
| `useAgentDetail(id)` | `['agents', 'detail', id]` | 10s |
| `useAgentTrace(id)` | `['agents', 'trace', id]` | 5s (실행 중) / 수동 (완료) |
| `useAgentLogs(id)` | `['agents', 'logs', id]` | 5s (실행 중) |
| `useSeedDetail(id)` | `['seeds', 'detail', id]` | 수동 |
| `useSeedEvolution(id)` | `['seeds', 'evolution', id]` | 수동 |
| `useSeedAgents(id)` | `['seeds', 'agents', id]` | 10s |

---

## 8. 에러 처리

| 시나리오 | 처리 |
|----------|------|
| API 응답 404 (Agent/Seed 없음) | `ErrorState` + "뒤로 가기" 버튼 |
| API 응답 401 (인증 만료) | `useAuthStore.logout()` + 로그인 리다이렉트 |
| Agent가 실행 중일 때 트레이스가 비어있음 | "에이전트가 아직 도구를 호출하지 않았습니다" EmptyState |
| Dream 리포트가 없음 | "아직 Dream이 실행되지 않았습니다" EmptyState |
| Memory stats API 실패 | 기존 `ErrorState` + 재시도 |

---

## 9. 구현 순서

각 단계는 독립적으로 커밋 가능한 단위입니다.

### Step 1: Types & Hooks (기반)
1. `types/memory.ts`, `types/seed.ts`, `types/agent.ts` 작성
2. `hooks/use-memory.ts` 작성
3. `hooks/use-agent-trace.ts` 작성
4. i18n 키 추가 (EN/KO)

### Step 2: Backend API (백엔드)
1. `BasicSupervisor::get(id)` 추가
2. `GET /api/agents/{id}` 엔드포인트
3. `GET /api/agents/{id}/trace` 엔드포인트 (trajectory_steps를 StateStore에 저장하는 로직 포함)
4. `GET /api/agents/{id}/logs` 엔드포인트
5. `GET /api/memory/stats` 엔드포인트
6. `GET /api/memory/tiers` 엔드포인트
7. `GET /api/memory/{id}` 엔드포인트
8. `PUT /api/memory/{id}/pin` 엔드포인트
9. `DELETE /api/memory/{id}` 엔드포인트
10. `GET /api/memory/dream/reports` 엔드포인트
11. `GET /api/memory/dream/status` 엔드포인트
12. `GET /api/seeds/{id}/agents` 엔드포인트
13. Seed 응답 형식 개선 (evaluation, phase_reached 포함)

### Step 3: Memory UI (프론트엔드)
1. 배지 컴포넌트 (`tier-badge`, `protection-badge`, `type-badge`)
2. `memory-overview.tsx` (차트)
3. `memory-browser.tsx` + `memory-card.tsx`
4. `memory-detail.tsx` (사이드 패널)
5. `dream-panel.tsx` + `dream-report-detail.tsx`
6. `memory-search.tsx` (시맨틱 모드 추가)
7. `routes/memory.tsx` 재작성 (3탭 구조)

### Step 4: Seed Detail UI (프론트엔드)
1. `phase-progress.tsx`
2. `constraint-list.tsx` + `criteria-list.tsx`
3. `evaluation-card.tsx`
4. `evolution-chain.tsx`
5. `ontology-grid.tsx`
6. `linked-agents.tsx`
7. `routes/seeds/$seedId.tsx` 재작성

### Step 5: Agent Detail + Trace UI (프론트엔드)
1. `agent-header.tsx` + `agent-budget-bar.tsx`
2. `execution-trace.tsx` + `trace-step.tsx`
3. `agent-logs.tsx`
4. `routes/agents/$agentId.tsx` 재작성 (탭 구조)
5. `routes/agents/$agentId/trace.tsx` 신규 (전체 화면)

### Step 6: 통합 & 테스트
1. Agent↔Seed↔Session 간 네비게이션 링크 확인
2. 실시간 폴링 동작 확인 (실행 중인 에이전트)
3. E2E 테스트 추가 (Memory 통계, Seed Detail, Agent Trace)

---

## 10. 의존성

| 의존 | 설명 |
|------|------|
| Recharts | 이미 설치됨. Memory Overview 차트에 사용 |
| TanStack Query | 이미 사용 중. 모든 데이터 페칭 |
| Zustand | 새 스토어 불필요. 기존 stores 사용 |
| i18next | 키만 추가 |
| Tailwind + shadcn/ui | 기존 컴포넌트 재사용 (Card, Badge, Tabs, Progress) |

---

## 11. Phase 2, 3 미리보기

### Phase 2 (Chat, Workspace, 검색/필터, MCP) — `docs/designs/2026-05-30-web-ui-phase2-design.md`
- Chat: 툴콜 인라인 표시, 에이전트 선택 드롭다운
- Workspace: 파일 미리보기 (CodeMirror), 업로드, 생성, 삭제
- 검색/필터: DataTable에 검색바 + 컬럼 필터 (Agents, Seeds, Sessions, Audit)
- MCP: `/mcp` 페이지 — 서버 등록/관리, 도구 목록, 호출 테스트

### Phase 3 (Budget, Agent Groups, A2A, 테스트) — `docs/designs/2026-05-30-web-ui-phase3-design.md`
- Budget: 에이전트별 한도 편집 모달, 시계열 차트
- Agent Groups: `/agent-groups` 페이지 — 그룹 생성, 멤버 관리
- A2A: `/a2a` 페이지 — 에이전트 간 메시지 모니터, 통신 그래프
- 테스트: Vitest 컴포넌트 테스트, Playwright E2E 확장
- Marketplace: Skills 탭과 중복 해소 (독립 페이지 제거 또는 리다이렉트)
