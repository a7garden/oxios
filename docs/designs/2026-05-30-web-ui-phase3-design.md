# Web UI Phase 3: Budget · Agent Groups · A2A · Marketplace · Tests 설계

> **날짜:** 2026-05-30
> **범위:** Budget 편집, Agent Groups 관리, A2A 모니터, Marketplace 정리, 테스트 인프라
> **목표:** 관리/운영 기능 완성 + 테스트 기반 구축으로 프로덕션 준비

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
│   │   ├── budget_routes.rs      # /api/budget (6개 엔드포인트)
│   │   ├── cron_jobs.rs          # /api/cron-jobs
│   │   ├── space_routes.rs       # /api/spaces
│   │   ├── git_routes.rs         # /api/git
│   │   ├── marketplace.rs        # /api/marketplace
│   │   ├── audit_routes.rs       # /api/audit
│   │   ├── resource_routes.rs    # /api/resources
│   │   └── agent_groups.rs       # /api/agent-groups (2개, 읽기 전용)
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
    │   ├── stores/               # Zustand 스토어
    │   ├── types/                # TypeScript 타입
    │   ├── lib/                  # api-client, utils, sse-client, ws-client
    │   ├── i18n/                 # 번역
    │   └── __tests__/            # 단위 테스트 (3개 파일)
    ├── e2e/                      # Playwright E2E (1개 파일, 10개 테스트)
    ├── vitest.config.ts
    └── playwright.config.ts
```

### 코딩 패턴 (반드시 따를 것)

**1. 라우트:** `createFileRoute('/path')({ component: ... })`. 로딩/에러/빈 상태는 항상 처리.

**2. API 훅:** `hooks/use-*.ts` 파일에 TanStack Query 훅. `useQuery`로 읽기, `useMutation`으로 쓰기. mutation 성공 시 `queryClient.invalidateQueries()`.

**3. API 클라이언트:** `import { api } from '@/lib/api-client'` — `api.get<T>(path, params?)`, `api.post<T>(path, body?)`, `api.put<T>(path, body?, raw?)`, `api.delete<T>(path)`.

**4. i18n:** `useTranslation()` → `t('key')`. 키는 `public/locales/en/common.json`과 `ko/common.json`에 추가. 현재 618키.

**5. shadcn/ui:** `@/components/ui/tabs`, `badge`, `card`, `button`, `progress`, `dialog`, `select`, `tooltip`, `input`, `textarea`, `separator`, `skeleton`, `scroll-area`, `switch`, `dropdown-menu`.

### 참고: 이 Phase에서 수정/확장할 기존 파일

반드시 구현 전에 먼저 읽을 것:
- `routes/budget.tsx` — 현재 Budget 페이지 (읽기 전용, 타입 불일치)
- `surface/oxios-web/src/routes/budget_routes.rs` — 백엔드 Budget API (6개 엔드포인트)
- `surface/oxios-web/src/routes/agent_groups.rs` — 백엔드 Agent Groups API (2개, 읽기 전용)
- `crates/oxios-kernel/src/agent_group.rs` — 커널 타입 (OxiosAgentGroup, OxiosGroupAgent)
- `crates/oxios-kernel/src/a2a.rs` — A2A 프로토콜 (A2AMessage, AgentCard, AgentCardRegistry, A2AProtocol)
- `crates/oxios-kernel/src/budget.rs` — 커널 BudgetManager (BudgetLimit, BudgetInfo, BudgetExceeded)
- `routes/marketplace.tsx` — 독립 마켓플레이스 페이지 (Skills 탭과 중복)
- `routes/skills.tsx` — Skills 페이지 (Installed + Marketplace 탭)
- `components/layout/sidebar.tsx` — 사이드바 내비게이션
- `__tests__/` — 기존 단위 테스트 (api-client, stores, utils)
- `e2e/app.spec.ts` — 기존 E2E 테스트

### 커널 타입 요약

**Budget (`crates/oxios-kernel/src/budget.rs`):**
```rust
pub struct BudgetLimit { pub agent_id: AgentId, pub token_budget: u64, pub calls_budget: u64, pub window_secs: u64 }
pub struct BudgetInfo { pub tokens_remaining: u64, pub calls_remaining: u64, pub window_remaining_secs: u64, pub is_exhausted: bool }
```

**Agent Groups (`crates/oxios-kernel/src/agent_group.rs`):**
```rust
pub struct OxiosAgentGroup { pub id: Uuid, pub parent_seed_id: Uuid, pub agents: Vec<OxiosGroupAgent> }
pub struct OxiosGroupAgent { pub id: AgentId, pub seed: Seed, pub status: OxiosAgentGroupStatus, pub result: Option<ExecutionResult> }
pub enum OxiosAgentGroupStatus { Pending, Running, Completed, Failed }
```

**A2A (`crates/oxios-kernel/src/a2a.rs`):**
```rust
pub enum A2AMessage { TaskDelegation{..}, StatusUpdate{..}, ResultSharing{..}, CapabilityQuery{..}, Handshake{..} }
pub struct AgentCard { pub agent_id: Uuid, pub name: String, pub description: String, pub capabilities: Vec<String>, pub skills: Vec<String>, pub status: String }
pub struct AgentCardRegistry { /* in-memory registry */ }
pub struct A2AProtocol { /* send_message, delegate_task, receive_messages, ... */ }
```

### 병렬 Worktree 전략

```
main 브랜치에서 출발:
  git worktree add ../oxios-p3 -b feature/web-ui-phase3
  cd ../oxios-p3
  bun install   # msw, testing-library 의존성 설치 필요
