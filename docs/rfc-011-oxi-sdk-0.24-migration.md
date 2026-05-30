# RFC-011: oxi-sdk 0.23.0 → 0.24.0 마이그레이션 + Model Routing UI

> **상태**: Draft v2 (리뷰 반영)
> **날짜**: 2026-05-30
> **범위**: SDK 업그레이드 + 라우팅 설정/통계 API + Web UI 설정/시각화
> **영향 크레이트**: oxios-kernel, oxios-ouroboros, oxios-web (backend + frontend)

---

## 1. 배경

### 1.1 oxi-sdk 0.24.0 핵심 변경

SDK 0.24.0은 **purely additive**다. 기존 0.23.0 API 시그니처가 그대로 유지된다.

| 영역 | 변경 내용 |
|------|-----------|
| **Model Routing** | `ComplexityRouter` + `MultiProviderBuilder` + `FallbackChain` 실구현 |
| **Runtime Routing** | `RoutingControl` — runtime toggle, model exclude, fallback 교체 |
| **Middleware** | `MiddlewarePipeline` + built-in middleware 정식 구현 |
| **Lifecycle** | `AgentSupervisor` + `AgentHandle` + `SnapshotStore` |

### 1.2 기존 라우팅 인프라

oxios에 **이미** 라우팅 기반이 존재한다:

```rust
// crates/oxios-kernel/src/config.rs — EngineConfig에 라우팅 필드 이미 있음
pub struct EngineConfig {
    pub routing_enabled: bool,        // ← 존재
    pub prefer_cost_efficient: bool,  // ← 존재
    pub fallback_models: Vec<String>, // ← 존재
    // excluded_models — 없음, 추가 필요
}

// src/kernel.rs — routing_enabled를 이미 체크
let engine = if config.engine.routing_enabled {
    let (engine, _routing_control) = engine_builder.build_with_routing();
    ...
}
```

```text
현재 데이터 흐름:

config.toml ──→ OxiosConfig ──→ EngineConfig
                                    │
     ┌──────────────────────────────┘
     │
     ├── EngineApi (KernelHandle.engine) ←── Web routes 읽기/쓰기
     │     config: Arc<RwLock<OxiosConfig>>
     │     config_path: PathBuf
     │
     └── OxiosEngine (Kernel 내부, Web 비노출)
           oxi: Oxi
           routing_control: Option<RoutingControl>
```

### 1.3 리뷰에서 발견한 설계 오류 (v1)

| # | 이슈 | 원인 |
|---|------|------|
| 1 | `RoutingSettings` struct가 기존 `EngineConfig`과 중복 | 기존 필드 미확인 |
| 2 | Web route가 `OxiosEngine`에 직접 접근한다고 설계 | `EngineApi` 퍼사드 무시 |
| 3 | API 엔드포인트가 기존 패턴과 불일치 | `/config` 확장 대신 별도 생성 |
| 4 | 통계를 `OxiosEngine`에 저장 | Web 접근 불가 |
| 5 | `excluded_models`가 config에 없음 | 누락 |

v2에서는 이 모든 문제를 해결한다.

---

## 2. 아키텍처 (수정됨)

### 2.1 레이어 다이어그램

