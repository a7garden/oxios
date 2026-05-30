# RFC-011: oxi-sdk 0.23.0 → 0.24.0 마이그레이션 + Model Routing UI

> **상태**: Draft
> **날짜**: 2026-05-30
> **범위**: SDK 업그레이드 + 백엔드 라우팅 엔진 + Web UI 설정/시각화
> **영향 크레이트**: oxios-kernel, oxios-ouroboros, oxios-web (backend + frontend)

---

## 1. 배경

### 1.1 oxi-sdk 0.24.0 핵심 변경사항

| 영역 | 변경 내용 | 영향도 |
|------|-----------|--------|
| **Model Routing** | `ComplexityRouter` + `MultiProviderBuilder` + `FallbackChain` 실구현 | High |
| **Runtime Routing** | `RoutingControl` (runtime toggle, exclude, fallback) 추가 | High |
| **Agent Supervisor** | SDK 내장 `AgentSupervisor` + `AgentHandle` + `SnapshotStore` | Medium |
| **Middleware** | `MiddlewarePipeline`, built-in middleware 정식 구현 | Medium |
| **Observability** | `Tracer`, `CostTracker`, `AuditLog`, `EventStore` 정식 API | Medium |
| **Security** | `Authorizer`, `CapabilitySet`, `SecurityMiddleware` | Low |
| **Coordination** | `WorkQueue`, `SharedMemory`, `Consensus`, `CoordinatedGroup` | Low |
| **Error 타입** | `SdkError` + `SdkResult` 통일 | Low |

### 1.2 호환성

API는 **purely additive**다. 기존 0.23.0 시그니처가 그대로 유지된다. Breaking change 없음.

---

## 2. 현재 구조 분석

### 2.1 Backend 엔진 API (engine_routes.rs)

```
GET  /api/engine/providers          → 프로바이더 목록
GET  /api/engine/models             → 모델 목록 (provider/q 필터)
GET  /api/engine/config             → 현재 엔진 설정
PUT  /api/engine/model              → 기본 모델 설정
PUT  /api/engine/api-key            → API 키 설정
PUT  /api/engine/provider-options   → 프로바이더 옵션 설정
POST /api/engine/validate-key       → API 키 검증
```

### 2.2 Frontend 구조

```
surface/oxios-web/
├── src/
│   ├── routes/
│   │   ├── index.tsx              → 대시보드 (에이전트 상태, 시스템 상태)
│   │   └── settings.tsx           → 설정 페이지 (엔진, 커널, 보안 탭)
│   ├── components/engine/
│   │   ├── provider-select.tsx     → 프로바이더 선택
│   │   ├── model-select.tsx       → 모델 선택
│   │   ├── api-key-input.tsx       → API 키 입력
│   │   └── provider-options.tsx    → 고급 옵션
│   └── hooks/
│       └── use-engine.ts          → TanStack Query hooks (API 호출)
```

### 2.3 현재 빈 공간 (라우팅 관련)

| 위치 | 현재 상태 | 필요 변경 |
|------|----------|-----------|
| `/api/engine/config` 응답 | `model`, `api_key`, `provider_options` | `routing` 필드 추가 |
| `settings.tsx` | 엔진/커널/보안 탭 | **라우팅 탭 추가** |
| `index.tsx` (대시보드) | 에이전트/메모리 카운트 | **모델 사용량 시각화 추가** |
| `use-engine.ts` | providers/models/config hooks | 라우팅 훅 추가 |

---

## 3. 아키텍처 설계

### 3.1 Backend 레이어

