# RFC-T1-B: Memory Embedding Map (시맨틱 시각화)

> **날짜:** 2026-06-03
> **Tier:** 1 (Quick Win, 시각적 임팩트 큼)
> **영역:** `surface/oxios-web/web/src/components/memory/`, `routes/memory.tsx`
> **기반:** 현재 `memory-browser.tsx` (Tier/Type 필터 + 카드 그리드)
> **연계:** `docs/ARCHITECTURE.md`의 "hyperbolic embeddings, HNSW" 메타포

---

## 1. 동기

현재 Memory UI는 **DB 테이블을 보는 느낌**:
- Tier(Hot/Warm/Cold) + Type(Fact/Episode/Knowledge/...) 필터
- 카드 그리드
- 검색은 별도 탭

Oxios의 차별점은 "**hyperbolic embeddings + HNSW**로 의미적 메모리를 계층화"한다는 것 (ARCHITECTURE.md). 이걸 **눈으로 보여주지 않으면** CLI에서 `memory list` 한 것과 다를 게 없음.

**목표:** 메모리의 의미적 관계를 Poincaré disk / 2D force layout으로 시각화. dream consolidation의 효과도 시각적으로 검증 가능.

---

## 2. 디자인

### 와이어프레임 — "Map" 탭 추가

```
┌─────────────────────────────────────────────────────────────────┐
│  Memory                                  [Map] [List] [Dream] [Search] │
├─────────────────────────────────────────────────────────────────┤
│  Filters: Tier [All ▾]  Type [All ▾]  Time [30d ▾]  [□ Animate]  │
│                                                                 │
│  ┌─────────────────────────────────────────┐  ┌─────────────┐  │
│  │                                         │  │ Selected    │  │
│  │           · ·  ·                        │  │ ─────────── │  │
│  │      · ⊙ ⊙ ⊙ ·                          │  │ "API rate   │  │
│  │    · ⊙ ⊙ ⊙ ⊙ ⊙ ·                        │  │  limit      │  │
│  │      · ⊙ ⊙ ⊙ ·   ← Hot cluster          │  │  exceeded"  │  │
│  │        · ·  ·                           │  │             │  │
│  │                                         │  │ Tier: Hot   │  │
│  │           · ·                            │  │ Type: Fact  │  │
│  │      ·  ⊙ ⊙ ·    ← Warm cluster         │  │ Sim: 0.87   │  │
│  │         ·                                │  │             │  │
│  │                                         │  │ Related:    │  │
│  │       · · ·                              │  │ • "rate..." │  │
│  │      · ⊙ ·       ← Cold sparse           │  │ • "429..."  │  │
│  │                                         │  │             │  │
│  │  [Legend]  ● Hot  ● Warm  ● Cold         │  │ [Open →]    │  │
│  │  Click ●: details   Drag: pan            │  └─────────────┘  │
│  └─────────────────────────────────────────┘                   │
│                                                                 │
│  [⏵ Re-cluster] [⏸ Pause animations] [⛶ Fullscreen]            │
└─────────────────────────────────────────────────────────────────┘
```

### 컴포넌트 트리

```
routes/memory.tsx (탭 추가: "Map"이 "Browse" 옆)
  └─ <MemoryMap memories embeddings />
        ├─ <EmbeddingCanvas>  ← d3-force + Canvas (500+ 노드 성능)
        │     ├─ <Node>  ← tier별 색/크기, type별 shape
        │     ├─ <Edge>  ← similarity > 0.7 (top-k)
        │     └─ <ClusterRing>  ← tier 경계 (배경 원)
        └─ <SelectionPanel>  ← 우측 또는 하단 drawer
```

### 시각 인코딩

| 차원 | 인코딩 |
|------|--------|
| 위치 (x, y) | UMAP 2D projection (의미적 거리) |
| 색상 (채도) | Tier: Hot=emerald, Warm=amber, Cold=zinc |
| 크기 | recency + access_count (가중) |
| 모양 | Type: Fact=●, Episode=▲, Decision=■, Skill=★ |
| 엣지 (opacity) | cosine similarity (0.5~1.0 → 0.1~0.8) |
| 클러스터 영역 | tier별 반투명 원 (D3 contour 또는 단순 background circle) |

### 인터랙션

- **Hover** → 노드 강조, 연결된 노드들 opacity ↑, 그 외 ↓
- **Click** → 우측 SelectionPanel
- **Double-click** → `MemoryDetail` 모달 (기존 컴포넌트 재사용)
- **Drag** → 팬
- **Wheel** → 줌
- **Cmd+F** → 검색 → 매칭 노드 강조 + 카메라 fly-to

---

## 3. 구현 계획

### 파일 변경

| 파일 | 변경 |
|------|------|
| `package.json` | `+d3-force`, `+d3-selection`, `+d3-zoom`, `+umap-js` (또는 백엔드에서 처리) |
| `types/memory.ts` | 추가: `embedding: number[] \| { x: number, y: number }` |
| `hooks/use-memory.ts` | 변경: `/api/memory/map` 쿼리 (좌표 포함) |
| `components/memory/memory-map.tsx` | **신규** — 메인 캔버스 |
| `components/memory/embedding-canvas.tsx` | **신규** — d3-force + Canvas |
| `components/memory/cluster-legend.tsx` | **신규** |
| `components/memory/selection-panel.tsx` | **신규** |
| `routes/memory.tsx` | 변경: "Map" 탭 추가 |