```
┌─────────────────────────────────────────────────────────────────────┐
│  React Frontend                                                    │
│  ├── /settings → Engine 탭 내 라우팅 섹션                           │
│  └── / (대시보드) → 모델 사용량 카드                                │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ HTTP
┌───────────────────────────▼─────────────────────────────────────────┐
│  engine_routes.rs (Axum handlers)                                  │
│  ├── GET  /api/engine/config          ← 라우팅 설정 포함 확장       │
│  ├── PUT  /api/engine/routing          ← 라우팅 설정 쓰기            │
│  ├── GET  /api/engine/routing/stats    ← 모델별 사용 통계           │
│  └── GET  /api/engine/routing/fallbacks ← fallback 이력             │
└───────────────────────────┬─────────────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────────────┐
│  EngineApi (KernelHandle.engine)                                   │
│  ├── config: Arc<RwLock<OxiosConfig>>   ← routing 설정 읽기/쓰기    │
│  ├── config_path: PathBuf               ← config.toml persist       │
│  └── routing_stats: Arc<RoutingStats>   ← in-memory 모델 사용량     │
│                                                                      │
│  메서드:                                                             │
│  ├── config()          → EngineConfigResponse (+ routing 섹션)      │
│  ├── set_routing()     → config.toml에 라우팅 설정 persist          │
│  ├── routing_stats()   → 모델별 호출/비용 통계                      │
│  └── fallback_history() → 최근 fallback 이력                       │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ Arc<RoutingStats> 공유
┌───────────────────────────▼─────────────────────────────────────────┐
│  AgentRuntime                                                      │
│  └── AgentEvent::Usage 후킹 → routing_stats.record_usage()        │
│      (이미 구현됨, stats 기록만 추가)                               │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 핵심 원칙

1. **EngineApi를 통해서만 Web 노출** — `OxiosEngine`은 Kernel 내부
2. **기존 `EngineConfig` 필드 재사용** — 새 struct 만들지 않음
3. **`Arc<RoutingStats>` 공유** — `EngineApi`와 `AgentRuntime`이 동일 인스턴스 참조
4. **기존 API 패턴 준수** — 읽기는 `/config` 확장, 쓰기는 전용 엔드포인트

---

## 3. 상세 변경 설계

### 3.1 config.rs: `excluded_models` 필드 추가

```rust
// crates/oxios-kernel/src/config.rs

pub struct EngineConfig {
    pub default_model: String,
    pub api_key: Option<String>,
    pub provider_options: Option<oxi_sdk::ProviderOptions>,
    pub routing_enabled: bool,          // 기존
    pub prefer_cost_efficient: bool,    // 기존
    pub fallback_models: Vec<String>,   // 기존
    pub excluded_models: Vec<String>,   // ← 신규
}
```

TOML에 `[engine]` 섹션에 자동 직렬화:
```toml
[engine]
default_model = "anthropic/claude-sonnet-4-20250514"
routing_enabled = true
prefer_cost_efficient = false
fallback_models = ["openai/gpt-4o-mini"]
excluded_models = []
```

### 3.2 engine_api.rs: 라우팅 읽기/쓰기/통계 추가

```rust
// crates/oxios-kernel/src/kernel_handle/engine_api.rs

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

// ── 신규 타입 ──────────────────────────────────────────────────

/// 라우팅 설정 스냅샷 (읽기 전용 응답).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfigSnapshot {
    pub routing_enabled: bool,
    pub prefer_cost_efficient: bool,
    pub fallback_models: Vec<String>,
    pub excluded_models: Vec<String>,
}

/// 모델별 사용 통계.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingStatsSnapshot {
    /// 모델ID → 호출 횟수.
    pub model_calls: HashMap<String, u64>,
    /// 모델ID → 추정 비용 (USD).
    pub model_cost: HashMap<String, f64>,
    /// 총 요청 수.
    pub total_requests: u64,
    /// 총 비용 (USD).
    pub total_cost: f64,
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

/// 라우팅 설정 변경 요청.
#[derive(Debug, Deserialize)]
pub struct RoutingUpdate {
    pub routing_enabled: Option<bool>,
    pub prefer_cost_efficient: Option<bool>,
    pub fallback_models: Option<Vec<String>>,
    pub excluded_models: Option<Vec<String>>,
}

// ── In-memory 통계 스토어 ─────────────────────────────────────

/// 라우팅 통계 (공유 가능).
/// `EngineApi`와 `AgentRuntime`이 `Arc`로 공유.
pub struct RoutingStats {
    /// 모델ID → 호출 횟수.
    calls: RwLock<HashMap<String, u64>>,
    /// 모델ID → 누적 비용.
    costs: RwLock<HashMap<String, f64>>,
    /// Fallback 이력 (최근 200개, circular).
    fallbacks: RwLock<Vec<FallbackEvent>>,
}

impl Default for RoutingStats {
    fn default() -> Self {
        Self {
            calls: RwLock::new(HashMap::new()),
            costs: RwLock::new(HashMap::new()),
            fallbacks: RwLock::new(Vec::new()),
        }
    }
}

