# RFC-011: oxi-sdk 0.22.0 마이그레이션 및 통합 모델/프로바이더 관리

> **Status**: Draft — Self-Review Completed (see Appendix A)
> **Date**: 2026-05-25
> **Author**: Oxios Team
> **Replaces**: —

---

## 1. 배경

oxios는 현재 `oxi-sdk = "0.19.0"` / `oxi-ai = "0.19.0"`에 고정되어 있다. oxi-sdk 0.22.0이 릴리스되면서 다음과 같은 변화가 생겼다:

### 1.1 SDK 변경 사항 (0.19 → 0.22)

| 영역 | 변경 내용 | 영향도 |
|------|-----------|--------|
| **`ProviderOptions`** (신규) | `StreamOptions`에 `provider_options: Option<ProviderOptions>` 필드 추가. Anthropic (adaptive thinking), OpenAI (reasoning_effort, text_verbosity), Google (thinking_level) 등 프로바이더별 세부 설정 가능 | **Medium** — OxiosEngine이 아직 ProviderOptions를 사용하지 않음 |
| **`AgentLoopConfig`** | `api_key: Option<String>` 및 `provider_options: Option<ProviderOptions>` 필드 추가 | **Low** — AgentRuntime에서 AgentLoopConfig에 api_key만 전달 중, provider_options 미사용 |
| **`normalize_messages`** (신규) | `oxi_ai::normalize_messages()` 공개. 메시지 정규화(빈 콘텐츠 필터링, tool ID 스크럽) | **Low** — 내부적으로 이미 사용 중, 공개 API로 승격만 |
| **`LagAwareReceiver`, `PublishResult`** | 메시지 버스 확장 | **Low** — A2A에서만 사용 |
| **AnthropicProvider** | `with_config()`, `with_base_url()`, `with_api_key()` 추가. `anthropic-beta` 헤더에 interleaved-thinking beta 포함 | **Medium** — 기존 `OpenAiProvider::with_base_url_and_key()` 패턴과 정렬 가능 |
| **model_db** | MiniMax 모델(M2.7) 추가. 전체 544개 모델, 28개 프로바이더 | **Low** — 자동 반영 |
| **ModelEntry** | 동일 (변경 없음) | **None** |
| **Model (struct)** | 동일 (변경 없음) | **None** |
| **Provider trait** | 동일 (변경 없음) | **None** |
| **BuiltinProvider 메타데이터** | 47개 빌트인 프로바이더, `category`, `description`, `auth_method` 등 풍부한 메타데이터 | **High** — Web UI에서 활용 가능 |

### 1.2 현재 Oxios의 아키텍처 문제

```
현재 구조 (이중 관리):

config.toml [engine]                → OxiosEngine (engine.rs)
  default_model = "anthropic/..."     → OxiBuilder::new().with_builtins()
  api_key = "sk-..."                  → CredentialStore (credential.rs)
                                      → register_compatible_providers() ← zai만 하드코딩
                                      
Web UI Settings                      → PUT /api/config → config.toml 덮어쓰기
  Engine 탭: default_model 텍스트 박스    (raw text, validation 없음)
            api_key 패스워드 박스

Onboarding (onboarding.rs)           → oxi_sdk::get_providers()
  CLI 마법사                           → oxi_sdk::get_provider_models()
                                      → CredentialStore::store()
```

**문제점:**

1. **Web UI에서 모델/프로바이더 변경이 raw text** — 드롭다운 없음, 검증 없음, 실시간 피드백 없음
2. **ProviderOptions 미노출** — reasoning effort, thinking level 등을 설정할 방법이 없음
3. **ZAI만 하드코딩** — `register_compatible_providers()`에 zai가 하드코딩되어 있음
4. **CredentialStore와 EngineProvider가 분리** — CredentialStore는 정적 메서드, Engine은 Oxi 인스턴스 기반
5. **config.toml이 유일한 설정 경로** — Web UI에서 변경해도 런타임에 Oxi 인스턴스 재생성 안 됨
6. **47개 빌트인 프로바이더 메타데이터 활용 안 함** — `BuiltinProvider`의 `category`, `description`, `auth_method` 등이 Web UI에 전달되지 않음