```

**이 Phase(P3)의 충돌 파일:**

| 파일 | 수정 내용 | 충돌 Phase |
|------|----------|----------|
| `src/routes/mod.rs` | A2A 라우트 등록 | P1, P2 도 라우트 추가 |
| `web/src/components/layout/sidebar.tsx` | Agents 그룹에 AgentGroups + Monitor 그룹에 A2A | P2 도 Monitor 그룹에 MCP 추가 |
| `web/src/routes/marketplace.tsx` | 리다이렉트로 변경 | 충돌 없음 |
| `web/src/routes/skills.tsx` | 상세 뷰, 업데이트 체크 추가 | 충돌 없음 |

**해결:** sidebar.tsx는 `navGroups` 배열에서 각기 다른 위치/그룹에 줄을 추가하므로 자동 병합 가능. mod.rs는 서로 다른 라우트 그룹 추가.

### 사이드바 변경 (이 Phase)

`components/layout/sidebar.tsx`의 `navGroups` 배열에 2개 항목 추가:

```tsx
// common.agents 그룹에 추가 (Seeds 앞에):
{ labelKey: 'common.agentGroups', href: '/agent-groups', icon: <Users className="h-4 w-4" /> },

// common.monitor 그룹에 추가 (Git 뒤에):
{ labelKey: 'common.a2aMonitor', href: '/a2a', icon: <Network className="h-4 w-4" /> },
```

참고: Phase 2가 같은 monitor 그룹에 MCP Servers를 추가합니다. P2 머지 후 리베이스하면 두 항목 모두 포함.

### Phase 2 DataTable 의존

Phase 2가 `components/shared/data-table.tsx`를 재작성합니다 (검색/필터/정렬/페이지네이션). 이 Phase의 Budget/Agent Groups 리스트에서 사용합니다.

- **P2가 먼저 머지된 경우:** 재작성된 DataTable을 그대로 사용.
- **P3가 먼저 구현되는 경우:** 기존 DataTable로 구현. P2 머지 후 DataTable props만 업데이트.

---

---

## 1. 개요

Phase 3은 관리자·운영자 관점의 기능과 테스트 인프라를 다룹니다:

```
Budget     Agent Groups   A2A Monitor   Marketplace   Tests
(예산)       (그룹)          (통신)         (마켓)       (품질)
```

### 기존 대비 변화

| 영역 | 현재 | 목표 |
|------|------|------|
| Budget | 읽기 전용 카드, 타입 불일치 (80%) | 한도 편집, 리셋, 타입 정렬 (95%) |
| Agent Groups | API만 존재, UI 없음 (0%) | 그룹 목록, 에이전트 진행 상태 (90%) |
| A2A | 내부 프로토콜만, 가시성 없음 (0%) | 에이전트 토폴로지, 메시지 흐름 (85%) |
| Marketplace | Skills 탭과 중복 (85%) | 중복 제거, 업데이트 체크, 상세 뷰 (95%) |
| Tests | 단위 3개, E2E 10개 (15%) | 컴포넌트 + 통합 + E2E 확장 (70%) |

---

## 2. Budget 개편

### 2.1 현재 문제점

1. **타입 불일치**: 프론트엔드 `Budget` 타입(`tokens_used`, `tokens_limit`, `cost_used`, `cost_limit`)이 백엔드 응답(`tokens_remaining`, `calls_remaining`, `window_remaining_secs`, `is_exhausted`)과 불일치
2. **비용 추적 없음**: 백엔드에 달러 비용 추적 없음 (토큰/호출 카운트만)
3. **편집 불가**: 한도 설정/수정 UI 없음 (백엔드 POST 존재)
4. **리셋/삭제 불가**: UI에 리셋/삭제 버튼 없음 (백엔드 엔드포인트 존재)

### 2.2 백엔드 변경

#### 타입 정렬

`budget_routes.rs`의 응답을 프론트엔드가 이해하기 쉬운 형태로 변경:

**`GET /api/budget` 응답 (개선):**
```json
{
  "agents": [
    {
      "agent_id": "uuid",
      "budget": {
        "token_limit": 50000,
        "tokens_used": 15000,
        "tokens_remaining": 35000,
        "calls_limit": 100,
        "calls_used": 23,
        "calls_remaining": 77,
        "window_secs": 3600,
        "window_remaining_secs": 2847,
        "is_exhausted": false
      }
    }
  ],
  "summary": {
    "total_agents": 5,
    "total_tokens_used": 75000,
    "total_tokens_limit": 250000,
    "exhausted_agents": 1
  }
}
```

**`GET /api/budget/{agent_id}` 응답 (개선):**
```json
{
  "agent_id": "uuid",
  "budget": {
    "token_limit": 50000,
    "tokens_used": 15000,
    "tokens_remaining": 35000,
    "calls_limit": 100,
    "calls_used": 23,
    "calls_remaining": 77,
    "window_secs": 3600,
    "window_remaining_secs": 2847,
    "is_exhausted": false
  }
}
```

변경 없는 엔드포인트 (기존 그대로):
- `POST /api/budget/{agent_id}` — 한도 설정
- `DELETE /api/budget/{agent_id}` — 한도 제거
- `POST /api/budget/{agent_id}/reserve` — 토큰 예약
- `POST /api/budget/{agent_id}/reset` — 윈도우 리셋

### 2.3 프론트엔드

#### 레이아웃

```
┌─────────────────────────────────────────────────────────────┐
│  Budget                                    [+ Set Budget]    │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─── Summary ──────────────────────────────────────────┐   │
│  │ Total Agents: 5   Tokens: 75K/250K   Exhausted: 1   │   │
│  │ ████████████░░░░░░░░░░░░░░  30%                      │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌─── Agent Budgets ────────────────────────────────────┐   │
│  │                                                      │   │
│  │  🟢 agent-abc123                                     │   │
│  │  Tokens: ████████░░░░ 15K/50K (30%)                  │   │
│  │  Calls:  ████████████████░░░░ 23/100 (23%)          │   │
│  │  Window: 47m remaining                               │   │
│  │  [Edit Limit] [Reset] [Remove]                       │   │
│  │                                                      │   │
│  │  🔴 agent-def456                                     │   │
│  │  Tokens: ██████████████████████ 50K/50K (100%)      │   │
│  │  Calls:  ██████████████████████ 100/100 (100%)      │   │
│  │  Window: EXHAUSTED                                   │   │
│  │  [Edit Limit] [Reset] [Remove]                       │   │
│  │                                                      │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