impl RoutingStats {
    pub fn new() -> Self { Self::default() }

    /// 모델 사용 1회 기록.
    pub fn record_model_usage(&self, model_id: &str, cost_usd: f64) {
        let mut calls = self.calls.write();
        *calls.entry(model_id.to_string()).or_insert(0) += 1;

        if cost_usd > 0.0 {
            let mut costs = self.costs.write();
            *costs.entry(model_id.to_string()).or_insert(0.0) += cost_usd;
        }
    }

    /// Fallback 이벤트 기록 (최대 200개 유지).
    pub fn record_fallback(&self, event: FallbackEvent) {
        let mut fb = self.fallbacks.write();
        fb.push(event);
        if fb.len() > 200 {
            fb.drain(0..fb.len() - 200);
        }
    }

    /// 스냅샷 반환.
    pub fn snapshot(&self) -> RoutingStatsSnapshot {
        let calls = self.calls.read();
        let costs = self.costs.read();
        let total_requests: u64 = calls.values().sum();
        let total_cost: f64 = costs.values().sum();
        RoutingStatsSnapshot {
            model_calls: calls.clone(),
            model_cost: costs.clone(),
            total_requests,
            total_cost,
        }
    }

    /// 최근 fallback 이력 반환.
    pub fn fallback_history(&self, limit: usize) -> Vec<FallbackEvent> {
        let fb = self.fallbacks.read();
        fb.iter().rev().take(limit).cloned().collect()
    }
}

// ── EngineApi 확장 ────────────────────────────────────────────

pub struct EngineApi {
    config: Arc<RwLock<OxiosConfig>>,
    config_path: PathBuf,
    routing_stats: Arc<RoutingStats>,  // ← 신규
}

impl EngineApi {
    pub fn new(
        config: Arc<RwLock<OxiosConfig>>,
        config_path: PathBuf,
        routing_stats: Arc<RoutingStats>,  // ← 신규 파라미터
    ) -> Self {
        Self { config, config_path, routing_stats }
    }

    // ── 기존 config() 응답에 routing 추가 ──────────────────────

    /// 기존 EngineConfigResponse에 routing 필드를 포함하여 반환.
    /// Web의 GET /api/engine/config가 이걸 그대로 JSON으로 보냄.
    pub fn config(&self) -> EngineConfigResponse {
        let cfg = self.config.read();
        let provider = CredentialStore::provider_from_model(&cfg.engine.default_model)
            .map(|s| s.to_string());
        // ... 기존 필드 ...
        let routing = RoutingConfigSnapshot {
            routing_enabled: cfg.engine.routing_enabled,
            prefer_cost_efficient: cfg.engine.prefer_cost_efficient,
            fallback_models: cfg.engine.fallback_models.clone(),
            excluded_models: cfg.engine.excluded_models.clone(),
        };
        EngineConfigResponse {
            default_model: cfg.engine.default_model.clone(),
            api_key_set,
            api_key_source,
            provider,
            routing,  // ← 신규 필드
        }
    }

    // ── 라우팅 설정 쓰기 ──────────────────────────────────────

    /// 라우팅 설정 변경. 변경된 필드만 업데이트하여 config.toml에 persist.
    pub fn set_routing(&self, update: RoutingUpdate) -> anyhow::Result<()> {
        {
            let mut cfg = self.config.write();
            if let Some(v) = update.routing_enabled {
                cfg.engine.routing_enabled = v;
            }
            if let Some(v) = update.prefer_cost_efficient {
                cfg.engine.prefer_cost_efficient = v;
            }
            if let Some(v) = update.fallback_models {
                cfg.engine.fallback_models = v;
            }
            if let Some(v) = update.excluded_models {
                cfg.engine.excluded_models = v;
            }
            self.persist(&cfg)?;
        }
        tracing::info!("Routing config updated in config.toml");
        Ok(())
    }

    // ── 라우팅 통계 ───────────────────────────────────────────

    /// routing_stats에 대한 참조 반환.
    /// AgentRuntime이 기록용으로 사용.
    pub fn routing_stats(&self) -> &Arc<RoutingStats> {
        &self.routing_stats
    }