---

## 2. 설계 원칙

| 원칙 | 의미 |
|------|------|
| **oxi-sdk가 Single Source of Truth** | 모델 목록, 프로바이더 메타데이터, credential resolution 모두 oxi-sdk에 위임. Oxios에서 재구현하지 않음 |
| **설정 = config.toml + 런타임 반영** | Web UI에서 변경 시 config.toml에 저장하고 런타임 Oxi 인스턴스를 핫스왑 |
| **Web UI = oxi-sdk 메타데이터의 뷰** | 모델/프로바이더 선택 UI는 oxi-sdk의 `model_db` + `BuiltinProvider`에서 동적으로 생성 |
| **이중 구현 금지** | Oxios가 자체 모델/프로바이더 목록을 유지하지 않음. 모두 oxi-sdk에서 가져옴 |

---

## 3. 아키텍처

### 3.1 새로운 데이터 흐름

```
┌──────────────────────────────────────────────────────────────┐
│                     oxi-sdk 0.22.0                          │
│                                                              │
│  BuiltinProvider (47개)    ModelEntry (544개)               │
│    name, display_name        id, name, provider              │
│    category, description     reasoning, cost                 │
│    env_key, base_url         context_window, max_tokens      │
│    auth_method, aliases      input modalities                │
│                                                              │
│  get_providers()            get_provider_models(p)          │
│  get_builtin_provider(n)    get_model_entry(p, id)          │
│  create_builtin_provider()  search_models(pattern)          │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│                     Oxios Backend                            │
│                                                              │
│  EngineProvider (engine.rs)                                  │
│    ┌─ OxiosEngine                                           │
│    │   oxi: Oxi (OxiBuilder에서 생성)                      │
│    │   default_model_id: String                             │
│    │   provider_options: Option<ProviderOptions>  ← 신규   │
│    └─                                                       │
│                                                              │
│  KernelHandle.engine → EngineApi (신규 퍼사드)              │
│    providers()      → Vec<ProviderInfo>  (from oxi-sdk)     │
│    models(provider) → Vec<ModelInfo>     (from oxi-sdk)     │
│    config()         → EngineConfig       (from config.toml) │
│    set_model()      → 핫스왑            (config + runtime)  │
│    set_api_key()    → 저장              (auth store)        │
│    set_provider_options() → 핫스왑      (config + runtime)  │
│    validate_key()   → 테스트 호출       (optional)          │
│                                                              │
│  config.toml [engine]                                        │
│    default_model = "anthropic/claude-sonnet-4-20250514"      │
│    api_key = "..."  (또는 auth store)                        │
│    [engine.provider_options]                      ← 신규     │
│    [engine.provider_options.anthropic]                        │
│    thinking_type = "adaptive"                                │
│    [engine.provider_options.openai]                          │
│    reasoning_effort = "high"                                 │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│                     Web UI                                   │
│                                                              │
│  /settings (기존) → /settings/engine (신규 탭)              │
│    ┌─────────────────────────────────────────────┐          │
│    │  Provider 선택 (드롭다운)                    │          │
│    │    ← GET /api/engine/providers               │          │
│    │    카테고리별 그룹핑, 키 감지 상태 표시      │          │
│    │                                              │          │
│    │  Model 선택 (드롭다운)                       │          │
│    │    ← GET /api/engine/models?provider=anthropic│          │
│    │    reasoning ✦, vision 👁 아이콘              │          │
│    │    context window, 가격 정보 표시             │          │
│    │                                              │          │
│    │  API Key 입력 (마스킹)                       │          │
│    │    ← 감지 상태: env / auth store / none       │          │
│    │                                              │          │
│    │  Provider Options (접이식)                    │          │
│    │    ← 선택된 프로바이더에 맞게 동적 렌더링     │          │
│    │    Anthropic: thinking_type, effort           │          │
│    │    OpenAI: reasoning_effort, text_verbosity   │          │
│    │    Google: thinking_level, thinking_budget    │          │
│    └─────────────────────────────────────────────┘          │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 핵심 API 경로

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/engine/providers` | oxi-sdk 빌트인 프로바이더 목록 + 메타데이터 |
| `GET` | `/api/engine/models?provider={name}` | 해당 프로바이더의 모델 목록 |
| `GET` | `/api/engine/models/search?q={pattern}` | 모델 검색 |
| `GET` | `/api/engine/config` | 현재 엔진 설정 (model, key 상태, provider_options) |
| `PUT` | `/api/engine/model` | 기본 모델 변경 (핫스왑) |
| `PUT` | `/api/engine/api-key` | API 키 저장 |
| `PUT` | `/api/engine/provider-options` | ProviderOptions 업데이트 (핫스왑) |
| `POST`| `/api/engine/validate-key` | 현재 키 유효성 검증 (테스트 호출) |

