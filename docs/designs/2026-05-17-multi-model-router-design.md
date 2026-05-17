# Multi-Model Router Design

> **Date:** 2026-05-17  
> **Feature:** Multi-Model Provider System with Complexity Routing + Failover  
> **Status:** Draft

## 1. Problem Statement

현재 oxios는 단일 모델만 사용한다. `config.toml`의 `default_model` 하나만 지정 가능하고, 실패 시 사용자에게 에러를 보여줄 뿐이다.

**원하는 기능:**
- 여러 모델/proviver를 등록
- 태스크 복잡도에 따라 자동 라우팅 (간단한 질문 → 빠른 모델, 복잡한 분석 → 강력한 모델)
- 모델 실패 시 순차 폴백
- 웹 UI로 모델 우선순위/설정 관리

## 2. Design Overview

```
User Prompt
    │
    ▼
┌─────────────────────┐
│  Complexity Router  │ ← LLM으로 복잡도 분류 (simple/medium/complex)
└──────────┬──────────┘
           │ tier 결정
           ▼
┌─────────────────────┐
│   Model Pool        │ ← 등록된 모델 목록 (Tier 배열)
│   [sonny, sonnet,   │
│    opus, gpt-4o]    │
└──────────┬──────────┘
           │ tier → 모델 매핑
           ▼
┌─────────────────────┐
│  Circuit Breaker    │ ← 각 모델별 CB (provider 단위)
│  per-provider       │
└──────────┬──────────┘
           │ allowed?
           ▼
┌─────────────────────┐
│  Execute Agent      │ ← AgentRuntime 실행
└──────────┬──────────┘
           │ 실패?
    ┌──────┴──────┐
    │ fallback   │
    │ next model │
    └────────────┘
```

## 3. Configuration Schema

### 3.1 config.toml

```toml
[engine]
# 기존 필드 (하위 호환성 유지)
default_model = "anthropic/claude-sonnet-4-20250514"

# ── Multi-Model Router ────────────────────────────

# 라우팅 모드: "auto" | "failover_only" | "disabled"
routing_mode = "auto"

# 모델 풀 — 각 모델의 tier와 우선순위 정의
[[engine.models]]
# 모델 ID (provider/model 형식)
model_id = "anthropic/claude-sonnet-4-20250514"
# 라우팅 티어: "fast" | "balanced" | "strong"
tier = "balanced"
# 복잡도 매핑: 이 모델이 처리하는 최대 복잡도
max_complexity = "complex"  # simple, medium, complex
# 폴백 순서 (숫자가 낮을수록 먼저 시도)
priority = 1

[[engine.models]]
model_id = "anthropic/claude-haiku-4-20250514"
tier = "fast"
max_complexity = "simple"
priority = 0

[[engine.models]]
model_id = "anthropic/claude-opus-4-20250514"
tier = "strong"
max_complexity = "complex"
priority = 2

# ── Provider Credentials ────────────────────────────
[engine.providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"  # 기본값

[engine.providers.openai]
api_key_env = "OPENAI_API_KEY"

[engine.providers.zai]
api_key_env = "ZAI_API_KEY"
base_url = "https://api.z.ai/api/coding/paas/v4"
```

### 3.2 Complexity Classification

사용자 입력의 복잡도를 3단계로 분류:

| Tier | Complexity | 예시 | 모델 후보 |
|------|------------|------|----------|
| `fast` | `simple` | "오늘 날짜 알려줘", "2+2 계산해줘" | Haiku, gpt-4o-mini |
| `balanced` | `medium` | "이 코드 리뷰해줘", "버그 찾아줘" | Sonnet, gpt-4o |
| `strong` | `complex` | "전체 아키텍처 설계해줘", "대규모 리팩토링" | Opus, o1 |

**분류 방식:**
- LLM 기반 분류 (작은 프롬프트로 3-tier 분류)
- 또는 키워드 기반 fast path (날짜/계산 → simple)

## 4. Architecture

### 4.1 New Components

```
crates/oxios-kernel/src/
├── router/
│   ├── mod.rs                    # Router public API
│   ├── complexity.rs             # Complexity classifier
│   ├── model_pool.rs             # Model registry & selection
│   ├── fallback.rs               # Fallback chain executor
│   └── config.rs                # RouterConfig deserialization
```

### 4.2 Core Types