    /// 통계 스냅샵 반환 (Web API용).
    pub fn routing_stats_snapshot(&self) -> RoutingStatsSnapshot {
        self.routing_stats.snapshot()
    }

    /// Fallback 이력 반환 (Web API용).
    pub fn fallback_history(&self, limit: usize) -> Vec<FallbackEvent> {
        self.routing_stats.fallback_history(limit)
    }

    // ── 모델 비용 추정 (유틸) ────────────────────────────────

    /// model_db에서 모델 단가를 조회하여 토큰 사용량으로 비용 추정.
    pub fn estimate_cost(model_id: &str, input_tokens: u64, output_tokens: u64) -> f64 {
        let entry = oxi_sdk::get_model_entry(model_id);
        match entry {
            Some(e) => {
                (e.cost_input * input_tokens as f64 / 1_000_000.0)
                    + (e.cost_output * output_tokens as f64 / 1_000_000.0)
            }
            None => 0.0,
        }
    }
}
```

### 3.3 EngineConfigResponse에 routing 필드 추가

```rust
// engine_api.rs — 기존 응답 타입에 routing 추가

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfigResponse {
    pub default_model: String,
    pub api_key_set: bool,
    pub api_key_source: Option<String>,
    pub provider: Option<String>,
    pub routing: RoutingConfigSnapshot,  // ← 신규
}
```

### 3.4 AgentRuntime: 모델 사용량 기록 후킹

```rust
// crates/oxios-kernel/src/agent_runtime.rs
// 기존 AgentEvent::Usage 핸들러에 stats 기록 추가

AgentEvent::Usage { input_tokens, output_tokens } => {
    // 기존: cost_tracker에 기록 (이미 구현됨)
    crate::observability::cost_tracker().record(...);

    // 신규: routing_stats에 모델 사용량 기록
    if let Some(stats) = &self.routing_stats {
        let cost = EngineApi::estimate_cost(
            &model_id_for_callback,
            *input_tokens as u64,
            *output_tokens as u64,
        );
        stats.record_model_usage(&model_id_for_callback, cost);
    }
}
```

**`AgentRuntime`에 `routing_stats: Option<Arc<RoutingStats>>` 필드 추가**:
- `Kernel`이 `AgentRuntime` 생성 시 `EngineApi.routing_stats()`의 `Arc`를 전달
- 라우팅이 비활성인 경우 `None` (오버헤드 없음)

### 3.5 KernelHandle 생성 수정

```rust
// crates/oxios-kernel/src/kernel_handle/mod.rs

pub struct KernelHandle {
    // ... 기존 13개 API ...
    pub engine: EngineApi,  // ← 생성자 파라미터 변경
}

// src/kernel.rs — Kernel 빌더
let routing_stats = Arc::new(RoutingStats::new());

let engine_api = EngineApi::new(
    config_clone,
    config_path.clone(),
    Arc::clone(&routing_stats),  // ← 공유
);

// AgentRuntime 생성 시
let agent_runtime = AgentRuntime::new(
    Arc::clone(&engine),
    model_id,
    kernel_handle.clone(),
    Some(Arc::clone(&routing_stats)),  // ← 공유
);
```

---

## 4. Backend API Routes

### 4.1 기존 엔드포인트 확장

```
GET /api/engine/config
```

**응답에 `routing` 섹션 추가** (기존 응답과 하위 호환):

```json
{
  "default_model": "anthropic/claude-sonnet-4-20250514",
  "api_key_set": true,
  "api_key_source": "env",
  "provider": "anthropic",
  "routing": {
    "routing_enabled": false,
    "prefer_cost_efficient": false,
    "fallback_models": [],
    "excluded_models": []
  }
}
```

### 4.2 신규 엔드포인트

```
PUT /api/engine/routing          ← 라우팅 설정 변경
GET /api/engine/routing/stats    ← 모델별 사용 통계
GET /api/engine/routing/fallbacks ← fallback 이력
```

**`PUT /api/engine/routing`**:
```json
// 요청 (변경할 필드만 전송)
{
  "routing_enabled": true,
  "prefer_cost_efficient": true,
  "fallback_models": ["openai/gpt-4o-mini", "anthropic/claude-3-5-haiku-20241022"]
}

