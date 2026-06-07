# 대시보드 재설계

> **상태**: 설계 완료, 구현 대기
> **날짜**: 2026-06-07
> **동기**: 기존 대시보드의 세로 과다 스크롤, Quick Links 중복, RAM KPI 누락, 에러 요약 부재, Card-inside-Card 등 문제 해결

---

## 1. 설계 목표

| 목표 | 설명 |
|------|------|
| **운영 모니터링** | 에이전트 실패, 리소스 병목, 승인 대기를 즉시 발견 |
| **시스템 현황 파악** | Oxios가 무엇을 하고 있는지 한눈에 |
| **랜딩 페이지** | 첫 방문자가 어디로 가야 할지 직관적 안내 |
| **1스크롤 (1920×1080)** | 사이드바 제외 ~1640px 가용 폭에서 모든 핵심 정보 above the fold |
| **Vercel/Railway 스타일** | 카드 기반, 적절한 여백, 주요 메트릭은 크게 |

## 2. 레이아웃

```
┌──────────────────────────────────────────────────────────────┐
│ 대시보드 · Oxios 에이전트 OS 개요                              │
├────────┬────────┬────────┬────────┬────────┬────────────────┤
│ 에이전트│ 실행 중 │ 토큰/분 │  CPU   │  RAM   │    승인 대기   │
│ 3/12   │   3    │  1.2k  │  45%   │  62%   │      2        │
├────────┴────────┴────────┴────────┴────────┴────────────────┤
│                                                              │
│ ┌──────────────────────────────┐ ┌────────────────────────┐ │
│ │ Agents & Activity            │ │ 시스템 상태            │ │
│ │                              │ │                        │ │
│ │ ┌─────────┬────────────────┐ │ │ 모델: gpt-4o [openai] │ │
│ │ │ 에이전트 │ 실시간 활동    │ │ │ ✅ Store ✅ Bus       │ │
│ │ │ 목록    │ (bare 모드)    │ │ │ ✅ Memory (142)       │ │
│ │ │ (8개)   │               │ │ │ ⏱ 1h 30m 5s          │ │
│ │ │         │               │ │ │ 12 완료 · 2 실패      │ │
│ │ │         │               │ │ │──────────────────────│ │
│ │ │         │               │ │ │ 모델 사용량           │ │
│ │ │         │               │ │ │ gpt-4o    65% $1.23  │ │
│ │ └─────────┴────────────────┘ │ │ claude    25% $0.45  │ │
│ └──────────────────────────────┘ └────────────────────────┘ │
│                                  ┌────────────────────────┐ │
│                                  │ 승인 대기열            │ │
│                                  │ ✅ 이상 없음           │ │
│                                  └────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### 행 구성

| 행 | 내용 | 비고 |
|----|------|------|
| Header | 제목 + 부제 | 기존과 동일 |
| Row 1 | KPI 6개 | AgentStatusCard 1개 + StatCard 5개 |
| Row 2 | 좌측: Agents & Activity (2/3) / 우측: Health+Usage, Approvals (1/3) | `grid lg:grid-cols-3` |

### 반응형 브레이크포인트

| 해상도 | KPI Row | Row 2 | AgentsActivityCard 내부 |
|--------|---------|-------|------------------------|
| < 768px (모바일) | 2열 × 3행 | 1컬럼 스택 | 탭 전환 (CSS hidden) |
| 768–1024px (태블릿) | 3열 × 2행 | 1컬럼 스택 | 탭 전환 (CSS hidden) |
| 1024–1279px | 3열 × 2행 | 2:1 그리드 | 좌우 나란히 |
| ≥ 1280px | 6열 × 1행 | 2:1 그리드 | 좌우 나란히 |

모바일/태블릿에서 탭 전환 시 `display: none`이 아닌 CSS `hidden` 클래스로 DOM 유지 → LiveActivityFeed의 Pause 상태, 필터, 스크롤 위치 보존.

---

## 3. KPI Row (6개 카드)

### 3-1. AgentStatusCard (전용 컴포넌트)

기존 `StatCard` 인터페이스(`value: string | number` 하나)에 맞지 않으므로 전용 컴포넌트로 분리.

```
┌──────────────────┐
│ 에이전트          │
│                  │
│ 3 / 12          │  ← big number: running / total
│ 2 실패           │  ← secondary: failed count (0이면 숨김)
│ [sparkline]      │  ← running count 시계열
└──────────────────┘
```

- `href="/agents"` 클릭 시 이동
- `total_forked`가 null이면 `?` 표시 + 툴팁
- 스파크라인은 running count만 (totalForked는 단조 증가로 의미 없음)
- `total_failed > 0`이면 빨간 텍스트로 실패 수 표시

### 3-2. StatCard × 5 (기존 컴포넌트 그대로)

| # | 라벨 | 값 | 아이콘 | 스파크라인 | 링크 |
|---|------|-----|--------|-----------|------|
| 2 | 실행 중 | `runningAgents.length` | `Activity` | running 수 | `/agents` |
| 3 | 토큰/분 | `formatTokensPerMin()` | `Zap` | 토큰율 | — |
| 4 | CPU | `cpuSeries[last]%` | `Cpu` | CPU% | `/resources` |
| 5 | **RAM (NEW)** | `memSeries[last]%` | `HardDrive` | RAM% | `/resources` |
| 6 | 승인 대기 | `pendingApprovals.length` | `AlertTriangle` | — | `/approvals` |

RAM은 `useResourceHistory`에서 `memory_percent`를 이미 제공하므로, `seriesFromSnapshots(snapshots, 'memory_percent')`로 시계열 생성. 스파크 컬러 `"rose"` 추가.

---

## 4. Agents & Activity Card (좌측 2/3)

### 구조

`ActiveAgentsList` + `LiveActivityFeed`를 **composition**으로 조합.

```tsx
<Card className="flex h-full flex-col">
  <CardHeader>
    <CardTitle>[Bot] 에이전트 & 활동 [12 events] [● 연결됨] [필터▼] [⏸]</CardTitle>
  </CardHeader>
  <CardContent className="flex-1 flex gap-4 min-h-[300px]">
    {/* 데스크톱: 좌우 분할. 모바일: 탭 전환 */}
    <div className="hidden md:block w-1/3">
      <ActiveAgentsList agents={runningAgents} />
    </div>
    <div className="md:w-2/3 flex-1">
      <LiveActivityFeed variant="bare" />
    </div>
  </CardContent>