#### 한도 편집 다이얼로그

```
┌──────────────────────────────────────────┐
│ Set Budget for agent-abc123               │
│──────────────────────────────────────────│
│ Token Limit:  [50000                  ]  │
│ Call Limit:   [100                    ]  │
│ Window (sec): [3600                   ]  │
│                                          │
│        [Cancel]  [Save]                  │
└──────────────────────────────────────────┘
```

### 2.4 신규/수정 파일

| 파일 | 변경 | 설명 |
|------|------|------|
| `routes/budget.tsx` | 재작성 | 요약 + 에이전트별 카드 + 편집/리셋/삭제 |
| `components/budget/budget-summary.tsx` | 신규 | 전체 요약 카드 |
| `components/budget/agent-budget-card.tsx` | 신규 | 에이전트별 예산 카드 |
| `components/budget/set-budget-dialog.tsx` | 신규 | 한도 설정 다이얼로그 |
| `hooks/use-budget.ts` | 신규 | Budget CRUD 훅 |
| `types/budget.ts` | 신규 | BudgetInfo, BudgetLimit 타입 (백엔드 정렬) |

---

## 3. Agent Groups 관리

### 3.1 백엔드 변경

#### 기존 API (읽기 전용)

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/agent-groups` | 그룹 목록 |
| `GET` | `/api/agent-groups/{id}` | 그룹 상세 |

#### 응답 형태 (백엔드에서 OxiosAgentGroup 직렬화)

```json
{
  "id": "uuid",
  "parent_seed_id": "uuid",
  "status": "Running",
  "agents": [
    {
      "id": "uuid",
      "seed": {
        "id": "uuid",
        "goal": "Implement user auth module",
        "generation": 1
      },
      "status": "Completed",
      "result": {
        "output": "Auth module implemented...",
        "success": true
      }
    },
    {
      "id": "uuid",
      "seed": {
        "id": "uuid",
        "goal": "Write tests for auth module",
        "generation": 1
      },
      "status": "Running",
      "result": null
    }
  ],
  "created_at": "2026-05-30T10:00:00Z"
}
```

#### 신규 엔드포인트

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/agent-groups/{id}/progress` | 실시간 진행률 (completion_pct, 완료/실패 카운트) |

**응답:**
```json
{
  "id": "uuid",
  "status": "Running",
  "total_agents": 4,
  "completed": 2,
  "failed": 0,
  "pending": 1,
  "running": 1,
  "completion_pct": 50.0,
  "combined_results": ["Auth module implemented...", "..."]
}
```