### 3.3 핵심 데이터 타입

#### Backend (Rust)

```rust
// ── engine_api.rs (신규: KernelHandle 퍼사드)

/// 프로바이더 정보 — oxi-sdk BuiltinProvider에서 변환
#[derive(Serialize, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub display_name: String,
    pub category: String,        // "primary", "chinese", "enterprise", ...
    pub description: String,
    pub api_type: String,        // "anthropic-messages", "openai-completions", ...
    pub env_key: String,         // "ANTHROPIC_API_KEY"
    pub has_key: bool,           // 현재 credential 감지 여부
    pub model_count: usize,
}

/// 모델 정보 — oxi-sdk ModelEntry에서 변환
#[derive(Serialize, Clone)]
pub struct ModelInfo {
    pub id: String,              // "claude-sonnet-4-20250514"
    pub full_id: String,         // "anthropic/claude-sonnet-4-20250514"
    pub name: String,            // "Claude Sonnet 4"
    pub provider: String,
    pub reasoning: bool,
    pub vision: bool,
    pub context_window: u32,
    pub max_tokens: u32,
    pub cost_input: f64,         // $/M tokens
    pub cost_output: f64,
}

/// 엔진 설정 상태
#[derive(Serialize, Clone)]
pub struct EngineConfigResponse {
    pub default_model: String,
    pub provider: String,
    pub api_key_set: bool,       // 키가 설정되어 있는지 (값은 노출 안 함)
    pub key_source: String,      // "env", "auth_store", "config", "none"
    pub provider_options: Option<ProviderOptions>,
}
```

#### Frontend (TypeScript)

```typescript
// types/engine.ts (신규)

export interface ProviderInfo {
  name: string
  display_name: string
  category: string
  description: string
  api_type: string
  env_key: string
  has_key: boolean
  model_count: number
}

export interface ModelInfo {
  id: string
  full_id: string
  name: string
  provider: string
  reasoning: boolean
  vision: boolean
  context_window: number
  max_tokens: number
  cost_input: number
  cost_output: number
}

export interface EngineConfig {
  default_model: string
  provider: string
  api_key_set: boolean
  key_source: 'env' | 'auth_store' | 'config' | 'none'
  provider_options?: ProviderOptions
}

export interface ProviderOptions {
  anthropic?: {
    thinking_type?: string
    thinking_budget?: number
    effort?: string
  }
  openai?: {
    store?: boolean
    reasoning_effort?: string
    reasoning_summary?: string
    text_verbosity?: string
  }
  google?: {
    include_thoughts?: boolean
    thinking_level?: string
    thinking_budget?: number
  }
  openai_compatible?: {
    reasoning_effort?: string
    enable_thinking?: boolean
  }
}
```

---

## 4. 변경 사항 상세