```rust
// crates/oxios-kernel/src/router/config.rs

use serde::{Deserialize, Serialize};

/// Routing mode for multi-model system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    /// 자동 복잡도 라우팅 + 폴백
    Auto,
    /// 폴백만 (순서대로 시도, 복잡도 무시)
    FailoverOnly,
    /// 라우팅 비활성화 (단일 모델)
    Disabled,
}

/// Complexity level of a task.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    Simple,
    Medium,
    Complex,
}

/// A registered model in the pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub model_id: String,
    pub tier: Tier,
    pub max_complexity: Complexity,
    /// 0 = highest priority (tried first)
    pub priority: u32,
}

/// Provider credentials for a specific provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Environment variable name for API key.
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,
    /// Custom base URL (optional).
    #[serde(default)]
    pub base_url: Option<String>,
}

fn default_api_key_env() -> String {
    "API_KEY".to_string()
}

/// Router configuration loaded from config.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct RouterConfig {
    #[serde(default = "default_routing_mode")]
    pub routing_mode: RoutingMode,
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    #[serde(default)]
    pub providers: std::collections::HashMap<String, ProviderConfig>,
}

fn default_routing_mode() -> RoutingMode {
    RoutingMode::Auto
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            routing_mode: RoutingMode::Auto,
            models: Vec::new(),
            providers: std::collections::HashMap::new(),
        }
    }
}
```

### 4.3 ModelPool

```rust
// crates/oxios-kernel/src/router/model_pool.rs

use super::config::{Complexity, ModelEntry, ProviderConfig, RoutingMode};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages the pool of registered models and provider credentials.
pub struct ModelPool {
    config: RouterConfig,
    /// Oxi builder for creating providers.
    oxi_builder: OxiBuilder,
    /// Cache of resolved models.
    models: RwLock<Vec<ModelEntry>>,
}

impl ModelPool {
    /// Select the best model for a given complexity.
    ///
    /// In Auto mode: picks the cheapest model that can handle the complexity.
    /// In Failover mode: always returns the highest priority model.
    pub async fn select_model(
        &self,
        routing_mode: RoutingMode,
        complexity: Complexity,
    ) -> Option<ModelEntry> {
        let models = self.models.read().await;
        let mut candidates: Vec<_> = models.iter().collect();

        match routing_mode {
            RoutingMode::Auto => {
                // Filter: max_complexity >= required
                candidates.retain(|m| m.max_complexity >= complexity);
            }
            RoutingMode::FailoverOnly => {
                // All models are candidates
            }
            RoutingMode::Disabled => {
                // Only first model
                return candidates.first().cloned();
            }
        }

        // Sort by priority (lower = higher priority)
        candidates.sort_by_key(|m| m.priority);
        candidates.first().cloned()
    }

    /// Get the next fallback model after a given model fails.
    pub async fn next_fallback(&self, current: &str) -> Option<ModelEntry> {
        let models = self.models.read().await;
        let mut sorted = models.clone();
        sorted.sort_by_key(|m| m.priority);

        let pos = sorted.iter().position(|m| m.model_id == current);
        let start_idx = pos.map(|p| p + 1).unwrap_or(0);

        sorted.get(start_idx).cloned()
    }
}
```

### 4.4 ComplexityClassifier

```rust
// crates/oxios-kernel/src/router/complexity.rs

use super::config::Complexity;
use anyhow::Result;

/// Classifies task complexity using lightweight heuristics + optional LLM call.
pub struct ComplexityClassifier {
    /// Threshold for simple keyword detection.
    simple_keywords: Vec<&'static str>,
}

impl ComplexityClassifier {
    /// Classify complexity via fast-path heuristics + optional LLM refinement.
    ///
    /// Fast path: if prompt matches simple keywords → Simple
    /// Otherwise: LLM classification call
    pub async fn classify(&self, prompt: &str) -> Result<Complexity> {
        // Fast path: simple keyword detection
        if self.is_simple_pattern(prompt) {
            return Ok(Complexity::Simple);
        }

        // TODO: LLM-based classification (separate prompt to classifier model)
        // For now: conservative default → Medium
        Ok(Complexity::Medium)
    }

    fn is_simple_pattern(&self, prompt: &str) -> bool {
        let lower = prompt.to_lowercase();
        let simple = [
            "날짜", "시간", "오늘", "계산", "날씨",
            "what day", "date", "time", "calculate",
        ];
        simple.iter().any(|kw| lower.contains(kw)) && prompt.len() < 100
    }
}
```

### 4.5 FallbackExecutor