```
┌─────────────────────────────────────────────────────────────┐
│  Web UI (React)                                             │
│  ├── /settings → 라우팅 설정 탭                               │
│  └── / (대시보드) → 모델 사용량 + fallback 히스토리 카드        │
└────────────────────────┬────────────────────────────────────┘
                         │ HTTP / SSE
┌────────────────────────▼────────────────────────────────────┐
│  engine_routes.rs                                        │
│  ├── GET  /api/engine/config          (+ 라우팅 상태)       │
│  ├── PUT  /api/engine/routing          (설정 변경)           │
│  ├── GET  /api/engine/routing/stats    (사용량 통계)         │
│  └── GET  /api/engine/routing/fallback-history              │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  OxiosEngine                                        │
│  ├── oxi: Oxi               (provider/model resolution)  │
│  ├── routing_control: RoutingControl                  │
│  │   ├── auto_routing: bool                             │
│  │   ├── prefer_cost_efficient: bool                    │
│  │   ├── fallback_models: Vec<String>                   │
│  │   └── excluded_models: Vec<String>                  │
│  ├── routing_stats: RoutingStats  (사용량 누적)            │
│  └── pools: HashMap<provider, PooledProvider>           │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  oxi-sdk 0.24.0                                       │
│  ├── OxiBuilder.enable_routing(RoutingConfig)          │
│  ├── RoutingControl (runtime toggle)                    │
│  ├── MultiProviderBuilder (complexity-based routing)    │
│  ├── ComplexityRouter (task → model 매핑)               │
│  └── FallbackChain (순차 fallback)                      │
└────────────────────────────────────────────────────────────┘
```

### 3.2 Frontend 라우팅 탭 UI

```
┌──────────────────────────────────────────────────────────────────────────┐
│  라우팅 설정                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─ 자동 라우팅 ──────────────────────────────────────────────────────┐   │
│  │                                                                      │   │
│  │  [ON/OFF]  복잡도 기반 자동 모델 선택 활성화                         │   │
│  │                                                                      │   │
│  │  설명: 작업 복잡도를 분석하여 적절한 모델 자동 선택                  │   │
│  │        (간단한 작업은 Haiku, 복잡한 작업은 Opus)                   │   │
│  │                                                                      │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─ 비용 최적화 ──────────────────────────────────────────────────────┐   │
│  │                                                                      │   │
│  │  [ON/OFF]  가능한 경우 비용 효율적인 모델 선호                       │   │
│  │                                                                      │   │
│  │  설명: 동일 성능 가능 시 더 저렴한 모델 선택                        │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─ fallback 모델 ───────────────────────────────────────────────────┐   │
│  │                                                                      │   │
│  │  1. [ Anthropic / Claude Sonnet 4     ▼ ]  ← 기본                   │   │
│  │  2. [ OpenAI / GPT-4o-mini           ▼ ]                           │   │
│  │  3. [ + fallback 모델 추가 ]                                       │   │
│  │                                                                      │   │
│  │  주요 모델 실패 시 순서대로 시도                                    │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─ 제외 모델 ─────────────────────────────────────────────────────────┐   │
│  │                                                                      │   │
│  │  ┌─model-chip─────────────────┐  ┌─model-chip──────────────────┐    │   │
│  │  │  GPT-4-Turbo    [x]       │  │  Gemini 1.5 Pro  [x]       │    │   │
│  │  └───────────────────────────┘  └─────────────────────────────┘    │   │
│  │                                                                      │   │
│  │  [ + 제외 모델 추가 ]                                              │   │
│  │                                                                      │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  [ 변경사항 저장 ]                                                         │
└──────────────────────────────────────────────────────────────────────────┘
```

### 3.3 대시보드 모델 사용량 시각화

```
┌──────────────────────────────────────────────────────────────────────────┐
│  모델 사용량                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  오늘        │ Claude Sonnet 4  ████████████████  67%  (1,234 calls)    │
│              │ GPT-4o-mini      ██████            25%  (461 calls)       │
│              │ Claude 3.5 Haiku ██                8%   (148 calls)       │
│                                                                          │
│  비용       │ $12.34 (입력: 2.1M 토큰 / 출력: 890K 토큰)                 │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│  최근 fallback 이력                                                         │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  14:32  Sonnet 4 실패 → GPT-4o-mini 성공  (	context exceeded	)        │
│  14:15  Haiku 실패 → Sonnet 4 성공     (	rate limit	)             │
│  13:58  Sonnet 4 실패 → Opus 4 성공     (	timeout	)                │
│                                                                          │
│  [전체 이력 보기 →]                                                        │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 4. 상세 구현 설계

### 4.1 Backend: OxiosEngine에 라우팅 상태 추가

**파일**: `crates/oxios-kernel/src/engine.rs`

```rust
// ── 라우팅 통계 ────────────────────────────────────────────────

