# RFC-T1-C: Live Operations Dashboard

> **날짜:** 2026-06-03
> **Tier:** 1→2 (Quick Win에서 약간 큰 작업)
> **영역:** `surface/oxios-web/web/src/routes/index.tsx` + 신규 위젯
> **기반:** 현재 4개 스탯 + 2개 카드 + 8개 링크 (정적)
> **연계:** RFC-015 SSE 이벤트, 기존 `useEvents` hook

---

## 1. 동기

`routes/index.tsx` (대시보드)는 **5~10초 refetch + 정적 카드**. 운영자에게 필요한 건:
- 지금 무슨 일이 일어나고 있는지 한눈에
- 승인이 필요한 액션 큐
- 리소스 임계치 경고
- 활성 에이전트 상태 변화의 즉각 반영

현재는 각 페이지(`/events`, `/agents`, `/resources`, `/approvals`)에 **따로 가야** 보임. TweetDeck / Grafana / Datadog 수준으로 **홈에 모든 것을 압축**.

---

## 2. 디자인

### 와이어프레임 — Operation Center

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Oxios                          [⏵ Live ●] [pause] [⏰ last 1h ▾]  [user] │
├────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ Agents   │ │ Active   │ │ Tokens   │ │ CPU      │ │ Pending  │        │
│  │ 12  ▲2   │ │ 5        │ │ 4.2k/min │ │ 34% ▓▓░░ │ │ Approve 2│        │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘        │
│                                                                            │
│  ┌─────────────────────────────┐ ┌──────────────────────────────────┐      │
│  │ 📡 Live Activity Feed       │ │ 📊 Resource Trends (1h)          │      │
│  │ ─────────────────────────── │ │                                  │      │
│  │ 12:34  🚀 fork    "summar…  │ │  CPU  ────  ▲ spike 12:30       │      │
│  │ 12:34  🧠 memory  recall 3  │ │       ╱╲                         │      │
│  │ 12:33  ✅ done    "Rate li…" │ │  MEM  ─────────────              │      │
│  │ 12:33  🔧 tool    exec/ls   │ │                                  │      │
│  │ 12:32  ⚠ approval  exec/cat  │ │  TOK  ╱╲    ╱╲                   │      │
│  │ 12:32  💬 msg     user       │ │       ╲╱    ╲╱                   │      │
│  │ [filter ▾] [scroll] [open→] │ │  AGENTS ▁▂▃▅▇▆▄                  │      │
│  └─────────────────────────────┘ └──────────────────────────────────┘      │
│                                                                            │
│  ┌─────────────────────────────┐ ┌──────────────────────────────────┐      │
│  │ 🤖 Active Agents (5)        │ │ ⏰ Scheduler / Cron Next         │      │
│  │ ─────────────────────────── │ │ ─────────────────────────────── │      │
│  │ 🟢 summarizer-1  [trace]    │ │ • 14:00 backup-daily    28m      │      │
│  │   2m elapsed · 1.2k tok     │ │ • 15:00 memory-dream   1h28m     │      │
│  │ 🟡 researcher-2   [trace]    │ │ • 18:00 session-clean  4h28m     │      │
│  │   5m idle · 0 tok           │ │ [view all →]                     │      │
│  │ 🟢 writer-3       [trace]    │ │                                  │      │
│  │   1m elapsed · 800 tok      │ │                                  │      │
│  │ [view all →]                │ │                                  │      │
│  └─────────────────────────────┘ └──────────────────────────────────┘      │
│                                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 🔔 Approvals Queue (2)                                              │   │
│  │ ─────────────────────────────────────────────────────────────────── │   │
│  │ ⚠ "exec" by researcher-2 wants to run `cat /etc/passwd`            │   │
│  │   Risk: read system file · [Approve] [Deny] [Details]               │   │
│  │ ⚠ "network" by writer-3 wants GET https://api.example.com/...      │   │
│  │   Risk: outbound network · [Approve] [Deny] [Details]               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────────┘
```

### 컴포넌트 트리

```
routes/index.tsx
  └─ <DashboardLayout>
        ├─ <StatRow>             (5 KPI 카드)
        ├─ <div className="grid grid-cols-2">
        │     ├─ <LiveActivityFeed>     (좌, 2/3)
        │     │   └─ SSE 구독, 가상 스크롤
        │     └─ <ResourceTrends>       (우, 1/3)
        │         └─ Recharts AreaChart (CPU/MEM/TOK 3 series)
        ├─ <div className="grid grid-cols-2">
        │     ├─ <ActiveAgentsList>     (좌, 1/2)
        │     └─ <SchedulerNext>        (우, 1/2)
        └─ <ApprovalsQueue>        (전체 폭)
```

### KPI 카드 변형

기존 4개에서 5개로 확장, 각각 미니 sparkline + 변화 화살표:

```tsx
<StatCard
  label="Tokens/min"
  value="4.2k"
  delta={+12}            // %
  sparkline={last60}     // array<number>
  sparkColor="violet"