// 응답
{ "ok": true }
```

**`GET /api/engine/routing/stats`**:
```json
{
  "model_calls": {
    "anthropic/claude-sonnet-4-20250514": 1234,
    "openai/gpt-4o-mini": 461
  },
  "model_cost": {
    "anthropic/claude-sonnet-4-20250514": 10.52,
    "openai/gpt-4o-mini": 1.82
  },
  "total_requests": 1695,
  "total_cost": 12.34
}
```

**`GET /api/engine/routing/fallbacks?limit=20`**:
```json
{
  "events": [
    {
      "timestamp": "2026-05-30T14:32:00Z",
      "from_model": "anthropic/claude-sonnet-4-20250514",
      "to_model": "openai/gpt-4o-mini",
      "reason": "rate_limit",
      "success": true
    }
  ],
  "total_count": 3
}
```

### 4.3 engine_routes.rs 핸들러

```rust
// surface/oxios-web/src/routes/engine_routes.rs

/// GET /api/engine/config — 기존 핸들러, routing 필드 자동 포함
/// (EngineConfigResponse에 routing 필드가 추가되었으므로 코드 변경 불필요)
pub(crate) async fn handle_engine_config(
    state: State<Arc<AppState>>,
) -> Result<Json<EngineConfigResponse>, AppError> {
    let config = state.kernel.engine.config();
    Ok(Json(config))
}

/// PUT /api/engine/routing — 라우팅 설정 변경
pub(crate) async fn handle_engine_set_routing(
    state: State<Arc<AppState>>,
    Json(body): Json<RoutingUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .engine
        .set_routing(body)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!("Routing config updated via API");
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/engine/routing/stats — 모델별 사용 통계
pub(crate) async fn handle_engine_routing_stats(
    state: State<Arc<AppState>>,
) -> Result<Json<RoutingStatsSnapshot>, AppError> {
    Ok(Json(state.kernel.engine.routing_stats_snapshot()))
}

/// GET /api/engine/routing/fallbacks — fallback 이력
pub(crate) async fn handle_engine_routing_fallbacks(
    state: State<Arc<AppState>>,
    Query(params): Query<PageParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let events = state.kernel.engine.fallback_history(params.limit);
    Ok(Json(serde_json::json!({
        "events": events,
        "total_count": events.len(),
    })))
}
```

### 4.4 mod.rs 라우트 등록

```rust
// surface/oxios-web/src/routes/mod.rs — build_routes()에 추가

// Engine routing
.route("/api/engine/routing", put(handle_engine_set_routing))
.route("/api/engine/routing/stats", get(handle_engine_routing_stats))
.route("/api/engine/routing/fallbacks", get(handle_engine_routing_fallbacks))
```

---

## 5. Frontend

### 5.1 types/engine.ts 타입 추가

```typescript
// surface/oxios-web/web/src/types/engine.ts

export interface RoutingConfig {
  routing_enabled: boolean
  prefer_cost_efficient: boolean
  fallback_models: string[]
  excluded_models: string[]
}

export interface RoutingStats {
  model_calls: Record<string, number>
  model_cost: Record<string, number>
  total_requests: number
  total_cost: number
}

export interface FallbackEvent {
  timestamp: string
  from_model: string
  to_model: string
  reason: string
  success: boolean
}
```

### 5.2 hooks/use-engine.ts 훅 추가

```typescript
// surface/oxios-web/web/src/hooks/use-engine.ts

import type { RoutingConfig, RoutingStats, FallbackEvent } from '@/types/engine'

/** GET /api/engine/config의 routing 섹션 */
export function useRoutingConfig() {
  const { data } = useQuery({
    queryKey: ['engine', 'config'],
    queryFn: () => api.get<EngineConfigResponse & { routing: RoutingConfig }>('/api/engine/config'),
  })
  return { data: data?.routing }
}

/** PUT /api/engine/routing */
export function useSetRouting() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (body: Partial<RoutingConfig>) =>
      api.put('/api/engine/routing', body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'config'] })
    },
  })
}