### 4.1 `Cargo.toml` — 버전 업그레이드

```toml
# 루트 Cargo.toml [workspace.dependencies]
oxi-sdk = "0.22.0"
oxi-ai = "0.22.0"
```

### 4.2 `engine.rs` — OxiosEngine 리팩터링

**현재 문제:**
- `register_compatible_providers()`에 zai가 하드코딩
- `OxiosEngine`이 `Arc<Mutex<>>`로 감싸지지 않아 핫스왑 불가
- `ProviderOptions` 지원 없음

**변경:**

```rust
// Before:
pub struct OxiosEngine {
    oxi: Oxi,
    default_model_id: String,
}

// After:
pub struct OxiosEngine {
    inner: parking_lot::RwLock<OxiosEngineInner>,
}

struct OxiosEngineInner {
    oxi: Oxi,
    default_model_id: String,
    provider_options: Option<oxi_sdk::ProviderOptions>,
}
```

**핵심 메서드:**

```rust
impl OxiosEngine {
    /// 핫스왑: 기본 모델 변경
    pub fn set_default_model(&self, model_id: &str) -> Result<()> { ... }
    
    /// 핫스왑: ProviderOptions 변경
    pub fn set_provider_options(&self, opts: Option<ProviderOptions>) { ... }
    
    /// 프로바이더 목록 (oxi-sdk에서)
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        oxi_sdk::get_builtin_providers()
            .iter()
            .map(|p| ProviderInfo::from_builtin(p))
            .collect()
    }
    
    /// 모델 목록 (oxi-sdk에서)
    pub fn list_models(&self, provider: &str) -> Vec<ModelInfo> {
        oxi_sdk::get_provider_models(provider)
            .iter()
            .map(|m| ModelInfo::from_entry(m))
            .collect()
    }
}
```

**`register_compatible_providers` 제거:**
zai 등 OpenAI-compatible 프로바이더는 이미 oxi-sdk 0.22의 빌트인에 포함됨. 하드코딩된 factory 등록 불필요.

### 4.3 `config.rs` — EngineConfig 확장

```rust
// Before:
pub struct EngineConfig {
    pub default_model: String,
    pub api_key: Option<String>,
}

// After:
pub struct EngineConfig {
    pub default_model: String,
    pub api_key: Option<String>,
    /// Per-provider options (thinking, reasoning effort, etc.)
    #[serde(default)]
    pub provider_options: Option<oxi_sdk::ProviderOptions>,
}
```

**config.toml 예시:**

```toml
[engine]
default_model = "anthropic/claude-sonnet-4-20250514"

[engine.provider_options]
[engine.provider_options.anthropic]
thinking_type = "adaptive"
effort = "high"

[engine.provider_options.openai]
reasoning_effort = "high"
```

### 4.4 `kernel_handle/` — EngineApi 추가

새로운 `engine_api.rs` 파일:

```rust
//! EngineApi — KernelHandle의 엔진 제어 퍼사드
//!
//! 모델/프로바이더 관련 모든 작업을 oxi-sdk에 위임.

pub struct EngineApi {
    config: Arc<parking_lot::RwLock<OxiosConfig>>,
    config_path: PathBuf,
}

impl EngineApi {
    // 읽기 작업 — oxi-sdk의 정적 데이터에서 직접
    pub fn providers(&self) -> Vec<ProviderInfo> { ... }
    pub fn models(&self, provider: &str) -> Vec<ModelInfo> { ... }
    pub fn search_models(&self, query: &str) -> Vec<ModelInfo> { ... }
    
    // 현재 설정 — config + credential 상태
    pub fn config(&self) -> EngineConfigResponse { ... }
    
    // 쓰기 작업 — config 업데이트 + 핫스왑
    pub fn set_model(&self, model_id: &str) -> Result<()> { ... }
    pub fn set_api_key(&self, provider: &str, key: &str) -> Result<()> { ... }
    pub fn set_provider_options(&self, opts: ProviderOptions) -> Result<()> { ... }
    
    // 검증
    pub fn validate_key(&self, provider: &str) -> Result<bool> { ... }
}
```

