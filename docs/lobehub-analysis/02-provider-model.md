# LobeHub ↔ Oxios: Provider & Model Architecture

> LobeHub: 91+ providers, 3-layer architecture. Oxios: 17-18 providers, oxi-sdk delegation.

## 1. Provider Architecture

### LobeHub: Three-Layer Architecture

```
┌─────────────────────────────────────────────────────┐
│ Layer 3: model-runtime (Runtime Implementations)    │
│  · ~88 provider runtime classes                     │
│  · 2 core factories: openaiCompatibleFactory,        │
│    anthropicCompatibleFactory                       │
│  · ModelRuntime orchestrator with lifecycle hooks   │
│  · 19 context builders, 17 stream handlers          │
├─────────────────────────────────────────────────────┤
│ Layer 2: model-bank (Metadata / Type Definitions)   │
│  · 85 ModelProviderCard definitions                │
│  · Per-model ChatModelCard entries (~2000+)         │
│  · Pricing, abilities, context windows, cutoff dates│
│  · AiModelType: chat|embedding|tts|asr|image|video  │
├─────────────────────────────────────────────────────┤
│ Layer 1: Database (User Configuration)              │
│  · AiProviderModel — per-user/workspace CRUD        │
│  · KeyVaults encryption (API keys at rest)          │
│  · Enabled/disabled, sort order, custom config      │
└─────────────────────────────────────────────────────┘
```

Provider count: 91 enum values in `ModelProvider`, 88 entries in `providerRuntimeMap`, 85 in `DEFAULT_MODEL_PROVIDER_LIST`.

#### Provider Interface (`OpenAICompatibleFactoryOptions`)

Every provider is defined through `createOpenAICompatibleRuntime()`:

- **baseURL**: API endpoint
- **chatCompletion**: `handlePayload` (transform), `handleError`, `handleStream`, `contextPreFlight` (token estimation)
- **models**: Dynamic model list fetching (`client.models.list()`)
- **debug**: Per-provider debug flags
- **generateObject**: Structured output with schema transforms
- **createImage / createVideo**: Multimodal generation
- **errorType**: Custom error classification
- **defaultHeaders / defaultQuery**: Custom HTTP injection

Sample: OpenAI provider (`packages/model-runtime/src/providers/openai/index.ts`):
```typescript
export const params = {
  baseURL: 'https://api.openai.com/v1',
  chatCompletion: {
    contextPreFlight: { models: openaiChatModels },
    handlePayload: (payload) => { /* Routing: Responses API vs Chat Completions */ },
  },
  models: async ({ client }) => {
    const modelsPage = await client.models.list();  // Live fetching!
    return processMultiProviderModelList(modelList, 'openai');
  },
} satisfies OpenAICompatibleFactoryOptions;
```

### Oxios: Single-Layer Delegation to oxi-sdk

```
┌────────────────────────────────────────────────┐
│ OxiosEngine (crates/oxios-kernel/src/engine.rs)│
│  · Wraps oxi_sdk::OxiBuilder                  │
│  · CredentialStore for API key resolution      │
│  · 18 hardcoded providers for credential lookup│
│  · FileModelCatalog for models.dev metadata    │
│  · RoutingControl via oxi-sdk                  │
├────────────────────────────────────────────────┤
│ EngineApi (kernel_handle/engine_api.rs)        │
│  · Hot-swap engine on config change            │
│  · Model validation before persistence         │
│  · Provider list, models query, routing stats   │
├────────────────────────────────────────────────┤
│ Frontend Catalog (web/src/hooks/use-engine.ts) │
│  · 17 static ProviderInfo entries              │
│  · ~30 static ModelInfo entries (5 providers)  │
└────────────────────────────────────────────────┘
```

Provider count: 17 frontend + 18 kernel (kernel adds `together` not in frontend).

## 2. Provider Config UI

### LobeHub: Full Settings Provider Config

Provider settings at `/settings/provider/<id>`:

- **`ProviderConfig`**: API key input, base URL, enabled toggle, model list
- **Per-provider detail pages**: Custom UIs for Bedrock, ComfyUI, Azure, GitHub
- **Provider-specific auth flows**: OAuth device flow (GitHub Copilot), AWS credential modes (Bedrock), password+username (ComfyUI)
- **`KeyVaultsGateKeeper`**: Encrypts API keys before storing in DB
- **`AiProviderModel`**: Full CRUD with workspace-scoping, key encryption, sort ordering

### Oxios: Settings Page Provider Section

Integrated into Settings page (`web/src/routes/settings.tsx`):

- **`ProviderCard`**: Provider name, category, key source, model count, validate, change key, delete
- **`AddProviderCard`**: Dropdown to select provider + API key input
- **`ProviderOptionsPanel`**: Per-provider advanced options (Anthropic thinking, OpenAI reasoning, Google thinking level)
- **Key validation**: `POST /api/engine/validate-key` endpoint
- **`ApiKeyInput`**: Source detection (env, auth_store, config, none)