/** GET /api/engine/routing/stats */
export function useRoutingStats() {
  return useQuery<RoutingStats>({
    queryKey: ['engine', 'routing', 'stats'],
    queryFn: () => api.get('/api/engine/routing/stats'),
    refetchInterval: 30000,
  })
}

/** GET /api/engine/routing/fallbacks */
export function useFallbackHistory(limit = 20) {
  return useQuery<{ events: FallbackEvent[]; total_count: number }>({
    queryKey: ['engine', 'routing', 'fallbacks', limit],
    queryFn: () => api.get(`/api/engine/routing/fallbacks?limit=${limit}`),
  })
}
```

### 5.3 settings.tsx: Engine 탭 내 라우팅 섹션

**새 탭이 아닌 기존 Engine 탭 내에 라우팅 섹션을 추가**한다.
설정 페이지의 탭 구조를 변경하지 않고, Engine 탭 하단에 자연스럽게 배치.

```
┌─ Engine 탭 ─────────────────────────────────────────────────────┐
│                                                                  │
│  프로바이더: [ Anthropic ▼ ]                                     │
│  모델:      [ Claude Sonnet 4 ▼ ]                                │
│  API 키:    [ •••••••• ]    [검증]                                │
│                                                                  │
│  ┌─ 라우팅 ──────────────────────────────────────────────────┐  │
│  │                                                            │  │
│  │  [ON/OFF] 자동 모델 라우팅                                 │  │
│  │  작업 복잡도에 따라 적절한 모델 자동 선택                  │  │
│  │                                                            │  │
│  │  [ON/OFF] 비용 최적화                                      │  │
│  │  동일 성능 시 더 저렴한 모델 우선                          │  │
│  │                                                            │  │
│  │  Fallback 모델:                                            │  │
│  │  1. [ GPT-4o-mini ▼ ] [x]                                 │  │
│  │  2. [ Claude 3.5 Haiku ▼ ] [x]                            │  │
│  │  [+ 추가]                                                  │  │
│  │                                                            │  │
│  │  제외 모델:                                                │  │
│  │  [GPT-4-Turbo ×] [Gemini 1.5 Pro ×]  [+ 추가]            │  │
│  │                                                            │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

**구현** (`surface/oxios-web/web/src/components/engine/routing-section.tsx`):

```tsx
import { useTranslation } from 'react-i18next'
import { useRoutingConfig, useSetRouting } from '@/hooks/use-engine'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { ModelSelect } from './model-select'

export function RoutingSection() {
  const { t } = useTranslation()
  const { data: routing } = useRoutingConfig()
  const setRouting = useSetRouting()

  if (!routing) return null

  const update = (patch: Partial<typeof routing>) => setRouting.mutate(patch)

  return (
    <>
      <Separator className="my-6" />
      <div className="space-y-4">
        <h3 className="text-lg font-semibold">{t('settings.routing.title')}</h3>

        {/* 자동 라우팅 */}
        <div className="flex items-center justify-between">
          <div>
            <Label>{t('settings.routing.auto')}</Label>
            <p className="text-sm text-muted-foreground">
              {t('settings.routing.autoDesc')}
            </p>
          </div>
          <Switch
            checked={routing.routing_enabled}
            onCheckedChange={(v) => update({ routing_enabled: v })}
          />
        </div>

        {/* 비용 최적화 */}
        <div className="flex items-center justify-between">
          <div>
            <Label>{t('settings.routing.costEfficient')}</Label>
            <p className="text-sm text-muted-foreground">
              {t('settings.routing.costEfficientDesc')}
            </p>
          </div>
          <Switch
            checked={routing.prefer_cost_efficient}
            onCheckedChange={(v) => update({ prefer_cost_efficient: v })}
          />
        </div>

        {/* Fallback 모델 */}
        <div className="space-y-2">
          <Label>{t('settings.routing.fallbacks')}</Label>
          <p className="text-sm text-muted-foreground">
            {t('settings.routing.fallbacksDesc')}
          </p>
          <div className="space-y-2">
            {routing.fallback_models.map((model, i) => (
              <div key={i} className="flex items-center gap-2">
                <span className="text-sm text-muted-foreground">{i + 1}.</span>
                <ModelSelect value={model} />
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() =>
                    update({
                      fallback_models: routing.fallback_models.filter((_, j) => j !== i),
                    })
                  }
                >
                  ×
                </Button>
              </div>
            ))}
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              update({ fallback_models: [...routing.fallback_models, ''] })
            }
          >
            + {t('settings.routing.addFallback')}
          </Button>
        </div>

        {/* 제외 모델 */}
        <div className="space-y-2">
          <Label>{t('settings.routing.excluded')}</Label>
          <div className="flex flex-wrap gap-2">
            {routing.excluded_models.map((model) => (
              <span
                key={model}
                className="inline-flex items-center gap-1 rounded-full bg-muted px-3 py-1 text-sm"
              >
                {model}
                <button
                  className="ml-1 text-muted-foreground hover:text-foreground"
                  onClick={() =>
                    update({
                      excluded_models: routing.excluded_models.filter((m) => m !== model),
                    })
                  }
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        </div>
      </div>
    </>
  )
}
```