</Card>
```

### ActiveAgentsList

- 기존 `ActiveAgentsCard`의 목록 부분만 추출
- 최대 8개 표시 (기존 5개 → 8개)
- 각 행: 아이콘 + 이름 + 상태 뱃지 + ID (6자리)
- 8개 초과 시 "전체 N개 보기" 링크 (`/agents`)
- 에이전트 0개면 빈 상태 메시지
- Card 래퍼 없이 내용만 렌더 (부모 Card에 포함되므로)

### LiveActivityFeed 변경사항

`variant` prop 추가:

```typescript
interface LiveActivityFeedProps {
  variant?: 'card' | 'bare'
}
```

- `"card"` (기본값): 기존 동작. 자체 `<Card>` 래퍼 포함. 독립 사용 시.
- `"bare"`: `<Card>` + `<CardHeader>` + `<CardContent>` 래퍼 생략. 내부 로직(hooks, 필터, 리스트)만 렌더. `AgentsActivityCard` 내부에서 사용.

모바일/태블릿에서 탭 전환 시 CSS `hidden` 클래스로 토글 (unmount하지 않음):

```tsx
{/* 모바일: 탭 헤더 */}
<div className="flex md:hidden border-b">
  <button onClick={() => setTab('agents')}>에이전트</button>
  <button onClick={() => setTab('activity')}>활동</button>
</div>
{/* 에이전트 탭 */}
<div className={cn(tab === 'agents' ? '' : 'hidden', 'md:hidden md:block w-1/3')}>
  <ActiveAgentsList />
</div>
{/* 활동 탭 */}
<div className={cn(tab === 'activity' ? '' : 'hidden', 'flex-1')}>
  <LiveActivityFeed variant="bare" />