**참고:** Agent Groups는 커널 내부에서 생성됩니다 (Seed-split). 수동 생성 엔드포인트는 제공하지 않습니다. UI는 모니터링 전용입니다.

### 3.2 프론트엔드

#### 그룹 목록 페이지

```
┌─────────────────────────────────────────────────────────────┐
│  Agent Groups                                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─── Group abc-123 ────────────────────────────────────┐   │
│  │  Parent Seed: #def-456 "Build full-stack app"        │   │
│  │  Status: 🟢 Running                                  │   │
│  │  Progress: ██████████░░░░░░░░ 50% (2/4 completed)    │   │
│  │  Created: 2026-05-30 10:00                           │   │
│  │  [View Detail →]                                     │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌─── Group xyz-789 ────────────────────────────────────┐   │
│  │  Parent Seed: #ghi-012 "Refactor codebase"           │   │
│  │  Status: ✅ Completed                                 │   │
│  │  Progress: ██████████████████████ 100% (3/3)         │   │
│  │  Created: 2026-05-29 15:30                           │   │
│  │  [View Detail →]                                     │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

#### 그룹 상세 페이지

```
┌─────────────────────────────────────────────────────────────┐
│ ← Back to Agent Groups                                      │
├─────────────────────────────────────────────────────────────┤
│  Group #abc-123           🟢 Running    50% (2/4)           │
│  Parent Seed: #def-456 "Build full-stack app" →            │
│  Created: 2026-05-30 10:00                                  │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─── Sub-Agents ───────────────────────────────────────┐   │
│  │                                                      │   │
│  │  ✅ agent-sub1    Completed    "Implement user auth"  │   │
│  │     Result: Auth module implemented with JWT... →    │   │
│  │                                                      │   │
│  │  ✅ agent-sub2    Completed    "Write auth tests"    │   │
│  │     Result: 42 tests passing... →                   │   │
│  │                                                      │   │
│  │  🟢 agent-sub3    Running      "Build API routes"   │   │
│  │     Steps: 8/50   Budget: 40%                       │   │
│  │                                                      │   │
│  │  ⏳ agent-sub4    Pending      "Setup CI pipeline"  │   │
│  │                                                      │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌─── Combined Results ─────────────────────────────────┐   │
│  │  [View Combined Output]                              │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 3.3 사이드바 내비게이션 변경

```
common.agents
  ├── Agents
  ├── Agent Groups    ← 신규 (Users 아이콘)
  ├── Seeds
  ├── Personas
  ├── Skills
```

### 3.4 신규 파일

| 파일 | 설명 |
|------|------|
| `routes/agent-groups/index.tsx` | 그룹 목록 |
| `routes/agent-groups/$groupId.tsx` | 그룹 상세 |
| `components/agent-group/group-card.tsx` | 그룹 요약 카드 |
| `components/agent-group/group-progress.tsx` | 진행률 바 |
| `components/agent-group/sub-agent-list.tsx` | 하위 에이전트 리스트 |
| `hooks/use-agent-groups.ts` | Agent Groups API 훅 |
| `types/agent-group.ts` | AgentGroup, GroupAgent, GroupStatus 타입 |

---

## 4. A2A 모니터

### 4.1 백엔드 변경

A2A는 현재 내부 전용 프로토콜입니다. 가시성을 위한 관찰 API를 추가합니다.