**settings.tsx Engine 탭에 `<RoutingSection />` 추가만 하면 됨**.

### 5.4 대시보드: 모델 사용량 카드

```tsx
// surface/oxios-web/web/src/routes/index.tsx에 추가

import { useRoutingStats } from '@/hooks/use-engine'
import { Progress } from '@/components/ui/progress'

function ModelUsageCard() {
  const { t } = useTranslation()
  const { data } = useRoutingStats()

  if (!data || data.total_requests === 0) return null

  const sorted = Object.entries(data.model_calls)
    .sort(([, a], [, b]) => b - a)
    .slice(0, 5)

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium">
          {t('dashboard.modelUsage')}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {sorted.map(([model, count]) => {
          const pct = (count / data.total_requests) * 100
          return (
            <div key={model} className="space-y-1">
              <div className="flex justify-between text-xs">
                <span className="truncate max-w-[60%]">{model}</span>
                <span className="text-muted-foreground">
                  {pct.toFixed(0)}% ({count})
                </span>
              </div>
              <Progress value={pct} className="h-1.5" />
            </div>
          )
        })}
        <div className="pt-1 text-xs text-muted-foreground">
          ${data.total_cost.toFixed(2)} 총 비용
        </div>
      </CardContent>
    </Card>
  )
}
```

### 5.5 i18n 번역 키

```json
// surface/oxios-web/web/src/i18n/ko.json (추가분)
{
  "settings.routing.title": "라우팅",
  "settings.routing.auto": "자동 모델 라우팅",
  "settings.routing.autoDesc": "작업 복잡도에 따라 적절한 모델을 자동 선택합니다",
  "settings.routing.costEfficient": "비용 최적화",
  "settings.routing.costEfficientDesc": "동일 성능 시 더 저렴한 모델을 우선 선택합니다",
  "settings.routing.fallbacks": "Fallback 모델",
  "settings.routing.fallbacksDesc": "주요 모델 실패 시 순서대로 시도합니다",
  "settings.routing.addFallback": "Fallback 모델 추가",
  "settings.routing.excluded": "제외 모델",
  "dashboard.modelUsage": "모델 사용량"
}
```

---

## 6. 파일별 변경 요약