```rust
// crates/oxios-kernel/src/router/fallback.rs

use super::model_pool::ModelPool;
use crate::AgentRuntime;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tracks fallback state for a single execution attempt.
pub struct FallbackState {
    pub current_model: String,
    pub attempts: usize,
    pub max_attempts: usize,
}

/// Executes a task with automatic fallback on failure.
///
/// Returns (result, final_model_used, attempts).
pub async fn execute_with_fallback(
    task: impl FnOnce(String) -> Result<ExecutionResult>,
    pool: Arc<ModelPool>,
    initial_model: String,
    max_attempts: usize,
) -> Result<(ExecutionResult, String, usize)> {
    let mut state = FallbackState {
        current_model: initial_model,
        attempts: 0,
        max_attempts,
    };

    loop {
        state.attempts += 1;
        tracing::info!(model = %state.current_model, attempt = state.attempts, "Executing with model");

        match task(state.current_model.clone()).await {
            Ok(result) => {
                tracing::info!(model = %state.current_model, "Execution succeeded");
                return Ok((result, state.current_model, state.attempts));
            }
            Err(e) => {
                tracing::warn!(model = %state.current_model, error = %e, "Execution failed, trying fallback");

                if state.attempts >= state.max_attempts {
                    anyhow::bail!("All {} fallback attempts exhausted: {}", state.max_attempts, e);
                }

                if let Some(next) = pool.next_fallback(&state.current_model).await {
                    state.current_model = next.model_id;
                } else {
                    anyhow::bail!("No more fallback models available: {}", e);
                }
            }
        }
    }
}
```

## 5. Integration Points

### 5.1 KernelBuilder (src/kernel.rs)

```rust
// src/kernel.rs 변경

// Before: 단일 engine_provider
let engine_provider = OxiEngineProvider::new(model_id);

// After: ModelPool + Router
let model_pool = ModelPool::from_config(&config.engine)?;
let router = MultiModelRouter::new(model_pool, config.engine.routing_mode);
let engine_provider = router; // KernelHandle에 전달
```

### 5.2 AgentRuntime Integration

```rust
// agent_runtime.rs 변경

// Before: 생성자에서 단일 provider/model
pub fn new(provider: Arc<dyn Provider>, model_id: impl Into<String>, ...)

// After: routing_mode에 따라 동적 선택
pub async fn execute_routed(&self, prompt: &str, router: &MultiModelRouter) -> Result<ExecutionResult> {
    let complexity = self.classifier.classify(prompt).await?;
    let model_entry = router.select_model(complexity).await?;
    let provider = router.create_provider(&model_entry.model_id).await?;
    self.execute_with_provider(provider, &model_entry.model_id).await
}
```

### 5.3 Circuit Breaker per Provider

기존 글로벌 CB를 provider별로 분리:

```rust
// engine.rs 변경

static PROVIDER_CB: std::sync::RwLock<HashMap<String, CircuitBreaker>> =
    std::sync::RwLock::new(HashMap::new());

fn get_provider_circuit_breaker(provider: &str) -> &'static CircuitBreaker {
    // provider별 CB 인스턴스 반환
}
```

## 6. Web UI (oxios-web)

### 6.1 API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/engine/models` | 등록된 모델 목록 |
| POST | `/api/engine/models` | 모델 추가 |
| PUT | `/api/engine/models/{id}` | 모델 설정 변경 |
| DELETE | `/api/engine/models/{id}` | 모델 제거 |
| GET | `/api/engine/status` | 모델 상태 (healthy/unhealthy) |
| PUT | `/api/engine/routing-mode` | 라우팅 모드 변경 |

### 6.2 Frontend Components

```
channels/oxios-web/frontend/src/
├── components/
│   └── engine/
│       ├── ModelList.tsx      # 모델 목록 + 상태
│       ├── ModelCard.tsx      # 개별 모델 카드
│       ├── AddModelModal.tsx  # 모델 추가 모달
│       └── RoutingConfig.tsx  # 라우팅 설정
```

## 7. Implementation Phases

### Phase 1: Core Infrastructure
- [ ] `RouterConfig` deserialization
- [ ] `ModelPool` struct
- [ ] `ComplexityClassifier` (fast path only)
- [ ] `FallbackExecutor`
- [ ] 단일 모델 실행은 기존과 동일

### Phase 2: Routing Integration
- [ ] `MultiModelRouter` 통합 (`AgentRuntime.execute_routed()`)
- [ ] Provider별 CircuitBreaker
- [ ] KernelBuilder 연동

### Phase 3: Web UI
- [ ] REST API endpoints
- [ ] Model management UI
- [ ] Drag-and-drop priority reorder

### Phase 4: LLM-based Classification (Optional)
- [ ] 분류 전용 모델 (작은 Haiku)
- [ ] 분류 결과 캐싱

## 8. Backward Compatibility

- `default_model`이 있으면 자동转换为 `models`의 첫 번째 항목
- `routing_mode = "disabled"`이면 기존 동작과 동일
- Web API는 RESTful, 기존 API 영향 없음

## 9. Open Questions

1. **Complexitty classification**: LLM 기반 분류 시 분류용 모델도 별도 등록 필요? 아니면 built-in fallback?
2. **Provider credentials**: 기존 `CredentialStore`와 어떻게 통합? 별도 `ProviderConfig` 재정의?
3. **UI**: 모델 우선순위 drag-drop vs number input哪个更直观?
4. **Cost tracking**: 각 모델별 사용량 추적? BudgetManager 연동?

---

이 디자인 문서로 진행할까? 아니면 수정하고 싶은 부분이 있어?