#### 신규 엔드포인트 (`routes/a2a.rs` — 신규 파일)

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/a2a/agents` | 등록된 에이전트 카드 목록 |
| `GET` | `/api/a2a/agents/{id}` | 에이전트 카드 상세 |
| `GET` | `/api/a2a/messages` | 최근 A2A 메시지 로그 (최근 100개) |
| `GET` | `/api/a2a/topology` | 에이전트 간 통신 토폴로지 |

**`GET /api/a2a/agents` 응답:**
```json
{
  "agents": [
    {
      "agent_id": "uuid",
      "name": "agent-review",
      "description": "Code review agent",
      "capabilities": ["code_analysis", "review"],
      "skills": ["code-review"],
      "status": "active",
      "endpoint": "local"
    }
  ]
}
```

**`GET /api/a2a/messages` 응답:**
```json
{
  "messages": [
    {
      "request_id": "uuid",
      "from_agent": "agent-review",
      "to_agent": "agent-test",
      "message_type": "TaskDelegation",
      "payload_summary": "Run tests for reviewed code",
      "accepted": true,
      "timestamp": "2026-05-30T10:00:15Z"
    }
  ]
}
```

**`GET /api/a2a/topology` 응답:**
```json
{
  "nodes": [
    { "id": "agent-review", "label": "Code Review", "status": "active" },
    { "id": "agent-test", "label": "Test Runner", "status": "active" }
  ],
  "edges": [
    { "from": "agent-review", "to": "agent-test", "type": "TaskDelegation", "count": 5 },
    { "from": "agent-test", "to": "agent-review", "type": "ResultSharing", "count": 4 }
  ]
}
```

#### 백엔드 구현 노트

```rust
// A2A 메시지 로깅: AgentCardRegistry에 메시지 로그 추가
// 또는 이벤트 버스에 A2A 이벤트를 게시하여 수집
//
// AgentLifecycleManager에서 A2A 메시지 전송 시 이벤트 발행:
// event_bus.publish(KernelEvent::A2AMessage { from, to, message_type })
//
// A2A API는 이 로그를 읽어 반환
```

### 4.2 프론트엔드

#### 레이아웃: 3탭

```
┌─────────────────────────────────────────────────────────────┐
│  A2A Protocol Monitor                                        │
├──────────┬──────────┬──────────────────────────────────────┤
│ Topology │ Messages │ Agents                                │
├──────────┴──────────┴──────────────────────────────────────┤
│  [Tab Content]                                              │
└─────────────────────────────────────────────────────────────┘
```

#### Tab 1: Topology (토폴로지 그래프)

```
┌─────────────────────────────────────────────────────────────┐
│                                                              │
│           ┌──────────┐   TaskDelegation (5)                  │
│           │  Review   │ ──────────────────→                  │
│           │  Agent    │                    ┌──────────┐      │
│           └──────────┘   ResultSharing (4) │   Test    │     │
│              │    ↑  ←──────────────────── │   Agent   │     │
│              │    │                        └──────────┘      │
│              │    │                                          │
│    Status    │    │ Handshake (1)                             │
│    Query (2) │    │                        ┌──────────┐      │
│              ↓    └────────────────────── │  Deploy   │     │
│           ┌──────────┐                    │  Agent    │     │
│           │   Main    │                    └──────────┘      │
│           │  Agent    │                                       │
│           └──────────┘                                       │
│                                                              │
│  ── TaskDelegation  ── ResultSharing  ── StatusUpdate       │
└─────────────────────────────────────────────────────────────┘
```

SVG 기반 그래프. Recharts는 그래프 타입이 없으므로:
- 간단한 force-directed 레이아웃을 직접 구현 (지식 그래프 `link-graph.tsx` 패턴)
- 또는 `react-force-graph-2d` 경량 라이브러리 사용

#### Tab 2: Messages (메시지 로그)

```
┌──────────────────────────────────────────────────────────────┐
│  [🔍 검색...] [타입 ▼ All] [시간 범위 ▼ 최근 1시간]          │
│──────────────────────────────────────────────────────────────│
│  10:00:15  Review → Test    TaskDelegation  ✅ Accepted      │
│  10:02:30  Test → Review    ResultSharing   ✅ Accepted      │
│  10:02:31  Review → Deploy  TaskDelegation  ✅ Accepted      │
│  10:05:00  Deploy → Review  StatusUpdate    ✅ Accepted      │
│  10:08:15  Review → Main    ResultSharing   ⏳ Pending       │
└──────────────────────────────────────────────────────────────┘
```

메시지 클릭 → 상세: 전체 페이로드, 응답, 타임스탬프

#### Tab 3: Agents (등록된 에이전트 카드)

```
┌──────────────────────────────────────────────────────────────┐
│  ┌─── Code Review Agent ──────────────────────────────┐      │
│  │  ID: agent-review     Status: 🟢 Active            │      │
│  │  Capabilities: code_analysis, review               │      │
│  │  Skills: code-review                               │      │
│  │  Messages sent: 12   Messages received: 8          │      │
│  └────────────────────────────────────────────────────┘      │
│                                                              │
│  ┌─── Test Runner Agent ──────────────────────────────┐      │
│  │  ID: agent-test       Status: 🟢 Active            │      │
│  │  Capabilities: testing, ci                         │      │
│  │  Skills: test-runner                               │      │
│  │  Messages sent: 8    Messages received: 12         │      │
│  └────────────────────────────────────────────────────┘      │
└──────────────────────────────────────────────────────────────┘
```

### 4.3 사이드바 내비게이션 변경

```
common.monitor
  ├── ...
  ├── Git
  ├── MCP Servers
  ├── A2A Monitor        ← 신규 (Network 아이콘)
```

### 4.4 신규 파일

| 파일 | 설명 |
|------|------|
| `routes/a2a.tsx` | A2A 모니터 페이지 (3탭) |
| `components/a2a/topology-graph.tsx` | 에이전트 토폴로지 SVG 그래프 |
| `components/a2a/message-log.tsx` | A2A 메시지 로그 테이블 |
| `components/a2a/message-detail.tsx` | 메시지 상세 패널 |
| `components/a2a/agent-card-list.tsx` | 등록된 에이전트 카드 리스트 |
| `hooks/use-a2a.ts` | A2A API 훅 |
| `types/a2a.ts` | AgentCard, A2AMessage, Topology 타입 |

---

## 5. Marketplace 정리

### 5.1 중복 제거

**문제:** `routes/marketplace.tsx`와 `routes/skills.tsx`의 Marketplace 탭이 거의 동일

**해결:** 독립 `/marketplace` 라우트를 Skills 페이지로 리다이렉트

```typescript
// routes/marketplace.tsx → 리다이렉트
import { createFileRoute, Navigate } from '@tanstack/react-router'