/// 라우팅 통계를 추적하기 위한 공유 상태.
#[derive(Default)]
pub struct RoutingStats {
    /// 모델별 호출 카운트.
    pub model_calls: RwLock<HashMap<String, u64>>,
    /// 모델별 총 비용.
    pub model_cost: RwLock<HashMap<String, f64>>,
    /// fallback 발생 이력 (최근 100개).
    pub fallback_history: RwLock<Vec<FallbackEvent>>,
}

/// 단일 fallback 이벤트.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackEvent {
    pub timestamp: DateTime<Utc>,
    pub from_model: String,
    pub to_model: String,
    pub reason: String,
    pub success: bool,
}

/// 엔진 라우팅 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSettings {
    pub auto_routing: bool,
    pub prefer_cost_efficient: bool,
    pub fallback_models: Vec<String>,
    pub excluded_models: Vec<String>,
}

impl Default for RoutingSettings {
    fn default() -> Self {
        Self {
            auto_routing: false,
            prefer_cost_efficient: false,
            fallback_models: Vec::new(),
            excluded_models: Vec::new(),
        }
    }
}

// ── OxiosEngine 확장 ──────────────────────────────────────────

pub struct OxiosEngine {
    oxi: Oxi,
    default_model_id: String,
    routing_control: oxi_sdk::RoutingControl,
    routing_settings: RwLock<RoutingSettings>,  // ← persisted
    routing_stats: RoutingStats,                 // ← in-memory
    pools: RwLock<HashMap<String, Arc<dyn Provider>>>,
}
```

### 4.2 Backend: Engine Routes에 라우팅 API 추가

**파일**: `surface/oxios-web/src/routes/engine_routes.rs`

```rust
// ── Request types ──────────────────────────────────────────────

/// PUT /api/engine/routing — 라우팅 설정 변경
#[derive(Debug, Deserialize)]
pub struct SetRoutingRequest {
    pub auto_routing: Option<bool>,
    pub prefer_cost_efficient: Option<bool>,
    pub fallback_models: Option<Vec<String>>,
    pub excluded_models: Option<Vec<String>>,
}

/// GET /api/engine/routing/stats — 라우팅 통계
#[derive(Debug, Serialize)]
pub struct RoutingStatsResponse {
    pub model_calls: HashMap<String, u64>,
    pub model_cost: HashMap<String, f64>,
    pub total_requests: u64,
    pub fallback_count: u64,
    pub success_rate: f64,
}

/// GET /api/engine/routing/fallback-history — fallback 이력
#[derive(Debug, Serialize)]
pub struct FallbackHistoryResponse {
    pub events: Vec<FallbackEvent>,
    pub total_count: usize,
}

// ── Handlers ───────────────────────────────────────────────────

/// GET /api/engine/config — 현재 엔진 설정 (라우팅 포함)
pub(crate) async fn handle_engine_config(state: State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let routing = state.kernel.engine.routing_settings();
    let config = state.kernel.engine.config();
    Ok(Json(json!({
        "model": config.model,
        "api_key_set": config.api_key_set,
        "provider_options": config.provider_options,
        "routing": routing,
    })))
}

/// PUT /api/engine/routing — 라우팅 설정 변경
pub(crate) async fn handle_engine_set_routing(
    state: State<Arc<AppState>>,
    Json(body): Json<SetRoutingRequest>,
) -> Result<Json<Value>, AppError> {
    state
        .kernel
        .engine
        .update_routing(body.into())
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(json!({ "ok": true })))
}