### 백엔드 변경 (커널)

`/api/memory/map` 새 엔드포인트:

```rust
// memory.rs
pub struct MemoryMapEntry {
    pub id: MemoryId,
    pub tier: MemoryTier,
    pub mem_type: MemoryType,
    pub content_preview: String,
    pub created_at: DateTime<Utc>,
    pub access_count: u32,
    pub coords_2d: (f32, f32),  // UMAP projection 결과
    pub top_neighbors: Vec<MemoryNeighbor>,  // similarity > 0.7, top 5
}

pub struct MemoryNeighbor {
    pub id: MemoryId,
    pub similarity: f32,
}
```

**UMAP은 어디서?**
- 옵션 A: **프론트엔드**에서 `umap-js`로 실행 (소규모는 가능, 1000+ 노드는 느림)
- 옵션 B: **백엔드**에서 Python FFI 또는 Rust crate (`umap-rs` 아직 미성숙 → `linfa-umap`은 stable, 0.7+)
- 옵션 C: **사전 계산** → `dream` 시 consolidation 후 캐시 → 단순 조회

→ **옵션 C 권장**. dream cycle이 이미 임베딩을 다루니까 (RFC-008), 그 단계에서 UMAP projection을 디스크 캐시. API는 캐시된 좌표만 반환. 비용 0.

### 단계별 작업

### Step 1: 백엔드 — UMAP projection 캐시 (4시간)
- `crates/oxios-kernel/src/memory/sona.rs` 또는 신규 `memory/embedding_viz.rs`
- `MemoryManager::compute_2d_projection() -> Vec<(MemoryId, (f32, f32))>`
- 꿈 consolidation 시 자동 갱신, 또는 별도 트리거
- `/api/memory/map` 핸들러

### Step 2: d3-force + Canvas 셋업 (3시간)
- `embedding-canvas.tsx` 기본 셋업
- 줌/팬, 노드 렌더링, hover effect
- 500 노드까지 60fps 확인 (Canvas 필수, SVG는 100개도 끊김)

### Step 3: 인코딩 + 인터랙션 (3시간)
- Tier/Type 시각 인코딩
- 엣지 (top-k similarity) 렌더
- SelectionPanel + Detail modal 연결

### Step 4: 필터 + 애니메이션 (2시간)
- Tier/Type/Time 필터 → d3 filter + transition
- "Re-cluster" 버튼 (force 시뮬레이션 재시작, 5초 fade in)
- "Animate" 토글 (시간 흐름에 따라 새 노드 fade in, 오래된 노드 fade out)

### Step 5: 검색 통합 (1시간)
- Cmd+F → 매칭 노드 강조
- 카메라 fly-to (`d3.zoom().transform()` 트랜지션)

### Step 6: 테스트 + 다듬기 (2시간)
- 100개 / 500개 / 1000개 노드 성능 측정
- E2E: 노드 클릭 → SelectionPanel 표시

**총: ~15시간 (2일)**

---

## 4. 위험 / 주의

| 위험 | 대응 |
|------|------|
| Canvas 텍스트 가독성 (작은 노드) | 줌인 시 텍스트 표시 (LOD) |
| 1000+ 노드에서 force 시뮬레이션 비용 | `d3-force` alpha decay 빠르게, 또는 사전 계산된 좌표 사용 |
| Dream consolidation 좌표 미갱신 | timestamp + "stale" 배지, "Re-cluster" CTA |
| 색맹 사용자 | 채도/명도 외에 모양(type별 shape)도 사용 → 2개 인코딩 |
| 모바일 터치 | d3-zoom은 터치 지원, 하지만 노드 hit-area 24px 이상 |

---

## 5. 의존성

```json
"dependencies": {
  "d3-force": "^3.0.0",
  "d3-selection": "^3.0.0",
  "d3-zoom": "^3.0.0",
  "d3-drag": "^3.0.0",
  "d3-scale-chromatic": "^3.1.0"
},
"devDependencies": {
  "@types/d3-force": "^3.0.10",
  "@types/d3-selection": "^3.0.11",
  "@types/d3-zoom": "^3.0.8",
  "@types/d3-drag": "^3.0.7"
}
```

> Note: `umap-js`는 **프론트 옵션**만 도입. 백엔드 UMAP 사용 시 `linfa-umap` (Rust).

---

## 6. 완료 기준

- [ ] 100개 노드: 60fps 인터랙션
- [ ] 500개 노드: force 시뮬레이션 5초 내 안정화
- [ ] 노드 hover → 강조, click → SelectionPanel
- [ ] Tier/Type 필터 적용
- [ ] Cmd+F 검색 → fly-to
- [ ] Dream consolidation 후 좌표 자동 갱신
- [ ] 색맹 안전 (모양 + 색상)
- [ ] E2E 테스트 1개