export const Route = createFileRoute('/marketplace')({
  component: () => <Navigate to="/skills" search={{ tab: 'marketplace' }} />,
})
```

### 5.2 Skills 페이지 개선

**신규 기능:**

| 기능 | 설명 |
|------|------|
| 스킬 상세 뷰 | 카드 클릭 → 사이드 패널 (설명, 요구사항, 설치 방식, 버전) |
| 업데이트 체크 | "업데이트 가능" 배지 + `GET /api/marketplace/updates` 연동 |
| 스킬 비활성화 | 활성/비활성 토글 (기존 API: `POST /api/skills/{name}/enable`) |
| 스킬 삭제 | 삭제 버튼 (기존 API: `DELETE /api/skills/{name}`) |
| 마켓플레이스 상세 | 검색 결과 카드 클릭 → `GET /api/marketplace/skills/{slug}` 상세 |

### 5.3 신규/수정 파일

| 파일 | 변경 | 설명 |
|------|------|------|
| `routes/marketplace.tsx` | 수정 | `/skills?tab=marketplace`로 리다이렉트 |
| `routes/skills.tsx` | 수정 | 상세 뷰, 업데이트 체크, 토글, 삭제 추가 |
| `components/skills/skill-detail.tsx` | 신규 | 스킬 상세 사이드 패널 |
| `components/skills/marketplace-detail.tsx` | 신규 | 마켓플레이스 스킬 상세 |
| `components/skills/update-badge.tsx` | 신규 | 업데이트 가능 배지 |

---

## 6. 테스트 인프라

### 6.1 현재 상태

| 유형 | 파일 수 | 커버리지 |
|------|--------|----------|
| Vitest 단위 | 3 (api-client, stores, utils) | 기본 유틸만 |
| Playwright E2E | 1 (10개 테스트) | 기본 네비게이션 |
| 컴포넌트 테스트 | 0 | 없음 |
| 통합 테스트 | 0 | 없음 |

### 6.2 목표 테스트 구조

```
web/src/__tests__/
├── unit/
│   ├── api-client.test.ts         ← 기존
│   ├── stores.test.ts             ← 기존
│   ├── utils.test.ts              ← 기존
│   ├── memory-types.test.ts       ← 신규: 타입 검증
│   └── format.test.ts             ← 신규: 포맷 유틸
│
├── components/
│   ├── memory/
│   │   ├── tier-badge.test.tsx
│   │   ├── memory-card.test.tsx
│   │   └── memory-detail.test.tsx
│   ├── seed/
│   │   ├── phase-progress.test.tsx
│   │   └── evaluation-card.test.tsx
│   ├── agent/
│   │   ├── execution-trace.test.tsx
│   │   └── agent-budget-bar.test.tsx
│   ├── chat/
│   │   └── tool-call-card.test.tsx
│   ├── workspace/
│   │   └── file-breadcrumb.test.tsx
│   ├── mcp/
│   │   └── tool-tester.test.tsx
│   └── shared/
│       └── data-table.test.tsx
│
└── hooks/
    ├── use-memory.test.ts
    └── use-budget.test.ts

web/e2e/
├── app.spec.ts                    ← 기존 (확장)
├── memory.spec.ts                 ← 신규: Memory 페이지
├── agents.spec.ts                 ← 신규: Agent Detail + Trace
├── seeds.spec.ts                  ← 신규: Seed Detail
├── chat.spec.ts                   ← 신규: Chat + Tool Calls
├── workspace.spec.ts              ← 신규: Workspace CRUD
├── mcp.spec.ts                    ← 신규: MCP 관리
├── budget.spec.ts                 ← 신규: Budget 편집
└── navigation.spec.ts             ← 신규: 전체 네비게이션
```

### 6.3 테스트 전략

#### 단위 테스트 (Vitest + Testing Library)

**컴포넌트 테스트 패턴:**
```typescript
// tier-badge.test.tsx 예시
import { render, screen } from '@testing-library/react'
import { TierBadge } from '@/components/memory/tier-badge'