</div>
```

### 카드 헤더 컨트롤

- 이벤트 카운트 뱃지: LiveActivityFeed에서 계산, AgentsActivityCard 헤더에 표시
- 연결 상태 표시등 (초록 점 / 빨간 뱃지)
- 필터 드롭다운: 헤더 우측
- Pause/Resume 버튼: 헤더 우측

---

## 5. System Health Card (우측 상단)

### 통합 내용

기존 `SystemHealthCard` + `CurrentModelCard` + `ModelUsageCard`를 **하나의 카드**로 병합.

```
┌──────────────────────────────────┐
│ [Shield] 시스템 상태    v0.14.2  │
├──────────────────────────────────┤
│ 💡 모델                          │
│ gpt-4o  [openai 배지]  [⚙️]     │
│                                  │
│ ✅ 상태 저장소    정상            │
│ ✅ 이벤트 버스    정상            │
│ ✅ 메모리         142개 항목      │
│ ⏱ 가동 시간      1h 30m 5s      │
│                                  │
│ 12 완료 · 2 실패                 │  ← agents.total_completed, total_failed
├──────────────────────────────────┤
│ 모델 사용량                      │
│ gpt-4o        65% · $1.23       │
│ claude-3.5    25% · $0.45       │
│ ollama/llama  10% · $0.00       │
│ $1.68 총 비용 · 342회 호출       │
└──────────────────────────────────┘
```

- "모델" 섹션: 모델명 + Provider 배지 + 설정 아이콘 (클릭 → `/settings?section=engine`)
- Health Rows: 기존과 동일
- `total_completed` / `total_failed`를 Health 아래 작은 텍스트로 표시
- `total_failed > 0`이면 빨간색으로 강조
- "모델 사용량" 섹션: ModelUsageCard 내용을 같은 카드 하단에 배치. `<Separator />`로 구분.
- ModelUsage의 `totalRequests === 0`이면 사용량 섹션 전체 숨김.
- 한국어 하드코딩 → i18n 키 사용

---

## 6. Approvals Queue (우측 하단)

기존 `ApprovalsQueue`와 동일하되:

- `max-h-[200px] overflow-y-auto`로 최대 높이 제한
- 3개 초과 시 내부 스크롤 + "전체 보기 → `/approvals`" 링크
- pending 0개: 1줄 "✅ 이상 없음" (기존과 동일)
- pending 있음: 인라인 Approve/Deny 버튼 (기존과 동일)

---

## 7. 삭제 항목

| 항목 | 사유 |
|------|------|
| Quick Links (8개) | 사이드바와 완전 중복 |
| `CurrentModelCard` (독립) | SystemHealthCard에 통합 |
| `ModelUsageCard` (독립) | SystemHealthCard에 통합 |

---

## 8. 파일 변경 계획

```
components/dashboard/
├── stat-card.tsx              # 유지 (SparkColor "rose" 추가만)
├── model-usage-card.tsx       # 삭제 (SystemHealthCard에 흡수)
├── approvals-queue.tsx        # 유지 (max-h 추가)
├── live-activity-feed.tsx     # 수정 (variant: "bare" | "card" prop 추가)
├── agents-activity-card.tsx   # 신규 (ActiveAgentsList + LiveActivityFeed 조합)
├── agent-status-card.tsx      # 신규 (전용 KPI 카드)
└── system-health-card.tsx     # 신규 (Health + Model + Usage 통합)

routes/index.tsx               # 수정
  - Quick Links 섹션 삭제
  - CurrentModelCard, SystemHealthCard(기존), ActiveAgentsCard 함수 제거
  - 새 컴포넌트 임포트로 교체
  - RAM 시계열 로직 추가 (seriesFromSnapshots(snapshots, 'memory_percent'))
  - KPI grid를 xl:grid-cols-6으로 변경

i18n keys 추가:
  - dashboard.memory: "메모리"
  - dashboard.ram: "RAM"
  - dashboard.agentsRunning: "실행 중"
  - dashboard.agentsFailed: "실패"
  - dashboard.agentsCompleted: "완료"
  - dashboard.modelLabel: "모델"
  - dashboard.modelNotSet: "모델 미설정"
  - dashboard.modelUsage: "모델 사용량"
  - dashboard.totalCostAndCalls: "총 비용 · 호출수" (기존 하드코딩 대체)
```

---

## 9. 구현 순서

1. **i18n 키 추가** — ko.json, en.json
2. **SparkColor "rose" 추가** — stat-card.tsx
3. **LiveActivityFeed `variant` prop** — 기존 코드 수정, card/bare 분기
4. **AgentStatusCard** — 신규 컴포넌트
5. **ActiveAgentsList** — 기존 ActiveAgentsCard에서 목록 부분만 추출
6. **AgentsActivityCard** — 조합 컴포넌트 (ActiveAgentsList + LiveActivityFeed bare)
7. **SystemHealthCard** — Health + Model + Usage 통합 카드
8. **routes/index.tsx** — 전체 레이아웃 재구성
9. **삭제** — model-usage-card.tsx, 기존 인라인 함수들
10. **반응형 테스트** — 1280px, 1024px, 768px, 375px 뷰포트 확인