/// GET /api/engine/routing/stats — 라우팅 통계
pub(crate) async fn handle_engine_routing_stats(
    state: State<Arc<AppState>>,
) -> Result<Json<RoutingStatsResponse>, AppError> {
    let stats = state.kernel.engine.routing_stats();
    Ok(Json(stats))
}

/// GET /api/engine/routing/fallback-history — fallback 이력
pub(crate) async fn handle_engine_routing_fallback_history(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Result<Json<FallbackHistoryResponse>, AppError> {
    let history = state.kernel.engine.fallback_history(params.limit);
    Ok(Json(FallbackHistoryResponse {
        events: history,
        total_count: history.len(),
    }))
}
```

### 4.3 Frontend: use-engine.ts에 라우팅 훅 추가

**파일**: `surface/oxios-web/web/src/hooks/use-engine.ts`

```typescript
// ── Routing hooks ───────────────────────────────────────────────

export function useRoutingConfig() {
  return useQuery<{
    auto_routing: boolean
    prefer_cost_efficient: boolean
    fallback_models: string[]
    excluded_models: string[]
  }>({
    queryKey: ['engine', 'routing', 'config'],
    queryFn: () => api.get('/api/engine/routing/config'),
  })
}

export function useSetRouting() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (body: {
      auto_routing?: boolean
      prefer_cost_efficient?: boolean
      fallback_models?: string[]
      excluded_models?: string[]
    }) => api.put('/api/engine/routing', body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'routing'] })
    },
  })
}

export function useRoutingStats() {
  return useQuery<{
    model_calls: Record<string, number>
    model_cost: Record<string, number>
    total_requests: number
    fallback_count: number
    success_rate: number
  }>({
    queryKey: ['engine', 'routing', 'stats'],
    queryFn: () => api.get('/api/engine/routing/stats'),
    refetchInterval: 30000,
  })
}

export function useFallbackHistory(limit = 50) {
  return useQuery<{ events: FallbackEvent[]; total_count: number }>({
    queryKey: ['engine', 'routing', 'fallback-history', limit],
    queryFn: () => api.get(`/api/engine/routing/fallback-history?limit=${limit}`),
  })
}
```

### 4.4 Frontend: 라우팅 설정 탭

**파일**: `surface/oxios-web/web/src/components/engine/routing-settings.tsx` (신규)

```tsx
import { useTranslation } from 'react-i18next'
import { useRoutingConfig, useSetRouting } from '@/hooks/use-engine'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import { ModelSelect } from './model-select'