### 4.5 `routes/engine_routes.rs` — 신규 웹 API

```rust
//! Engine management routes — model/provider configuration.
//!
//! All provider and model data comes from oxi-sdk's static model_db.
//! No local model/provider database in oxios.

/// GET /api/engine/providers
pub(crate) async fn handle_providers(
    state: State<Arc<AppState>>,
) -> Json<Vec<ProviderInfo>> { ... }

/// GET /api/engine/models?provider=anthropic&q=sonnet
pub(crate) async fn handle_models(
    state: State<Arc<AppState>>,
    Query(params): Query<ModelQueryParams>,
) -> Json<Vec<ModelInfo>> { ... }

/// GET /api/engine/config
pub(crate) async fn handle_engine_config(
    state: State<Arc<AppState>>,
) -> Json<EngineConfigResponse> { ... }

/// PUT /api/engine/model
pub(crate) async fn handle_set_model(
    state: State<Arc<AppState>>,
    Json(body): Json<SetModelRequest>,
) -> Result<Json<EngineConfigResponse>, AppError> { ... }

/// PUT /api/engine/api-key
pub(crate) async fn handle_set_api_key(
    state: State<Arc<AppState>>,
    Json(body): Json<SetApiKeyRequest>,
) -> Result<(), AppError> { ... }

/// PUT /api/engine/provider-options
pub(crate) async fn handle_set_provider_options(
    state: State<Arc<AppState>>,
    Json(body): Json<ProviderOptions>,
) -> Result<Json<EngineConfigResponse>, AppError> { ... }

/// POST /api/engine/validate-key
pub(crate) async fn handle_validate_key(
    state: State<Arc<AppState>>,
) -> Result<Json<ValidateKeyResponse>, AppError> { ... }
```

### 4.6 `agent_runtime.rs` — ProviderOptions 전달

```rust
// AgentLoopConfig에 provider_options 전달
let loop_config = AgentLoopConfig {
    model_id: config.model_id,
    system_prompt: Some(system_prompt),
    // ... (기존 필드)
    api_key: None, // credential은 CredentialStore에서 provider가 직접 해결
    provider_options: config.provider_options,  // ← 신규
};
```

### 4.7 Frontend — `settings.tsx` → `engine/` 컴포넌트

새로운 디렉토리 구조:

```
web/src/
  routes/settings.tsx          → Engine 탭 추가
  components/engine/
    provider-select.tsx         → 카테고리별 그룹 드롭다운
    model-select.tsx            → reasoning/vision 아이콘 + 정보
    api-key-input.tsx           → 상태 표시 (감지됨/미설정)
    provider-options.tsx        → 동적 프로바이더 옵션 폼
  hooks/use-engine.ts           → TanStack Query 훅
  types/engine.ts               → 타입 정의
```

**`use-engine.ts` 훅:**

```typescript
export function useProviders() {
  return useQuery({
    queryKey: ['engine', 'providers'],
    queryFn: () => api.get<ProviderInfo[]>('/api/engine/providers'),
  })
}

export function useModels(provider: string) {
  return useQuery({
    queryKey: ['engine', 'models', provider],
    queryFn: () => api.get<ModelInfo[]>(`/api/engine/models?provider=${provider}`),
    enabled: !!provider,
  })
}

export function useEngineConfig() {
  return useQuery({
    queryKey: ['engine', 'config'],
    queryFn: () => api.get<EngineConfig>('/api/engine/config'),
  })
}

export function useSetModel() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (model: string) => api.put('/api/engine/model', { model }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['engine'] }),
  })
}
```

---

## 5. 마이그레이션 단계

### Phase 1: SDK 버전 업그레이드 (breaking change 최소화)