### Key Differences

| Aspect | LobeHub | Oxios |
|--------|---------|-------|
| Key storage | Encrypted in PostgreSQL | Env vars + config.toml + ~/.oxi/auth.json |
| Provider enable/disable | Per-user toggle | Key presence determines availability |
| Custom base URL | Per-provider setting | Per-provider setting |
| OAuth auth | GitHub Copilot device flow | None |
| Model list management | Per-provider custom model list | None |
| Connection checker | "Check" button per provider | Validate key endpoint |

## 3. Model Selection UX

### LobeHub: ModelSwitchPanel

Triggered from chat input action bar. Features:

- **Searchable** with keyword highlighting
- **Group modes**: By provider / By model (cross-provider)
- **Provider headers** with settings shortcut
- **Model detail panel** (right-side submenu): radar chart (ratings), pricing, context window, parameter size
- **Reasoning/thinking parameter sliders**: Per-model controls (GPT-5 reasoning effort, Claude thinking budget, Gemini thinking level)
- **Benchmark comparison modal**: Compare model ratings across providers
- **"New" badge** for recently released models
- **Resizable panel** (in dev mode)

### Oxios: ModelSelect

Simpler dropdown (`web/src/components/engine/model-select.tsx`):

- **Searchable** by name, id, or provider
- **Grouped by provider** with reasoning/standard split
- **Reasoning indicator** (✦) and vision indicator (👁)
- **Cost + context window display** per row
- **Check mark** for selected model

### Gap Analysis

| Feature | LobeHub | Oxios |
|---------|---------|-------|
| Model detail panel | Radar chart, benchmark, pricing | None |
| Per-model parameter controls | 40+ specialized slider components | Generic ProviderOptions (3 providers) |
| Benchmark comparison | Cross-provider rating comparison | None |
| "New" badges | Release tracking | None |
| Cross-provider grouping | Group by model across providers | Provider-grouped only |
| Empty state CTA | "No provider configured" → settings | None |
| Resizable panel | Yes (dev mode) | No |

## 4. Environment-Based Configuration

### LobeHub

**Dockerfile** (`lines 223-343`): ~40 provider blocks, each with:
```bash
OPENAI_API_KEY=""  OPENAI_MODEL_LIST=""  OPENAI_PROXY_URL=""
ANTHROPIC_API_KEY=""  ANTHROPIC_MODEL_LIST=""  ANTHROPIC_PROXY_URL=""
# ... 38 more providers
```

**`.env.example`**: ~25 commented-out provider blocks with documentation.

### Oxios

**Credential resolution order** (kernel `CredentialStore::resolve()`):
1. Config-level key from `config.toml` (`[secrets.providers]`)
2. Environment variable (`OXIOS_<PROVIDER>_API_KEY` or `<PROVIDER>_API_KEY`)
3. `~/.oxi/auth.json` (shared oxi-cli store)

18 kernel providers in `engine.rs:130-155`:
```rust
let mut providers_to_try: Vec<String> = vec![
    "anthropic", "openai", "google", "deepseek", "xai", "groq",
    "openrouter", "mistral", "cerebras", "fireworks", "github-copilot",
    "huggingface", "together", "minimax", "moonshotai",
    "kimi-coding", "zai", "opencode",
];
```

## 5. What Oxios Does Better

- **Rust engine**: Type-safe, no runtime type errors, better performance
- **Hot-swap**: `EngineHandle` allows engine rebuild without daemon restart
- **Cost-efficient routing**: `preferCostEfficient` toggle with fallback/excluded model lists
- **Role-based routing**: Persona→model mapping (RFC-032)
- **Circuit breaker**: 3-state protection against provider failures
- **Resilience ladder**: RFC-029 execution resilience with model switching on failure
- **Token-maxing**: Unattended work scheduling within subscription quotas
- **Provider pooling**: Rate-limited provider pools (`pooled_provider()`)
- **Model catalog port**: `FileModelCatalog` with models.dev metadata, cache, user overrides

## 6. Recommended Additions (Priority-Ordered)

### High Priority
1. **Model list management**: Per-provider custom model list (allow/deny specific models)
2. **Model metadata enrichment**: Surface pricing, context window, abilities in model select
3. **Connection checker**: "Test Connection" button visible in provider card
4. **Provider enable/disable**: Toggle providers without deleting keys

### Medium Priority
5. **Model detail panel**: Slide-out with pricing breakdown, parameter info
6. **Per-model parameter controls**: Reasoning effort, thinking budget per model (not just per provider)
7. **Provider sort ordering**: Pin/reorder providers in model picker
8. **Provider registry expansion**: Add top 30 providers (Cohere, TogetherAI, Perplexity, Qwen, etc.)

### Low Priority
9. **OAuth device flow**: For GitHub Copilot and similar
10. **Live model discovery**: `GET /api/engine/providers/:id/models` → provider API
11. **Benchmark comparison**: Cross-provider model rating comparison