describe('TierBadge', () => {
  it('renders hot tier with red styling', () => {
    render(<TierBadge tier="hot" />)
    expect(screen.getByText('Hot')).toHaveClass('bg-red-500')
  })

  it('renders warm tier with yellow styling', () => {
    render(<TierBadge tier="warm" />)
    expect(screen.getByText('Warm')).toHaveClass('bg-amber-500')
  })

  it('renders cold tier with blue styling', () => {
    render(<TierBadge tier="cold" />)
    expect(screen.getByText('Cold')).toHaveClass('bg-blue-500')
  })
})
```

**훅 테스트 패턴:**
```typescript
// use-memory.test.ts 예시
// MSW (Mock Service Worker)로 API 모킹
import { renderHook, waitFor } from '@testing-library/react'
import { useMemoryStats } from '@/hooks/use-memory'

describe('useMemoryStats', () => {
  it('fetches and returns memory stats', async () => {
    const { result } = renderHook(() => useMemoryStats())
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.total).toBeGreaterThan(0)
  })
})
```

#### E2E 테스트 (Playwright)

**확장된 E2E 패턴:**
```typescript
// memory.spec.ts 예시
import { test, expect } from '@playwright/test'

test.describe('Memory Page', () => {
  test('shows overview tab with charts', async ({ page }) => {
    await page.goto('/memory')
    await expect(page.getByText('Overview')).toBeVisible()
    await expect(page.getByText('Browse')).toBeVisible()
    await expect(page.getByText('Dream')).toBeVisible()
  })

  test('switches to browse tab and filters by tier', async ({ page }) => {
    await page.goto('/memory')
    await page.getByText('Browse').click()
    await page.getByRole('combobox', { name: /tier/i }).selectOption('hot')
    // Verify filtered results
  })
})
```

### 6.4 테스트 의존성

| 패키지 | 용도 | 설치 필요 |
|--------|------|----------|
| `msw` | API 모킹 (훅/컴포넌트 테스트) | ✅ |
| `@testing-library/react` | 컴포넌트 테스트 | 이미 설치됨 |
| `@testing-library/jest-dom` | DOM 매처 | ✅ |
| `@testing-library/user-event` | 사용자 인터랙션 | ✅ |
| `@playwright/test` | E2E 테스트 | 이미 설치됨 |

### 6.5 CI 통합

`package.json` scripts에 추가:
```json
{
  "test": "vitest run",
  "test:watch": "vitest",
  "test:coverage": "vitest run --coverage",
  "test:e2e": "playwright test",
  "test:all": "bun run test && bun run test:e2e"
}
```

---

## 7. 사이드바 최종 구조 (Phase 3 완료 후)

```
Oxios
├── Main
│   ├── Dashboard
│   └── Chat
│
├── Agents
│   ├── Agents
│   ├── Agent Groups      ← Phase 3 신규
│   ├── Seeds
│   ├── Personas
│   └── Skills
│
├── Storage
│   ├── Knowledge
│   ├── Memory
│   └── Workspace
│
├── Monitor
│   ├── Resources
│   ├── Scheduler
│   ├── Cron Jobs
│   ├── Budget
│   ├── Security
│   ├── Events
│   ├── Git
│   ├── MCP Servers       ← Phase 2 신규
│   └── A2A Monitor       ← Phase 3 신규
│
├── [Theme Toggle]
└── [Settings]
```

---

## 8. 데이터 흐름

### 8.1 Budget

```
Kernel (BudgetManager)
  ├── set_budget(BudgetLimit)
  ├── remaining(agent_id) → BudgetInfo
  ├── reserve(agent_id, tokens)
  ├── reset_window(agent_id)
  └── remove_budget(agent_id)

Backend API
  GET /api/budget → 모든 에이전트 예산 정보
  GET /api/budget/{agent_id} → 단일 에이전트
  POST /api/budget/{agent_id} → 한도 설정
  DELETE /api/budget/{agent_id} → 한도 제거
  POST /api/budget/{agent_id}/reset → 윈도우 리셋

Frontend
  useBudgetList() → 요약 + 에이전트별 카드
  useBudgetSet() → 한도 설정 mutation
  useBudgetDelete() → 한도 삭제 mutation
  useBudgetReset() → 리셋 mutation
```

### 8.2 Agent Groups

```
Kernel (OxiosAgentGroup)
  ├── StateStore에 저장: agent_groups/{id}.json
  ├── new(parent_seed, subtask_descriptions) → 내부에서만 생성
  ├── completion_pct(), combined_results()
  └── all_completed(), any_failed()

Backend API
  GET /api/agent-groups → list_category("agent_groups")
  GET /api/agent-groups/{id} → load_json()
  GET /api/agent-groups/{id}/progress → 진행률 집계

Frontend
  useAgentGroups() → 그룹 카드 리스트
  useAgentGroupDetail(id) → 상세 + 하위 에이전트
  useAgentGroupProgress(id) → 실시간 진행률 (5s 폴링)
```

### 8.3 A2A

```
Kernel (A2AProtocol)
  ├── AgentCardRegistry → in-memory 에이전트 카드 등록
  ├── send_message() → A2ARequest 전송
  ├── receive_messages() → 메시지 수신
  └── 이벤트 버스에 A2A 메시지 로그 게시