export function RoutingSettings() {
  const { t } = useTranslation()
  const { data, isLoading } = useRoutingConfig()
  const setRouting = useSetRouting()

  if (isLoading || !data) return null

  return (
    <div className="space-y-6">
      {/* 자동 라우팅 */}
      <Card>
        <CardHeader>
          <CardTitle>{t('settings.routing.auto')}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-muted-foreground">
                {t('settings.routing.autoDescription')}
              </p>
            </div>
            <Switch
              checked={data.auto_routing}
              onCheckedChange={(checked) =>
                setRouting.mutate({ auto_routing: checked })
              }
            />
          </div>
        </CardContent>
      </Card>

      {/* 비용 최적화 */}
      <Card>
        <CardHeader>
          <CardTitle>{t('settings.routing.costEfficient')}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-muted-foreground">
                {t('settings.routing.costEfficientDescription')}
              </p>
            </div>
            <Switch
              checked={data.prefer_cost_efficient}
              onCheckedChange={(checked) =>
                setRouting.mutate({ prefer_cost_efficient: checked })
              }
            />
          </div>
        </CardContent>
      </Card>

      {/* Fallback 모델 */}
      <Card>
        <CardHeader>
          <CardTitle>{t('settings.routing.fallbackModels')}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          {data.fallback_models.map((model, i) => (
            <ModelSelect key={i} value={model} />
          ))}
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              setRouting.mutate({
                fallback_models: [...data.fallback_models, ''],
              })
            }
          >
            + {t('settings.routing.addFallback')}
          </Button>
        </CardContent>
      </Card>

      {/* 제외 모델 */}
      <Card>
        <CardHeader>
          <CardTitle>{t('settings.routing.excludedModels')}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex flex-wrap gap-2">
            {data.excluded_models.map((model) => (
              <div
                key={model}
                className="flex items-center gap-2 rounded-full bg-muted px-3 py-1 text-sm"
              >
                {model}
                <button
                  onClick={() =>
                    setRouting.mutate({
                      excluded_models: data.excluded_models.filter(
                        (m) => m !== model
                      ),
                    })
                  }
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
```

### 4.5 Frontend: settings.tsx에 라우팅 탭 통합

**파일**: `surface/oxios-web/web/src/routes/settings.tsx` (수정)

```tsx
import { RoutingSettings } from '@/components/engine/routing-settings'

// Tabs에 "routing" 추가
const tabs = [
  { value: 'engine', label: t('settings.tabs.engine'), content: <EngineTab /> },
  { value: 'routing', label: t('settings.tabs.routing'), content: <RoutingSettings /> },
  { value: 'kernel', label: t('settings.tabs.kernel'), content: <KernelTab /> },
  { value: 'security', label: t('settings.tabs.security'), content: <SecurityTab /> },
]
```

### 4.6 Frontend: 대시보드에 모델 사용량 카드 추가

**파일**: `surface/oxios-web/web/src/routes/index.tsx` (수정)

```tsx
import { useRoutingStats } from '@/hooks/use-engine'

function ModelUsageCard() {
  const { data } = useRoutingStats()

  if (!data) return null

  const total = Object.values(data.model_calls).reduce((a, b) => a + b, 0)
  const sorted = Object.entries(data.model_calls)
    .sort(([, a], [, b]) => b - a)
    .slice(0, 5)

  return (
    <Card>
      <CardHeader>
        <CardTitle>모델 사용량</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {sorted.map(([model, count]) => {
          const pct = total > 0 ? (count / total) * 100 : 0
          return (
            <div key={model} className="space-y-1">
              <div className="flex justify-between text-sm">
                <span>{model}</span>
                <span className="text-muted-foreground">
                  {pct.toFixed(0)}% ({count})
                </span>
              </div>
              <Progress value={pct} />
            </div>
          )
        })}
        <div className="pt-2 text-xs text-muted-foreground">
          Fallback: {data.fallback_count}회 | 성공률: {(data.success_rate * 100).toFixed(1)}%
        </div>
      </CardContent>
    </Card>
  )
}
```

---

## 5. 마이그레이션 단계

### Phase 1: SDK 버전업 + 빌드 확인 (~10분)
```
Cargo.toml: oxi-sdk = "0.24.0"
cargo build
```

### Phase 2: Backend 라우팅 상태 구현 (~2시간)
```
engine.rs
  ├── RoutingSettings struct 추가
  ├── routing_stats struct 추가
  ├── FallbackEvent struct 추가
  ├── routing_settings() getter 추가
  ├── update_routing() method 추가
  ├── routing_stats() method 추가
  └── fallback_history() method 추가

engine_routes.rs
  ├── GET /api/engine/routing/config
  ├── PUT /api/engine/routing
  ├── GET /api/engine/routing/stats
  └── GET /api/engine/routing/fallback-history
```

### Phase 3: Frontend 라우팅 설정 탭 (~2시간)
```
use-engine.ts
  ├── useRoutingConfig()
  ├── useSetRouting()
  ├── useRoutingStats()
  └── useFallbackHistory()

components/engine/routing-settings.tsx (신규)
settings.tsx (탭 추가)
```

### Phase 4: Frontend 대시보드 시각화 (~1시간)
```
index.tsx
  ├── ModelUsageCard 추가
  └── FallbackHistoryCard 추가 (선택)
```

### Phase 5: 라우팅 로깅 통합 (~1시간)
```
AgentRuntime 실행 시:
  1. 라우팅이 active면 ComplexityRouter가 호출되는 시점 로깅
  2. fallback 발생 시 FallbackEvent 기록
  3. 성공/실패 결과 후 model_calls, model_cost 업데이트
```

---

## 6. 파일별 변경 요약

| 파일 | 변경 유형 | 설명 |
|------|-----------|------|
| `Cargo.toml` | 수정 | `oxi-sdk = "0.24.0"` |
| `crates/oxios-kernel/src/engine.rs` | 수정 | 라우팅 상태/통계 추가 |
| `surface/oxios-web/src/routes/engine_routes.rs` | 수정 | 라우팅 API 4개 추가 |
| `surface/oxios-web/src/routes/mod.rs` | 수정 | 라우팅 핸들러 export |
| `surface/oxios-web/web/src/hooks/use-engine.ts` | 수정 | 라우팅 훅 4개 추가 |
| `surface/oxios-web/web/src/components/engine/routing-settings.tsx` | **신규** | 라우팅 설정 UI |
| `surface/oxios-web/web/src/routes/settings.tsx` | 수정 | 라우팅 탭 추가 |
| `surface/oxios-web/web/src/routes/index.tsx` | 수정 | 모델 사용량 카드 추가 |
| `i18n/ko.json` | 수정 | 라우팅 관련 번역 키 추가 |
| `docs/rfc-011-oxi-sdk-0.24-migration.md` | 수정 | 본 RFC (이 파일) |

---

## 7. 우선순위 및 일정

```
Week 1:
  ├── Phase 1 (SDK 버전업)              ← 하루 안에 끝
  ├── Phase 2 (Backend 라우팅 상태)     ← 핵심 로직
  └── Phase 3 (Frontend 설정 탭)        ← 사용자-facing

Week 2:
  ├── Phase 4 (대시보드 시각화)          ← 있으면 좋음
  └── Phase 5 (로깅 통합)               ← 모니터링 완성

총 예상 시간: ~8시간 (backend 5h + frontend 3h)
```

---

## 8.風險 및 완화

| 위험 | 가능성 | 영향 | 완화 |
|------|--------|------|------|
| SDK API 미스매치 | 낮음 | 중간 | Phase 1 빌드 확인으로 즉시 감지 |
| 라우팅 설정 persisted 형태 | 중간 | 중간 | `RoutingSettings`를 config.toml에 별도 섹션으로 저장 |
| 라우팅 stats 메모리 누수 | 낮음 | 낮음 | `fallback_history`는 고정 크기 circular buffer |
| UI 상태와 백엔드 불일치 | 낮음 | 중간 | TanStack Query invalidation으로 동기화 |

---

## 9. 테스트

1. **Backend**: `cargo test -p oxios-kernel` — engine 라우팅 테스트
2. **API**: `curl http://localhost:4200/api/engine/routing/config` — 설정 조회
3. **API**: `curl -X PUT http://localhost:4200/api/engine/routing -d '{"auto_routing": true}'` — 설정 변경
4. **Frontend**: `cd surface/oxios-web/web && bun dev` → `/settings` → 라우팅 탭 동작 확인
5. **E2E**: `/settings` → 라우팅 on → 채팅 → fallback 발생 → 대시보드에서 확인

---

## 10. 결론

이 RFC는 기존 rfc-011을 **Web UI까지 포함하는 완전한 설계**로 확장한다.

- Phase 1 (SDK 버전업): 10분, breaking change 없음
- Phase 2 (Backend): 2시간, 핵심 로직
- Phase 3 (Frontend 설정): 2시간, 사용자-facing
- Phase 4 (Frontend 시각화): 1시간, 대시보드 보강
- Phase 5 (로깅): 1시간, 모니터링 완성

**전체工期: ~6시간** (설계 제외, 구현만)

백엔드 없이 UI만 만드는 것은 의미 없으므로, Phase 2 → 3 순서로 진행하는 것을 권장한다.