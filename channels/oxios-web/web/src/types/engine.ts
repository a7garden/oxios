// ─── Engine Types ─────────────────────────────────────────────
// Types for the engine configuration UI — provider selection,
// model browsing, API key management, and per-provider options.

/** Input modality for a model. */
export type InputModality = 'text' | 'image'

/** API protocol used by a provider. */
export type ApiProtocol =
  | 'openai-completions'
  | 'openai-responses'
  | 'anthropic-messages'
  | 'google-generative-ai'
  | 'mistral-conversations'

/** A model entry from the provider catalog. */
export interface ModelInfo {
  /** Model identifier (e.g. "claude-sonnet-4-20250514") */
  id: string
  /** Human-readable name (e.g. "Claude Sonnet 4") */
  name: string
  /** API protocol */
  api: ApiProtocol
  /** Provider name */
  provider: string
  /** Whether the model supports reasoning/thinking */
  reasoning: boolean
  /** Supported input modalities */
  input: InputModality[]
  /** Cost per million input tokens (USD) */
  cost_input: number
  /** Cost per million output tokens (USD) */
  cost_output: number
  /** Cost per million cached read tokens (USD) */
  cost_cache_read: number
  /** Cost per million cached write tokens (USD) */
  cost_cache_write: number
  /** Maximum context window in tokens */
  context_window: number
  /** Maximum output tokens */
  max_tokens: number
}

/** Provider category for grouping in the UI. */
export type ProviderCategory = 'major' | 'open' | 'regional' | 'local'

/** Provider info for selection UI. */
export interface ProviderInfo {
  /** Provider identifier (e.g. "anthropic") */
  id: string
  /** Human-readable name */
  name: string
  /** Category for grouping */
  category: ProviderCategory
  /** Whether an API key is detected (env or auth store) */
  has_key: boolean
  /** Number of available models */
  model_count: number
}

/** API key source detection. */
export type ApiKeySource = 'env' | 'auth_store' | 'config' | 'none'

/** Engine configuration returned from the backend. */
export interface EngineConfig {
  /** Default model in "provider/model" format */
  default_model: string
  /** Whether an API key is currently set (masked in API responses) */
  api_key_set: boolean
  /** API key source if detectable */
  api_key_source?: ApiKeySource
}

/** Per-provider options for advanced configuration. */
export interface ProviderOptions {
  // Anthropic
  thinking_type?: 'enabled' | 'disabled'
  thinking_budget_tokens?: number
  // OpenAI
  reasoning_effort?: 'low' | 'medium' | 'high'
  text_verbosity?: 'low' | 'medium' | 'high'
  // Google
  thinking_level?: 'none' | 'light' | 'medium' | 'heavy'
  thinking_budget?: number
}

/** Response shape for the engine config endpoint. */
export interface EngineConfigResponse {
  engine: EngineConfig
  providers: ProviderInfo[]
}

/** Response shape for provider models. */
export interface ProviderModelsResponse {
  provider: string
  models: ModelInfo[]
}