1. `Cargo.toml`에서 `oxi-sdk = "0.22.0"`, `oxi-ai = "0.22.0"`으로 변경
2. `engine.rs`에서 `register_compatible_providers()` 제거 (oxi-sdk 빌트인에 이미 포함)
3. `agent_runtime.rs`에서 `AgentLoopConfig`의 `provider_options` 필드 추가 (일단 `None`으로)
4. `cargo test --workspace` 통과 확인

### Phase 2: EngineApi + 웹 API 라우트

1. `kernel_handle/engine_api.rs` 추가
2. `routes/engine_routes.rs` 추가
3. `routes/mod.rs`에 라우트 등록
4. `KernelHandle`에 `EngineApi` 필드 추가
5. `config.rs`의 `EngineConfig`에 `provider_options` 필드 추가

### Phase 3: Frontend UI

1. `types/engine.ts` 타입 정의
2. `hooks/use-engine.ts` API 훅
3. `components/engine/` 컴포넌트 4개
4. `settings.tsx`에 Engine 탭 통합

### Phase 4: 핫스왑 + ProviderOptions 완성

1. `OxiosEngine` → `RwLock` 래핑으로 핫스왑 지원
2. `agent_runtime.rs`에서 `config.provider_options`를 `AgentLoopConfig`에 전달
3. `onboarding.rs` 개선 (기존 구조 유지, oxi-sdk 0.22 API만 사용)

---

## 6. 삭제되는 코드

| 파일 | 코드 | 이유 |
|------|------|------|
| `engine.rs` | `register_compatible_providers()` | oxi-sdk 0.22 빌트인에 zai 포함 |
| `engine.rs` | `OxiosEngine::new()`의 하드코딩된 provider 분기 | `OxiBuilder::with_builtins()`가 모든 것을 처리 |
| `engine.rs` | `EngineProvider` 트레이트 (선택적) | `OxiosEngine` 직접 사용으로 단순화 가능 |
| `settings.tsx` | Engine 섹션의 raw text 필드 | 드롭다운 기반 컴포넌트로 교체 |

---

## 7. 호환성 보장

| 보장 | 방법 |
|------|------|
| **config.toml 하위 호환** | `provider_options`가 없으면 `None`으로 기본값 처리. 기존 config.toml 그대로 작동 |
| **API 하위 호환** | `GET /api/config`는 유지. 새 `/api/engine/*`은 추가 경로 |
| **Onboarding 무변경** | CLI 마법사는 oxi-sdk 0.22 API로 자동 작동 (기존 `get_providers()`, `get_provider_models()` 동일) |
| **런타임 무변경** | Kernel builder에서 Oxi 인스턴스 생성 로직은 동일. `with_builtins()`가 0.22의 확장된 빌트인을 자동 포함 |

---

## 8. 검증 체크리스트

- [ ] `cargo build` — oxi-sdk 0.22.0으로 빌드 성공
- [ ] `cargo test --workspace` — 모든 테스트 통과
- [ ] 기존 `~/.oxios/config.toml`으로 데몬 시작 — 하위 호환
- [ ] `GET /api/engine/providers` — 47개 프로바이더 반환
- [ ] `GET /api/engine/models?provider=anthropic` — 23개 모델 반환
- [ ] `PUT /api/engine/model` — 모델 변경 후 config.toml에 반영
- [ ] Web UI에서 Anthropic 선택 → adaptive thinking 설정 → config.toml에 반영
- [ ] `oxios run --json "test"` — CLI 실행 정상
- [ ] Onboarding 마법사 — 0.22 API로 정상 동작

---

# Appendix A: Self-Review Findings

> 초기 설계에 대한 자체 리뷰 결과. Critical/Major/Minor 분류.

## Critical 1: 핫스왑이 불가능한 객체 그래프

**문제**: RFC에서 "Web UI에서 모델 변경 → 런타임 핫스왑"을 주장하지만, 실제 객체 그래프는 핫스왑을 지원하지 않음.

