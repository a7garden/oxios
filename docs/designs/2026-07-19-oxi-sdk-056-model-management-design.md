# Oxios: oxi-sdk 0.56.0 통합 & LobeHub급 모델 관리 설계

> 2026-07-19. oxi-sdk 0.54.0 → 0.56.0 업그레이드 완료 후, 신규 기능 통합 설계.
> LobeHub v2.2.9의 provider/model 관리 UX를 참조하여 Oxios Web UI를 재설계.

## 목차

1. [데이터 모델 & 설정 스키마](#1-데이터-모델--설정-스키마)
2. [프론트엔드 컴포넌트 계층](#2-프론트엔드-컴포넌트-계층)
- **byModel 모드**: 동일 displayName을 가진 모델을 provider 간에 dedup. 예: `claude-sonnet-4-20250514`가 Anthropic과 AWS Bedrock 둘 다 있으면 하나의 행으로 표시하고, 클릭 시 provider 선택 서브메뉴 노출.
3. [백엔드 변경사항](#3-백엔드-변경사항)
4. [SdkUrlResolver 통합](#4-sdkurlresolver-통합)
5. [LSP Tool 통합](#5-lsp-tool-통합)
6. [AgentDecorator 마이그레이션](#6-agentdecorator-마이그레이션)
7. [구현 페이즈](#7-구현-페이즈)

---

## 1. 데이터 모델 & 설정 스키마

### 1.1 config.toml 확장

```toml
# ── 신규: Provider별 설정 ──
[engine.providers.anthropic]
enabled = true                      # provider 활성/비활성
sort_order = 0                      # 정렬 순서 (낮을수록 먼저)
custom_endpoint = ""               # 커스텀 base URL 오버라이드

[engine.providers.anthropic.models]
mode = "all"                        # "allowlist" | "denylist" | "all"
allow = []                          # allowlist일 때만 사용
deny = []                           # denylist일 때만 사용

# ── 신규: 커스텀 프로바이더 ──
[[engine.custom_providers]]
id = "my-openai-proxy"
name = "My OpenAI Proxy"
sdk_type = "openai"                 # openai | anthropic | google | openai-compatible
base_url = "https://llm.internal.example.com/v1"
api_key_env = "MY_PROXY_KEY"        # env var 이름 (선택)

# ── 기존 routing 섹션 유지 (호환) ──
[engine.routing]
enabled = false
prefer_cost_efficient = false
fallback_models = []
excluded_models = []
```

### 1.2 Rust 타입

```rust
// crates/oxios-kernel/src/config.rs — 추가

/// Provider별 설정
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub custom_endpoint: Option<String>,
    #[serde(default)]
    pub models: ModelListSettings,
}

/// Provider별 모델 리스트 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelListSettings {
    #[serde(default)]
    pub mode: ModelListMode,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

impl Default for ModelListSettings {
    fn default() -> Self {
        Self { mode: ModelListMode::All, allow: vec![], deny: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModelListMode {
    #[default]
    All,
    Allowlist,
    Denylist,
}

/// 커스텀 프로바이더
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderDef {
    pub id: String,
    pub name: String,
    pub sdk_type: SdkType,
    pub base_url: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SdkType {
    OpenAI,
    Anthropic,
    Google,
    #[serde(rename = "openai-compatible")]
    OpenAICompatible,
}

// OxiosConfig 최상위에 추가
pub struct EngineConfig {
    // ... 기존 필드 ...
    #[serde(default)]
    pub providers: HashMap<String, ProviderSettings>,
    #[serde(default)]
    pub custom_providers: Vec<CustomProviderDef>,
}
```

### 1.3 프론트엔드 타입

```typescript
// web/src/types/engine.ts — 확장

export interface ProviderSettings {
  enabled: boolean;
  sortOrder: number;
  customEndpoint?: string;
  modelListConfig: ModelListConfig;
  isCustom: boolean;
  sdkType?: 'openai' | 'anthropic' | 'google' | 'openai-compatible';
}

export interface ModelListConfig {
  mode: 'all' | 'allowlist' | 'denylist';
  allow: string[];
  deny: string[];
}

// LobeHub의 EnabledProviderWithModels를 포트
export interface EnabledProviderWithModels {
  provider: ProviderInfo;
  models: ModelInfo[];
}

// 신규 API 응답 타입
export interface ProviderConfigResponse {
  provider: ProviderInfo;
  settings: ProviderSettings;
  models: ModelInfo[];
}

export interface ConnectionCheckResult {
  success: boolean;
  model: string;
  latencyMs: number;
  error?: string;
}

export interface CustomProviderInput {
  id: string;
  name: string;
  sdkType: 'openai' | 'anthropic' | 'google' | 'openai-compatible';
  baseUrl: string;
  apiKeyEnv?: string;
}
```

### 1.4 신규 API 엔드포인트

| Method | Path | 설명 | Request Body | Response |
|---|---|---|---|---|
| `GET` | `/api/engine/providers/:id/config` | provider 설정 + 모델 목록 | — | `ProviderConfigResponse` |
| `PUT` | `/api/engine/providers/:id/config` | provider 설정 저장 | `ProviderSettings` | `ProviderConfigResponse` |
| `POST` | `/api/engine/providers/:id/check` | connection test | `{ model: string }` | `ConnectionCheckResult` |
| `PUT` | `/api/engine/providers/:id/models` | 모델 리스트 설정 | `ModelListConfig` | `ProviderConfigResponse` |
| `POST` | `/api/engine/custom-providers` | 커스텀 provider 추가 | `CustomProviderInput` | `ProviderInfo` |
| `DELETE` | `/api/engine/custom-providers/:id` | 커스텀 provider 삭제 | — | `{ deleted: true }` |

기존 14개 엔드포인트 + 신규 6개 = 총 20개.

---

## 2. 프론트엔드 컴포넌트 계층

### 2.1 전체 구조

```
web/src/
├── types/
│   └── engine.ts                    # 타입 확장 (ProviderSettings, EnabledProviderWithModels, etc.)
├── hooks/
│   ├── use-engine.ts               # 기존 + useProviderConfig, useCheckConnection, useCustomProviders
│   └── use-model-switch.ts          # 신규: ModelSwitchPanel 상태 관리
├── features/
│   └── model-switch/                # NEW: 채팅 입력바 모델 피커 (LobeHub ModelSwitchPanel 포트)
│       ├── index.tsx                #   ModelSwitchPanel root (DropdownMenu)
│       ├── types.ts                 #   ListItem discriminated union
│       ├── const.ts                 #   너비, 높이, 아이템 높이 상수
│       ├── Toolbar.tsx              #   검색 + 그룹 모드 토글 (byModel/byProvider)
│       ├── List.tsx                 #   가상화 리스트 렌더러
│       ├── ListItemRenderer.tsx     #   아이템 타입별 디스패치
│       ├── ModelDetailPanel.tsx     #   우측 아코디언 서브메뉴 (abilities, pricing, context, ratings)
│       ├── ModelRatingRadar.tsx     #   SVG 레이더 차트 (LobeHub 직접 포트)
│       ├── ModelInfoTags.tsx        #   abilities 아이콘 + context token 배지
│       ├── ProviderItemRender.tsx   #   provider 헤더 행
│       └── hooks/
│           ├── useBuildListItems.ts     # byModel/byProvider 로직
│           ├── useModelAndProvider.ts   # 선택된 모델 상태
│           └── useModelDetailPanel.ts   # detail 데이터 계산
├── routes/
│   └── settings/
│       ├── index.tsx                # settings root (라우터)
│       ├── provider/                # NEW: Provider 관리 (LobeHub 구조 포트)
│       │   ├── index.tsx            #   ProviderLayout (sidebar + content)
│       │   ├── ProviderMenu.tsx     #   sidebar: enabled/disabled accordion
│       │   ├── ProviderGrid.tsx     #   3-section 그리드 (enabled / custom / disabled)
│       │   ├── ProviderCard.tsx     #   카드: 아이콘, 이름, EnableSwitch, 모델 수
│       │   ├── ProviderConfig.tsx   #   detail: API key 폼, endpoint, checker
│       │   ├── ModelList.tsx        #   provider별 모델 CRUD 그리드
│       │   ├── ModelItem.tsx        #   모델 행: 체크박스, abilities, 설정
│       │   ├── Checker.tsx          #   "Test Connection" 버튼 + 결과
│       │   └── CreateNewProvider.tsx #  모달: 커스텀 provider 추가
│       └── engine/                  # EXISTING: 기존 엔진 설정 (확장)
│           ├── ProviderOptionsPanel.tsx
│           ├── RoutingSection.tsx    # Oxi::routing() 연동
│           └── RoleSection.tsx
└── components/
    └── chat/
        └── model-picker.tsx         # → features/model-switch/index.tsx 로 교체
```

### 2.2 주요 컴포넌트 설계

#### ModelSwitchPanel (features/model-switch/index.tsx)

```
Props:
  model?: string          # 현재 선택된 모델 ID
  provider?: string       # 현재 선택된 provider
  onModelChange: (modelId: string, providerId: string) => void

내부 상태:
  open: boolean
  searchKeyword: string
  groupMode: 'byModel' | 'byProvider'
  detailModel: ModelInfo | null   # 우측 패널에 표시할 모델

동작:
  1. DropdownMenu 트리거 → PanelContent 열기
  2. Toolbar: 검색어 입력 → 리스트 필터링
  3. groupMode 토글: byProvider (provider → models) / byModel (model → providers)
  4. 모델 행 호버 → 우측 ModelDetailPanel 표시
  5. 모델 클릭 → onModelChange 호출 + 드롭다운 닫기
```

#### ModelDetailPanel (features/model-switch/ModelDetailPanel.tsx)

```
Props:
  model: ModelInfo

섹션 (Accordion):
  1. Rating      — ModelRatingRadar SVG (intelligence, agentic, speed, price, writing, design)
  2. Context     — "200K tokens" (+ maxOutput 표시)
  3. Abilities   — 태그 리스트 (functionCall, vision, files, reasoning, search, imageOutput)
  4. Pricing     — input/output/cached, unit type별 그룹 (text/image/audio)
```

#### ProviderGrid (routes/settings/provider/ProviderGrid.tsx)

```
3개 섹션:
  1. Enabled Providers   — enabled + 키 있음 → ProviderCard + EnableSwitch (on)
  2. Custom Providers    — 사용자 추가 provider → ProviderCard + 삭제 버튼
  3. Disabled Providers  — disabled 또는 키 없음 → ProviderCard + EnableSwitch (off)

ProviderCard:
  - provider 아이콘 (ModelIcon)
  - provider 이름 + 카테고리
  - 키 상태 표시 (configured / missing)
  - 모델 수 배지
  - 클릭 → ProviderConfig detail 페이지
```

#### ProviderConfig (routes/settings/provider/ProviderConfig.tsx)

```
섹션:
  1. Header        — provider 로고, 이름, EnableSwitch
  2. API Key       — FormPassword + source 표시 (env / auth_store / config / none)
  3. Endpoint URL  — custom_endpoint 입력 (기본 URL 표시)
  4. Model List    — ModelList 컴포넌트 (allowlist/denylist 편집)
  5. Checker       — "Test Connection" 버튼 → checkProviderConnectivity
```

#### ModelList (routes/settings/provider/ModelList.tsx)

```
Props:
  providerId: string
  models: ModelInfo[]
  config: ModelListConfig
  onChange: (config: ModelListConfig) => void

기능:
  - 모드 선택: All / Allowlist / Denylist (라디오 그룹)
  - Allowlist 모드: 검색 → 모델 선택 → allow[]에 추가
  - Denylist 모드: 모델 목록에서 체크박스로 제외 선택
  - 모델 행: 이름, abilities 아이콘, context window, pricing
  - "Fetch Live Models" 버튼 → provider API에서 실시간 모델 목록 가져오기
```

### 2.3 LobeHub → Oxios 의존성 번역

| LobeHub | Oxios | 변환 방식 |
|---|---|---|
| `Flexbox` | `<div className="flex flex-col gap-{n}">` | 기계적 |
| `Accordion` | shadcn/ui `<Accordion>` | 이미 있음 |
| `SearchBar` | `<Input>` + `Search` 아이콘 | 커스텀 |
| `DropdownMenu` | shadcn/ui `<DropdownMenu>` | 이미 있음 |
| `Tabs` | shadcn/ui `<Tabs>` | 이미 있음 |
| `createStaticStyles` | Tailwind `cn()` | 기계적 |
| `cx` from antd-style | `cn()` from clsx + tailwind-merge | 이미 있음 |
| `ModelIcon` | Oxios `ModelIcon` | 이미 있음 |
| `InstantSwitch` | shadcn/ui `<Switch>` | 이미 있음 |
| `SWR` / `useSWR` | React Query `useQuery` | 이미 있음 |
| `tRPC lambdaClient` | Oxios `apiClient` | 이미 있음 |
| `FormPassword` | `<Input type="password">` | shadcn/ui |
| `Rnd` (resizable) | 고정 380px | 제거 (간소화) |
| `GlobalStore` (systemStatus) | localStorage + React state | 간소화 |
| `aiInfraStore` (Zustand) | React Query 캐시 | 기존 패턴 유지 |
| `agentStore` (Zustand) | Oxios chatStore (Zustand) | 기존 패턴 유지 |
| `updateAgentConfig()` | `useSetModel()` | 기존 API 유지 |

---

## 3. 백엔드 변경사항

### 3.1 OxiosEngine — RoutingControl 통합

**현재 상태:**
- `OxiosEngine`이 자체 `routing_control: Option<oxi_sdk::RoutingControl>` 필드를 가지고 있으나 `None`으로 초기화되고 사용되지 않음.
- `OxiosEngine`이 별도 `authorizer`, `tracer`, `cost_tracker` 필드를 가지고 있음.

**변경:**
- `routing_control` 필드 제거. `Oxi::routing()`이 이미 `Arc<RoutingControl>`을 제공하므로 중복 제거.
- `OxiosEngine` 빌드 시 `OxiBuilder`를 통해 provider 설정을 `RoutingControl`에 반영.

```rust
// engine.rs — 변경 후
impl OxiosEngine {
    pub fn from_config_with_catalog_opt(
        default_model_id: impl Into<String>,
        config_api_key: Option<&str>,
        catalog: Option<Arc<dyn ModelCatalog>>,
    ) -> Self {
        let mut builder = OxiBuilder::new().with_builtins();

        // 1. Credential injection (기존 로직 유지)
        for provider in &providers_to_try {
            if let Some((key, source)) = CredentialStore::resolve(provider, config_key) {
                builder = builder.api_key(provider, key);
            }
        }

        // 2. Catalog wiring (기존)
        // 3. Custom provider registration (신규)
        for cp in &config.custom_providers {
            builder = builder.provider_factory(cp.id.clone(), move || {
                create_custom_provider(cp)
            });
        }

        let oxi = builder.build();

        // 4. 초기 RoutingControl 설정 (신규)
        for (provider_id, ps) in &config.providers {
            if !ps.enabled {
                // 비활성 provider의 모든 모델을 exclude
                // (또는 set_enabled(false)로 provider 전체 off)
            }
            for denied_model in &ps.models.deny {
                oxi.routing().exclude_model(&format!("{provider_id}/{denied_model}"));
            }
        }

        Self { oxi, default_model_id: model_id, pools: ..., authorizer: None, tracer: None, cost_tracker: None }
    }

    /// RoutingControl 직접 접근 (신규)
    pub fn routing(&self) -> &Arc<oxi_sdk::RoutingControl> {
        self.oxi.routing()
    }
}
```

### 3.2 EngineApi 확장

```rust
// kernel_handle/engine_api.rs — 신규 메서드

impl EngineApi {
    /// Provider 설정 조회
    pub async fn get_provider_config(&self, provider_id: &str) -> Result<ProviderConfigResponse>;

    /// Provider 설정 저장 → config.toml 업데이트 + RoutingControl 반영
    pub async fn set_provider_config(&self, provider_id: &str, settings: ProviderSettings) -> Result<ProviderConfigResponse>;

    /// Connection check — Oxi::create_provider()로 실제 provider 생성 시도
    pub async fn check_provider_connection(&self, provider_id: &str, model: &str) -> Result<ConnectionCheckResult>;

    /// 모델 리스트 설정 저장
    pub async fn set_model_list(&self, provider_id: &str, config: ModelListConfig) -> Result<ProviderConfigResponse>;

    /// 커스텀 provider 추가
    pub async fn add_custom_provider(&self, input: CustomProviderDef) -> Result<ProviderInfo>;

    /// 커스텀 provider 삭제
    pub async fn remove_custom_provider(&self, id: &str) -> Result<()>;
}
```

### 3.3 Connection Checker 구현

```rust
// oxi-sdk 0.56.0: create_provider가 AuthProvider 포트를 매 호출 조회
// → credential 변경이 엔진 재빌드 없이 즉시 반영됨
async fn check_provider_connection(&self, provider_id: &str, model: &str) -> Result<ConnectionCheckResult> {
    let start = Instant::now();
    match self.engine.read().oxi().create_provider(provider_id) {
        Ok(provider) => {
            // 간단한 모델 리스트 조회로 연결 확인
            // (실제 LLM 호출 없이 provider 연결만 검증)
            let latency = start.elapsed().as_millis() as u64;
            Ok(ConnectionCheckResult {
                success: true,
                model: model.to_string(),
                latency_ms: latency,
            })
        }
        Err(e) => {
            Ok(ConnectionCheckResult {
                success: false,
                model: model.to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
                error: Some(e.to_string()),
            })
        }
    }
}
```

### 3.4 API 라우트 등록

```rust
// src/api/routes/engine_routes.rs

// 신규 라우트
.route("/api/engine/providers/:id/config", get(handle_get_provider_config).put(handle_set_provider_config))
.route("/api/engine/providers/:id/check", post(handle_check_provider_connection))
.route("/api/engine/providers/:id/models", put(handle_set_model_list))
.route("/api/engine/custom-providers", post(handle_add_custom_provider))
.route("/api/engine/custom-providers/:id", delete(handle_remove_custom_provider))
```

---

## 4. SdkUrlResolver 통합

### 4.1 목표

Agent tool (`read`, `grep`, `glob`)에서 `issue://`, `pr://`, `memory://`, `skill://` 등 Oxios 내부 URL을 네이티브로 해석 가능하게 함.

### 4.2 구현

```rust
// crates/oxios-kernel/src/url_resolver.rs — 신규 파일

use oxi_sdk::SdkUrlResolver;
use oxi_sdk::ports::{InternalUrlRouter, ResolveContext, ResolvedUrl};
use std::sync::Arc;

/// Oxios의 InternalUrlRouter 구현.
/// oxi-sdk의 InternalUrlRouter 포트를 구현하여
/// oxios만의 프로토콜 (memory://, knowledge://)을 지원.
pub struct OxiosUrlRouter {
    knowledge_base: Arc<KnowledgeBase>,   // ~/.oxios/knowledge/
    memory_manager: Arc<MemoryManager>,    // agent memory
    // issue tracker, PR cache 등 추후 추가
}

impl InternalUrlRouter for OxiosUrlRouter {
    fn registered_schemes(&self) -> Vec<String> {
        vec![
            "issue".into(), "pr".into(),
            "memory".into(), "knowledge".into(),
            "skill".into(), "agent".into(),
            "history".into(), "artifact".into(),
        ]
    }

    async fn resolve(&self, uri: &str, ctx: &ResolveContext) -> Result<ResolvedUrl, SdkError> {
        // scheme별 디스패치
    }
}
```

### 4.3 AgentConfig 주입

```rust
// agent_runtime.rs — build_agent_config()
let url_resolver = Arc::new(SdkUrlResolver::new(
    Arc::new(OxiosUrlRouter::new(kb.clone(), memory.clone()))
));

let agent_config = AgentConfig {
    // ... 기존 필드 ...
    url_resolver: Some(url_resolver),
    lsp: None,  // Phase B에서 활성화
    ..Default::default()로 채워지는 나머지
};
```

---

## 5. LSP Tool 통합

### 5.1 목표

Agent가 프로젝트 workspace의 LSP 서버를 통해 심볼 rename, 정의 찾기, 참조 찾기 등을 수행.

### 5.2 설계

Oxios의 경우 워크스페이스별로 LSP 서버를 관리해야 하므로, oxi-sdk의 `LspProvider` trait을 구현하는 `OxiosLspProvider` 작성:

```rust
// crates/oxios-kernel/src/lsp.rs — 신규 파일

pub struct OxiosLspProvider {
    // workspace_path → LspManager 매핑
    managers: Arc<RwLock<HashMap<PathBuf, Arc<LspManager>>>>,
}

impl LspProvider for OxiosLspProvider {
    async fn get_lsp(&self, workspace: &Path) -> Option<Arc<dyn LspClient>> {
        // 캐시된 LspManager 조회 또는 새로 spawn
    }
}
```

### 5.3 도입 시기

Phase B (Provider/Model 관리 안정화 후). AgentConfig.lsp 필드는 `None`으로 시작.

---

## 6. AgentDecorator 마이그레이션

### 6.1 현재 상태

`OxiosEngine`이 개별 필드로 관리:
```rust
pub struct OxiosEngine {
    authorizer: Option<Arc<Authorizer>>,
    tracer: Option<Arc<Tracer>>,
    cost_tracker: Option<Arc<CostTracker>>,
}
```

### 6.2 변경

oxi-sdk 0.56.0의 `AgentDecorator` trait 사용. `SupervisorBuilder`가 `agent_decorator`를 통해 통합 전달:

```rust
// engine.rs
use oxi_sdk::observability::{AgentDecorator, ObservabilityDecorator};

impl OxiosEngine {
    pub fn build_observability_decorator(&self) -> Option<Arc<dyn AgentDecorator>> {
        let tracer = self.tracer.clone()?;
        Some(Arc::new(ObservabilityDecorator::new(
            tracer,
            self.audit_log.clone(),
            self.cost_tracker.clone(),
            self.authorizer.clone(),
        )))
    }
}
```

### 6.3 영향

- `agent_runtime.rs`에서 개별 `AgentBuilder::tracer()` / `::audit_log()` / `::cost_tracker()` 호출을 `::decorator(decorator)` 하나로 대체.
- `SupervisorBuilder::build()` 호출부에서 `agent_decorator` 주입.

### 6.4 도입 시기

Phase A (Provider/Model 관리) 완료 후 리팩토링. 기능 변경 없이 코드 정리만.

---

## 7. 구현 페이즈

### Phase A: Provider/Model 관리 (핵심)

| 순서 | 작업 | 파일 | 예상 시간 |
|---|---|---|---|
| A1 | `config.toml` 스키마 확장 | `crates/oxios-kernel/src/config.rs` | 2h |
| A2 | `EngineApi` 신규 메서드 6개 | `crates/oxios-kernel/src/kernel_handle/engine_api.rs` | 4h |
| A3 | `OxiosEngine` RoutingControl 통합 | `crates/oxios-kernel/src/engine.rs` | 2h |
| A4 | API 라우트 등록 (Axum) | `src/api/routes/engine_routes.rs` | 2h |
| A5 | 프론트엔드 타입 확장 | `web/src/types/engine.ts` | 1h |
| A6 | React Query hooks 확장 | `web/src/hooks/use-engine.ts` | 3h |
| A7 | `features/model-switch/` 전체 | 7개 파일 + hooks | 8h |
| A8 | `routes/settings/provider/` 전체 | 9개 파일 | 10h |
| A9 | 기존 `model-picker.tsx` → `model-switch` 교체 | `web/src/components/chat/model-picker.tsx` | 2h |
| A10 | 통합 테스트 + smoke test | — | 3h |

**Phase A 총 예상: 37h**

### Phase B: Agent Capability

| 순서 | 작업 | 파일 |
|---|---|---|
| B1 | `OxiosUrlRouter` 구현 | `crates/oxios-kernel/src/url_resolver.rs` |
| B2 | `SdkUrlResolver` AgentConfig 주입 | `crates/oxios-kernel/src/agent_runtime.rs` |
| B3 | `OxiosLspProvider` 구현 | `crates/oxios-kernel/src/lsp.rs` |
| B4 | LSP AgentConfig 주입 | `crates/oxios-kernel/src/agent_runtime.rs` |

**Phase B 총 예상: 10h**

### Phase C: Observability 정리

| 순서 | 작업 | 파일 |
|---|---|---|
| C1 | `OxiosEngine`에 `ObservabilityDecorator` 빌더 추가 | `crates/oxios-kernel/src/engine.rs` |
| C2 | `agent_runtime.rs`에서 개별 hook → decorator로 대체 | `crates/oxios-kernel/src/agent_runtime.rs` |

**Phase C 총 예상: 4h**

---

## 부록: LobeHub 참조 파일 맵

| Oxios 파일 | LobeHub 원본 | 포트 방식 |
|---|---|---|
| `features/model-switch/index.tsx` | `src/features/ModelSwitchPanel/index.tsx` | 구조 복사, Tailwind 변환 |
| `features/model-switch/Toolbar.tsx` | `ModelSwitchPanel/components/Toolbar.tsx` | 구조 복사 |
| `features/model-switch/List.tsx` | `ModelSwitchPanel/components/List/index.tsx` | 구조 복사 |
| `features/model-switch/ListItemRenderer.tsx` | `List/ListItemRenderer.tsx` | 간소화 (4 variants) |
| `features/model-switch/ModelDetailPanel.tsx` | `ModelSwitchPanel/components/ModelDetailPanel.tsx` | 직접 복사 |
| `features/model-switch/ModelRatingRadar.tsx` | `ModelSwitchPanel/components/ModelRatingRadar.tsx` | 직접 복사 (순수 SVG) |
| `features/model-switch/ModelInfoTags.tsx` | `src/components/ModelSelect/index.tsx` | 직접 복사 |
| `features/model-switch/hooks/useBuildListItems.ts` | `ModelSwitchPanel/hooks/useBuildListItems.ts` | 구조 복사 |
| `routes/settings/provider/ProviderGrid.tsx` | `routes/(main)/settings/provider/(list)/ProviderGrid/` | 구조 복사 |
| `routes/settings/provider/ProviderConfig.tsx` | `features/ProviderConfig/index.tsx` | 구조 복사 |
| `routes/settings/provider/ModelList.tsx` | `features/ModelList/index.tsx` | 구조 복사 |
| `routes/settings/provider/Checker.tsx` | `features/ProviderConfig/Checker.tsx` | 직접 복사 |