| 파일 | 유형 | 변경 내용 |
|------|------|-----------|
| `Cargo.toml` | 수정 | `oxi-sdk = "0.24.0"` |
| `crates/oxios-kernel/src/config.rs` | 수정 | `EngineConfig`에 `excluded_models` 필드 추가 |
| `crates/oxios-kernel/src/kernel_handle/engine_api.rs` | 수정 | `RoutingStats`, `RoutingConfigSnapshot`, `FallbackEvent`, `RoutingUpdate` 타입 추가; `EngineApi`에 `routing_stats` 필드, `set_routing()`, `routing_stats_snapshot()`, `fallback_history()`, `estimate_cost()` 메서드 추가; `EngineConfigResponse`에 `routing` 필드 추가 |
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | 수정 | `EngineApi::new()` 시그니처에 `routing_stats` 파라미터 추가; re-export 신규 타입 |
| `crates/oxios-kernel/src/agent_runtime.rs` | 수정 | `routing_stats: Option<Arc<RoutingStats>>` 필드; `AgentEvent::Usage` 핸들러에 `record_model_usage()` 호출 추가 |
| `src/kernel.rs` | 수정 | `RoutingStats` 생성, `EngineApi`와 `AgentRuntime`에 공유 |
| `crates/oxios-kernel/src/lib.rs` | 수정 | 신규 타입 re-export |
| `surface/oxios-web/src/routes/engine_routes.rs` | 수정 | 라우팅 핸들러 3개 추가 |
| `surface/oxios-web/src/routes/mod.rs` | 수정 | 라우팅 라우트 3개 등록 |
| `surface/oxios-web/web/src/types/engine.ts` | 수정 | `RoutingConfig`, `RoutingStats`, `FallbackEvent` 타입 |
| `surface/oxios-web/web/src/hooks/use-engine.ts` | 수정 | 라우팅 훅 4개 |
| `surface/oxios-web/web/src/components/engine/routing-section.tsx` | **신규** | 라우팅 설정 섹션 컴포넌트 |
| `surface/oxios-web/web/src/routes/settings.tsx` | 수정 | Engine 탭에 `<RoutingSection />` 추가 |
| `surface/oxios-web/web/src/routes/index.tsx` | 수정 | `<ModelUsageCard />` 추가 |
| `surface/oxios-web/web/src/i18n/ko.json` | 수정 | 라우팅 번역 키 9개 |

---

## 7. 작업 순서

```
Phase 1: SDK 버전업                          [~10분]
   └── Cargo.toml: oxi-sdk = "0.24.0"
   └── cargo build && cargo test --workspace

Phase 2: Backend — config + engine_api 확장   [~2시간]
   ├── config.rs: excluded_models 추가
   ├── engine_api.rs: 라우팅 타입 + 메서드
   ├── kernel.rs: RoutingStats 공유
   └── agent_runtime.rs: Usage 후킹

Phase 3: Backend — API routes                 [~1시간]
   ├── engine_routes.rs: 핸들러 3개
   └── mod.rs: 라우트 등록

Phase 4: Frontend — 설정 UI                   [~2시간]
   ├── types/engine.ts
   ├── hooks/use-engine.ts
   ├── routing-section.tsx (신규)
   ├── settings.tsx (수정)
   └── i18n/ko.json

Phase 5: Frontend — 대시보드 시각화            [~30분]
   └── index.tsx: ModelUsageCard
```

**총 예상: ~6시간**

---

## 8. v1에서 v2로의 변경 요약

| v1 (초안) | v2 (수정) | 이유 |
|-----------|-----------|------|
| `RoutingSettings` struct 신규 생성 | 기존 `EngineConfig` 필드 재사용 | 중복 방지 |
| Web route → `OxiosEngine` 직접 | Web route → `EngineApi` (퍼사드) | 아키텍처 준수 |
| 별도 라우팅 탭 | Engine 탭 내 라우팅 섹션 | UI 일관성 |
| `GET /api/engine/routing/config` | `GET /api/engine/config`에 routing 포함 | API 패턴 일관 |
| `RoutingStats` in `OxiosEngine` | `Arc<RoutingStats>`를 `EngineApi`와 `AgentRuntime` 공유 | Web 접근 가능 |
| `ModelSelect` 재사용 | `ModelSelect` 사용 + 순서/삭제 UI | 검증 후 결정 |
| 번역 키 목록 없음 | 9개 키 명시 | 구현 가이드 |
| 비용 출처 불명확 | `model_db::get_model_entry()` 사용 | 명확화 |

---

## 9. 테스트 계획

| 단계 | 테스트 | 방법 |
|------|--------|------|
| Phase 1 | 전체 빌드 + 테스트 | `cargo test --workspace` |
| Phase 2 | 라우팅 설정 읽기/쓰기 | 유닛 테스트 |
| Phase 3 | API 엔드포인트 | `curl` 또는 httpie |
| Phase 4 | 설정 UI 렌더링 | `bun dev` → `/settings` |
| Phase 5 | 대시보드 카드 | 에이전트 실행 후 카드 확인 |
| E2E | 라우팅 on → 채팅 → 통계 확인 | 전체 흐름 |
