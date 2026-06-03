# RFC-T1-A: A2A Interactive Topology Graph

> **날짜:** 2026-06-03
> **Tier:** 1 (Quick Win, 시각적 임팩트 큼)
> **영역:** `surface/oxios-web/web/src/components/a2a/`, `routes/a2a.tsx`
> **기반:** 현재 `topology-graph.tsx` (230줄, 정적 SVG 원형 배치)

---

## 1. 동기

현재 `topology-graph.tsx`는 노드를 원형으로 정렬한 단순 SVG. **에이전트 OS의 핵심 시각화**(에이전트 간 통신)인데 80% 비어 있어 보임.

- 정적 (드래그/줌 없음)
- 메시지 흐름 표현 없음
- 노드 클릭 시 상세 정보로 점프 불가 (탭 전환 필요)
- 라이브 변화 없음

**목표:** Gemini Agents / Anthropic Multi-Agent Dashboard 수준의 인터랙티브 토폴로지.

---

## 2. 디자인

### 와이어프레임

```
┌─────────────────────────────────────────────────────────────────┐
│  A2A Monitor                                  [⟳ refresh] [⚙]   │
├─────────────────────────────────────────────────────────────────┤
│  [Topology] Messages  Agents                                    │
│  ══════════                                                    │
│                                                                 │
│  ┌──────────────────────────┐  ┌──────────────────────────┐   │
│  │                          │  │ AgentCard                │   │
│  │   [Interactive Graph]    │  │ ───────                  │   │
│  │                          │  │ 🟢 orchestrator-1        │   │
│  │      (A)──msg──>(B)      │  │ Status: Running          │   │
│  │       \      /           │  │ Capabilities: read, write│   │
│  │        \    /            │  │ Skills: code-review      │   │
│  │      (C)──msg──>(D)      │  │ Last msg: 2s ago         │   │
│  │         |                │  │                          │   │
│  │      [animated edge]     │  │ [View trace] [Stop]      │   │
│  │                          │  │                          │   │
│  │  ●●● legend: live msg    │  │ Messages (12)            │   │
│  │  ━━ handshake            │  │ ─ A→B  TaskDelegation    │   │
│  │  ── capability query     │  │ ─ B→C  StatusUpdate      │   │
│  │                          │  │ ─ C→A  ResultSharing     │   │
│  └──────────────────────────┘  └──────────────────────────┘   │
│                                                                 │
│  [⏸ Pause] [📸 Snapshot] [🔍 Zoom fit] [⛶ Fullscreen]         │
└─────────────────────────────────────────────────────────────────┘
```

### 컴포넌트 트리

```
routes/a2a.tsx (탭 그대로, "Topology"가 기본)
  └─ <InteractiveTopology nodes edges liveMessages />
        ├─ <ReactFlowProvider>
        │     ├─ <Background dotted />
        │     ├─ <Controls /> (zoom, fit, lock)
        │     ├─ <MiniMap />
        │     └─ custom <AgentNode /> (status, name, cap count)
        ├─ <AnimatedEdge /> (live 메시지: 흐르는 점선)
        └─ <AgentInspector /> (우측 슬라이드 패널, 노드 클릭 시)
```

### 노드 디자인 (Custom React Flow Node)

```tsx
function AgentNode({ data }) {
  return (
    <div className={cn('rounded-lg border-2 bg-card p-3 min-w-[180px]',
                       data.status === 'running' && 'border-emerald-500',
                       data.status === 'idle' && 'border-amber-500',
                       data.status === 'stopped' && 'border-red-500')}>
      <div className="flex items-center gap-2">
        <Bot className="h-4 w-4" />
        <span className="font-medium text-sm">{data.label}</span>
        <span className="ml-auto h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
      </div>
      <div className="text-xs text-muted-foreground mt-1">
        {data.capabilities_count} caps · {data.skills_count} skills
      </div>
      <div className="text-[10px] text-muted-foreground/70 mt-0.5">
        Last activity: {data.last_seen_human}
      </div>
    </div>
  )
}
```

### 라이브 메시지 애니메이션

- WS로 새 `A2AMessage` 수신 시 → 해당 edge의 `AnimatedEdge`에 pulse 트리거
- 점선 흐름 (CSS `@keyframes` + `stroke-dashoffset` 애니메이션)
- 메시지 종류별 색상:
  - `TaskDelegation` → blue
  - `StatusUpdate` → gray
  - `ResultSharing` → green
  - `CapabilityQuery` → purple
  - `Handshake` → amber

---

## 3. 구현 계획

### 파일 변경