Backend API (신규)
  GET /api/a2a/agents → 레지스트리 스냅샷
  GET /api/a2a/messages → 최근 메시지 로그
  GET /api/a2a/topology → 노드+엣지 그래프

Frontend
  useA2AAgents() → 등록된 에이전트
  useA2AMessages() → 메시지 로그 (10s 폴링)
  useA2ATopology() → 토폴로지 그래프
```

---

## 9. 구현 순서

### Step 1: Budget (타입 정렬 + 편집 UI)
1. `types/budget.ts` 작성 (백엔드 응답 형태에 맞춤)
2. `hooks/use-budget.ts` 작성
3. `budget_routes.rs` 응답 형태 개선
4. `budget-summary.tsx`, `agent-budget-card.tsx`, `set-budget-dialog.tsx`
5. `routes/budget.tsx` 재작성

### Step 2: Agent Groups (모니터링 UI)
1. `types/agent-group.ts` 작성
2. `hooks/use-agent-groups.ts` 작성
3. 백엔드: `GET /api/agent-groups/{id}/progress` 추가
4. `group-card.tsx`, `group-progress.tsx`, `sub-agent-list.tsx`
5. `routes/agent-groups/index.tsx`, `routes/agent-groups/$groupId.tsx`
6. 사이드바에 Agent Groups 추가

### Step 3: A2A (관찰 API + 모니터 UI)
1. 백엔드: A2A 이벤트 로깅 (이벤트 버스 활용)
2. `routes/a2a.rs` 신규 (3개 GET 엔드포인트)
3. `routes/mod.rs`에 A2A 라우트 등록
4. `types/a2a.ts`, `hooks/use-a2a.ts`
5. `topology-graph.tsx`, `message-log.tsx`, `agent-card-list.tsx`
6. `routes/a2a.tsx`
7. 사이드바에 A2A Monitor 추가

### Step 4: Marketplace 정리
1. `routes/marketplace.tsx` → 리다이렉트
2. `routes/skills.tsx`에 상세 뷰, 업데이트 체크, 토글, 삭제 추가
3. `skill-detail.tsx`, `marketplace-detail.tsx`, `update-badge.tsx`

### Step 5: 테스트 인프라
1. 테스트 의존성 설치 (`msw`, `@testing-library/jest-dom`, `@testing-library/user-event`)
2. MSW 핸들러 설정 (API 모킹)
3. 핵심 컴포넌트 단위 테스트 (배지, 카드, 프로그레스)
4. 핵심 훅 테스트 (useMemory, useBudget)
5. E2E 테스트 확장 (Memory, Agents, Chat, Workspace)
6. CI 스크립트에 `test:all` 추가

---

## 10. 전체 의존성 요약 (Phase 1-3)

| 패키지 | Phase | 용도 |
|--------|-------|------|
| `@codemirror/view` + 관련 언어 팩 | Phase 2 | Workspace 파일 뷰어/에디터 |
| `msw` | Phase 3 | API 모킹 |
| `@testing-library/jest-dom` | Phase 3 | DOM 매처 |
| `@testing-library/user-event` | Phase 3 | 사용자 인터랙션 |

**기존 패키지 재사용:** Recharts, TanStack Router+Query, Zustand, i18next, HyperMD, Radix/shadcn, Tailwind, react-markdown, lucide-react

---

## 11. Phase 1-3 완료 후 완성도 예상

```
┌─────────────────────────────────────────────────────────────────┐
│                 OXIOS WEB UI 완성도: 95%                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  인프라/파운데이션    ████████████████████  98%                │
│  UI 컴포넌트          ████████████████████  95%                │
│  Knowledge 앱         ████████████████████  95%                │
│  Memory               ████████████████████  95%  ← Phase 1   │
│  Seed/Agent/Trace     ████████████████████  95%  ← Phase 1   │
│  Chat                 ████████████████████  95%  ← Phase 2   │
│  Workspace            ████████████████████  95%  ← Phase 2   │
│  DataTable/필터       ███████████████████░  90%  ← Phase 2   │
│  MCP 관리             ███████████████████░  90%  ← Phase 2   │
│  Budget               ████████████████████  95%  ← Phase 3   │
│  Agent Groups         ███████████████████░  90%  ← Phase 3   │
│  A2A Monitor          ██████████████████░░  85%  ← Phase 3   │
│  Marketplace          ████████████████████  95%  ← Phase 3   │
│  i18n                 ████████████████████  98%                │
│  테스트               ████████████████░░░░  70%  ← Phase 3   │
│                                                                 │
│  전체: 95%                                                      │
│  미달: A2A (내부 프로토콜 가시성 한계), 테스트 (지속적 보강)    │
└─────────────────────────────────────────────────────────────────┘
```