```
Oxi 인스턴스 (한 번 생성)
  → resolve_model() → Model (값)
  → create_provider() → Arc<dyn Provider>
    ↓ 복사됨
    ├→ OuroborosEngine { provider: Arc<dyn Provider>, model: Model }
    ├→ AgentRuntime { provider: Arc<dyn Provider>, ... }
    │   └→ BasicSupervisor { runtime: Arc<AgentRuntime> }  // runtime이 Arc로 고정
    └→ Orchestrator { ouroboros: Arc<dyn OuroborosProtocol> }
```

`Arc<dyn Provider>`가 4곳에 복사되어 들어가 있고, `Arc<AgentRuntime>`은 `BasicSupervisor`에 고정됨.
`OxiosEngine`에 `RwLock`을 씌워도 하위 컴포넌트의 `Arc`는 업데이트되지 않음.

**해결 방안 (둘 중 하나 선택)**:

- **A. 핫스왑 포기**: config.toml 저장만 하고, 런타임 반영은 데몬 재시작으로 처리.
  가장 단순하고 안전. "변경 사항을 적용하려면 재시작이 필요합니다" 배너 표시.
- **B. 간접 참조 도입**: `AgentRuntime`과 `OuroborosEngine`이 직접 `Arc<dyn Provider>`를
  들고 있는 대신 `Arc<ArcSwap<dyn Provider>>` 같은 간접 참조를 사용. 하지만 이건
  oxi-sdk의 `Provider` 트레이트와 oxi-ouroboros 크레이트에도 영향을 줌.

**권장**: **A안 (핫스왑 포기)**. Phase 4를 전체 삭제하고, Web UI에서는
"Restart required" 상태를 명시적으로 보여줌. 핫스왑은 추후 별도 RFC로 설계.

## Critical 2: EngineApi가 핫스왑에 필요한 의존성이 없음

**문제**: RFC 4.4절에서 `EngineApi`는 `config`와 `config_path`만 가짐.
하지만 실제 핫스왑을 하려면 Oxi 인스턴스, Supervisor, OuroborosEngine 접근이 필요.

Critical 1의 해결 방안(A안)을 채택하면, EngineApi의 역할은 **읽기 전용 뷰**로 축소됨:

```rust
pub struct EngineApi {
    config: Arc<parking_lot::RwLock<OxiosConfig>>,
    config_path: PathBuf,
}
```

쓰기 작업은 config.toml 업데이트만 하면 됨. 핫스왑은 불필요.

## Critical 3: `register_compatible_providers()` 제거 불가

**문제**: RFC에서 "zai 등은 oxi-sdk 0.22 빌트인에 포함되므로 하드코딩된 factory 등록이 불필요"라고 했으나,
이건 **틀림**.

oxi-sdk 0.22의 `create_builtin_provider("zai")`는 `OpenAiProvider { api_key: None, base_url: Some("https://api.z.ai/...") }`를 생성함.
`api_key`가 `None`이면 `stream()` 호출 시 `ProviderError::MissingApiKey` 에러 발생.

현재 Oxios의 `provider_factory("zai", ...)`는 `CredentialStore::resolve()`로 키를 찾아서
`with_base_url_and_key(url, Some(key))`로 주입하는 역할을 함.
이걸 제거하면 **zai 프로바이더가 동작하지 않음**.

**해결 방안 (둘 중 하나 선택)**:

- **A. factory 유지**: `register_compatible_providers()`를 유지하되, zai 뿐만 아니라
  credential이 필요한 모든 프로바이더에 대해 factory를 등록. Oxios의 CredentialStore가
  oxi-sdk의 `load_token()`보다 우선순위가 높기 때문에 여전히 의미가 있음.
- **B. AgentLoopConfig.api_key 활용**: oxi-sdk 0.22의 새 기능인 `AgentLoopConfig.api_key`에
  CredentialStore에서 resolve한 키를 전달. 이러면 factory가 필요 없음. 대신
  `AgentRuntime`이 매 요청마다 `CredentialStore::resolve()`를 호출해야 함.