| 파일 | 변경 |
|------|------|
| `package.json` | `+reactflow ^11` (또는 `@xyflow/react ^12`) |
| `components/a2a/topology-graph.tsx` | **삭제 또는 deprecated로** (이름 유지하면 import 깨짐) |
| `components/a2a/interactive-topology.tsx` | **신규** — ReactFlow 래퍼 |
| `components/a2a/agent-node.tsx` | **신규** — Custom node |
| `components/a2a/animated-edge.tsx` | **신규** — 메시지 애니메이션 |
| `components/a2a/agent-inspector.tsx` | **신규** — 사이드 패널 |
| `hooks/use-a2a-topology.ts` | 변경: `{ nodes, edges }` 형태로 (백엔드 변경 필요) |
| `routes/a2a.tsx` | 변경: Topology 탭이 새 컴포넌트 사용 |

### 백엔드 변경 (커널)

`/api/a2a/topology` 응답 형식 확장:

```rust
// 현재: Vec<TopologyNode>
// 변경: { nodes: [...], edges: [{ from, to, message_kind, count }] }

pub struct TopologyResponse {
    pub nodes: Vec<TopologyNode>,  // { id, label, status, capabilities: Vec<String>, skills: Vec<String>, last_seen }
    pub edges: Vec<TopologyEdge>,  // { from: AgentId, to: AgentId, message_count_5m, last_kind: A2AMessageKind }
}
```

`A2AProtocol::recent_messages(secs: u64)` 추가 → 최근 N초간 메시지를 집계해 edges로 변환.

WS 이벤트로 `A2AMessage` 단발 발생 시 → 같은 hook(`use-a2a-topology.ts`)이 React Query 캐시에 mutation:
- `qc.setQueryData(['a2a', 'topology'], ...)` 로 노드/엣지 즉시 업데이트
- Animated edge reflow 트리거

---

## 4. 단계별 작업

### Step 1: 백엔드 TopologyResponse 변경 (3시간)
- `crates/oxios-kernel/src/a2a.rs`: `TopologyResponse` 정의
- `surface/oxios-web/src/routes/a2a.rs`: 핸들러 갱신
- 기존 단일 노드 응답에서 변환

### Step 2: ReactFlow 통합 (4시간)
- `package.json` 의존성 추가
- `interactive-topology.tsx` 기본 셋업 (Background, Controls, MiniMap)
- `agent-node.tsx` Custom node
- 기존 `topology-graph.tsx` 호환을 위해 `TopologyGraph` 이름 유지, 새 파일 추가

### Step 3: 라이브 메시지 애니메이션 (3시간)
- `animated-edge.tsx`: `getBezierPath` + `<circle>` + `<animateMotion>` 대안으로 CSS keyframes
- WS 핸들러 (`stores/chat.ts` 또는 신규 `stores/a2a.ts`)에서 캐시 mutation
- 메시지 발생 시 edge의 `lastPulse` 타임스탬프 업데이트 → 3초간 애니메이션

### Step 4: AgentInspector 사이드 패널 (2시간)
- `agent-inspector.tsx`: 노드 클릭 시 `selectedNodeId` state로 슬라이드 인
- 기존 `agent-card-list.tsx` 재사용, 한 에이전트 상세로 축소
- 마지막 메시지 5개, capabilities 목록, [Stop agent] 버튼

### Step 5: 풀스크린 / 스냅샷 (2시간)
- 풀스크린 모달
- SVG/PNG export (`html-to-image` 또는 `dom-to-image`)

### Step 6: 테스트 + 다듬기 (2시간)
- E2E: `e2e/a2a-topology.spec.ts` — 노드 2개 + 메시지 1개 fixture
- 빈 상태, 로딩 상태, 에러 상태
- 키보드 단축키 (선택 노드 화살표, ESC로 디셀렉트)

**총: ~16시간 (2일)**

---

## 5. 위험 / 주의

| 위험 | 대응 |
|------|------|
| ReactFlow 번들 크기 (gzipped ~70KB) | route-level lazy load (TanStack Router 기본 동작) |
| 100+ 노드 성능 | force layout은 `dagre` 또는 `elkjs`로 (결정적, 빠름) |
| WS 재연결 시 중복 pulse | `pulseId` set으로 1초 내 중복 dedupe |
| 라이브 노드 추가/제거 시 layout 점프 | 새 노드는 fade-in 200ms, 제거는 fade-out |

---

## 6. 의존성

```json
"dependencies": {
  "reactflow": "^11.11.4"
},
"devDependencies": {
  "dagre": "^0.8.5",
  "@types/dagre": "^0.7.52"
}
```

> Note: `reactflow` v12 (`@xyflow/react`)는 React 19 호환성 확인 필요. 호환 안 되면 v11.

---

## 7. 완료 기준

- [ ] 노드 드래그, 줌, fit-view, 미니맵 동작
- [ ] 노드 클릭 → 우측 사이드 패널 (Inspector) 슬라이드 인
- [ ] 라이브 메시지 → edge에 3초간 애니메이션
- [ ] 풀스크린 토글, PNG export
- [ ] 키보드 접근성 (Tab으로 노드 순회, Enter로 인스펙터)
- [ ] E2E 테스트 1개 통과
- [ ] 기존 `topology-graph.tsx` 호출처 마이그레이션 완료