/>
```

### Live Activity Feed

- `useEvents` (기존 SSE) → 필터링:
  - `agent.fork`, `agent.kill`, `agent.done`
  - `memory.recall`, `memory.consolidate`
  - `tool.start`, `tool.end`
  - `approval.requested`, `approval.resolved`
  - `cron.executed`
- 각 이벤트 → 아이콘 + 1줄 요약 + 상대 시간
- **클릭 시**: 해당 객체 상세 페이지로 라우팅 (에이전트 → `/agents/$id`, 승인 → `/approvals`)

### Resource Trends

- 기존 `/api/resources/history?last_n=30` → `last_n=120` (1h @ 30s 간격) 또는 WS로 1s 단위 푸시
- Recharts AreaChart, 3 series (CPU, MEM, TOK)
- 임계치 라인 (CPU 80%, MEM 90%) 점선

### Approvals Queue

- 기존 `/api/approvals` 쿼리 → pending만 필터
- 인라인 버튼 (Approve/Deny) → mutation
- "Details" → diff/dialog (RFC-018 연계)
- 0개일 때 카드 숨김, 1개+일 때만 표시

---

## 3. 구현 계획

### 파일 변경

| 파일 | 변경 |
|------|------|
| `routes/index.tsx` | **대폭 재작성** (현재 175줄 → ~250줄) |
| `components/dashboard/stat-card.tsx` | **신규** — sparkline + delta |
| `components/dashboard/live-activity-feed.tsx` | **신규** |
| `components/dashboard/resource-trends.tsx` | **신규** |
| `components/dashboard/active-agents-list.tsx` | **신규** |
| `components/dashboard/scheduler-next.tsx` | **신규** |
| `components/dashboard/approvals-queue.tsx` | **신규** |
| `hooks/use-events.ts` | 변경: 필터링 hook 추가 |
| `lib/event-formatter.ts` | **신규** — 이벤트 타입 → 아이콘/색/요약 |
| `types/events.ts` | 추가: `Approval` (이미 일부 있음) |

### 단계별 작업

### Step 1: 기존 페이지 인벤토리 + 공용 hook 추출 (3시간)
- `useEvents` (이미 있음) 동작 확인
- `useApprovals` 신규 hook
- `useSchedulerNext` 신규 hook (다음 N개 cron/scheduled)
- `useResourceTrends` — 기존 history를 120개로 늘림

### Step 2: 공용 위젯 작성 (6시간)
- `StatCard` (sparkline, delta)
- `LiveActivityFeed` (SSE 구독, virtualized list)
- `ResourceTrends` (Recharts)
- `ActiveAgentsList` (기존 `/api/agents` 폴링)
- `SchedulerNext` (`/api/scheduler/tasks` + `/api/cron-jobs`)
- `ApprovalsQueue` (mutation 인라인)

### Step 3: DashboardLayout 재구성 (2시간)
- `routes/index.tsx` 재작성
- 그리드 레이아웃 (Tailwind grid, 12-col 또는 수동)
- 반응형 (모바일: 1열, 태블릿: 2열, 데스크탑: 2~3열)

### Step 4: 인터랙션 + 라이브 업데이트 (3시간)
- Approvals 큐에서 인라인 Approve/Deny → optimistic update
- 새 이벤트 도착 시 부드러운 fade-in (Framer Motion? 아니면 CSS `@starting-style` 또는 `transition`)
- "⏸ Pause" 버튼: 새 이벤트 누적 멈춤 (분석용)

### Step 5: 권한 + 비용 (2시간)
- 위젯별 폴링 간격 합리화:
  - `useEvents`: SSE (push)
  - `useApprovals`: 5s 폴링
  - `useAgents`: 5s
  - `useResourceTrends`: 30s history + 1s WS delta
  - `useSchedulerNext`: 30s
- 백엔드에 위젯 집계 API가 있으면 사용 (예: `/api/dashboard/aggregates`)

### Step 6: 테스트 + 다듬기 (2시간)
- E2E: 대시보드 진입, 위젯 렌더, 승인 처리
- 빈 상태 (에이전트 0, 이벤트 없음, 리소스 데이터 없음)
- 로딩 skeleton

**총: ~18시간 (2.5일)**

---

## 4. 위험 / 주의

| 위험 | 대응 |
|------|------|
| 위젯 너무 많으면 정신없음 | 사용자 토글로 위젯 on/off, "Reset to default" |
| N+1 폴링 (5개 위젯 × 5s) | 가능하면 백엔드 aggregate API 1개로 묶기 |
| Live activity feed 스크롤 점프 | 사용자가 위로 스크롤 시 자동 스크롤 멈춤 (현재 chat에서 구현된 패턴) |
| 모바일에서 5열 그리드 | `grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5` |
| 색맹 / 접근성 | sparkline 외에 delta 숫자/화살표로 |

---

## 5. 완료 기준

- [ ] 5개 KPI 카드, 각각 sparkline + delta
- [ ] Live Activity Feed: SSE 기반, 필터링, 클릭 라우팅
- [ ] Resource Trends: 1h 3-series area chart
- [ ] Active Agents: 클릭 시 상세 페이지로
- [ ] Scheduler Next: 다음 3개 cron, [view all] 링크
- [ ] Approvals Queue: 인라인 Approve/Deny, 0개 시 숨김
- [ ] 반응형 (모바일 1열)
- [ ] "Pause" 토글 (분석 모드)
- [ ] E2E 테스트 1개