**권장**: **B안**. `AgentRuntimeConfig`에 `api_key` 필드를 추가하고, `run_agent_loop()`에서
`AgentLoopConfig.api_key`에 전달. 이러면 factory 패턴 전체를 제거할 수 있음.
factory는 "첫 호출시에만" credential을 resolve하는데 비해, 이 방식은 "매 요청마다"
resolve하므로 credential이 중간에 변경되어도 반영됨.

## Major 1: ProviderOptions 전달 경로 누락

**문제**: RFC 4.6절에서 `AgentLoopConfig`에 `provider_options`를 넣는다고 했지만,
`AgentRuntimeConfig`에는 `provider_options` 필드가 없음.

전달 경로:
```
config.toml [engine.provider_options]
  → EngineConfig.provider_options
  → AgentRuntimeConfig.provider_options (신규 필드 필요)
  → AgentLoopContext.config.provider_options
  → AgentLoopConfig.provider_options
  → StreamOptions.provider_options
  → Provider.stream()에서 읽음
```

`AgentRuntimeConfig`에 `pub provider_options: Option<ProviderOptions>` 필드를 추가해야 함.

## Major 2: 핫스왑 과도 설계 — 실제 필요성 검증 누락

**문제**: 현재 사용자는 모델 변경 시 데몬을 재시작함. 핫스왑 없이도 충분.

Critical 1과 동일한 맥락에서, Phase 4 전체를 별도 RFC로 분리하는 것이 합리적.
Phase 1-3은 "config 저장 + Web UI 개선"에 집중.

## Major 3: `EngineProvider` 트레이트 제거 검증 누락

**문제**: RFC 6절에서 `EngineProvider` 트레이트를 "선택적 삭제" 대상으로 분류했으나,
이 트레이트는 `engine.rs`의 테스트와 mock 교체에 사용됨.

삭제하려면 테스트를 모두 `OxiosEngine` 직접 사용으로 변경해야 함.
마이그레이션 범위를 최소화하려면 유지하는 것이 좋음.

**권장**: 유지. 마이그레이션 범위 최소화.

## Major 4: `ProviderOptions`의 TOML 직렬화 검증 누락

**문제**: `ProviderOptions`는 oxi-ai의 타입인데, 이걸 `EngineConfig`에 넣으면
config.toml에 TOML로 직렬화/역직렬화해야 함.

```toml
[engine.provider_options.anthropic]
thinking_type = "adaptive"
effort = "high"
```

`ProviderOptions`는 `#[derive(Serialize, Deserialize)]`가 있지만, `skip_serializing_if`가
`serde(default)`와 결합되어 있어 TOML 파서에서의 동작을 검증해야 함.
특히 빈 `[engine.provider_options]` 섹션이 있을 때의 동작.

**권장**: Phase 2에서 config.toml round-trip 테스트를 반드시 포함.

## Minor 1: API 경로 설계가 RESTful이 아님

`PUT /api/engine/model`, `PUT /api/engine/api-key` 등은 RESTful이 아님.
엔진은 단일 리소스이므로:

```
GET  /api/engine          → 현재 설정
PUT  /api/engine          → 전체 업데이트
GET  /api/engine/providers → 프로바이더 목록
GET  /api/engine/models    → 모델 목록
```

또는 기존 `GET/PUT /api/config`를 확장하는 것이 일관성에 좋음.

**권장**: 우선순위 낮음. 기능적으로 문제없음.

## Minor 2: Frontend 타입이 oxi-ai에 종속됨

`ProviderOptions`의 TypeScript 타입을 Oxios에서 수동 정의하면,
oxi-ai에 새 프로바이더 섹션이 추가될 때마다 수동으로 업데이트해야 함.

**권장**: Backend에서 `GET /api/engine/config` 응답에 provider_options 스키마를
포함하거나, 프론트엔드에서 dynamic object로 처리.
