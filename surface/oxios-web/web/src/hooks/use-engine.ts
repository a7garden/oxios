import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type {
  EngineConfigResponse,
  ModelInfo,
  ProviderInfo,
  ProviderModelsResponse,
} from '@/types/engine'

// ── Static provider catalog ──────────────────────────────────
// Mirrors the oxi-ai provider registry so the frontend can
// display provider/model information without a dedicated API.

/** Known providers with display metadata. */
const PROVIDER_CATALOG: ProviderInfo[] = [
  { id: 'anthropic', name: 'Anthropic', category: 'major', has_key: false, model_count: 0 },
  { id: 'openai', name: 'OpenAI', category: 'major', has_key: false, model_count: 0 },
  { id: 'google', name: 'Google Gemini', category: 'major', has_key: false, model_count: 0 },
  { id: 'groq', name: 'Groq', category: 'open', has_key: false, model_count: 0 },
  { id: 'openrouter', name: 'OpenRouter', category: 'open', has_key: false, model_count: 0 },
  { id: 'deepseek', name: 'DeepSeek', category: 'open', has_key: false, model_count: 0 },
  { id: 'mistral', name: 'Mistral', category: 'open', has_key: false, model_count: 0 },
  { id: 'xai', name: 'xAI (Grok)', category: 'open', has_key: false, model_count: 0 },
  { id: 'cerebras', name: 'Cerebras', category: 'open', has_key: false, model_count: 0 },
  { id: 'fireworks', name: 'Fireworks', category: 'open', has_key: false, model_count: 0 },
  { id: 'github-copilot', name: 'GitHub Copilot', category: 'open', has_key: false, model_count: 0 },
  { id: 'huggingface', name: 'Hugging Face', category: 'open', has_key: false, model_count: 0 },
]

/** Static model catalog (key providers with popular models). */
const MODEL_CATALOG: Record<string, ModelInfo[]> = {
  anthropic: [
    {
      id: 'claude-sonnet-4-20250514',
      name: 'Claude Sonnet 4',
      api: 'anthropic-messages',
      provider: 'anthropic',
      reasoning: true,
      input: ['text', 'image'],
      cost_input: 3,
      cost_output: 15,
      cost_cache_read: 0.3,
      cost_cache_write: 3.75,
      context_window: 200000,
      max_tokens: 16384,
    },
    {
      id: 'claude-opus-4-20250514',
      name: 'Claude Opus 4',
      api: 'anthropic-messages',
      provider: 'anthropic',
      reasoning: true,
      input: ['text', 'image'],
      cost_input: 15,
      cost_output: 75,
      cost_cache_read: 1.5,
      cost_cache_write: 18.75,
      context_window: 200000,
      max_tokens: 32768,
    },
    {
      id: 'claude-3-5-haiku-20241022',
      name: 'Claude 3.5 Haiku',
      api: 'anthropic-messages',
      provider: 'anthropic',
      reasoning: false,
      input: ['text', 'image'],
      cost_input: 1,
      cost_output: 5,
      cost_cache_read: 0.1,
      cost_cache_write: 1.25,
      context_window: 200000,
      max_tokens: 8192,
    },
  ],
  openai: [
    {
      id: 'gpt-4o',
      name: 'GPT-4o',
      api: 'openai-completions',
      provider: 'openai',
      reasoning: false,
      input: ['text', 'image'],
      cost_input: 2.5,
      cost_output: 10,
      cost_cache_read: 1.25,
      cost_cache_write: 0,
      context_window: 128000,
      max_tokens: 16384,
    },
    {
      id: 'gpt-4.1',
      name: 'GPT-4.1',
      api: 'openai-responses',
      provider: 'openai',
      reasoning: true,
      input: ['text', 'image'],
      cost_input: 2,
      cost_output: 8,
      cost_cache_read: 0.5,
      cost_cache_write: 0,
      context_window: 1047576,
      max_tokens: 32768,
    },
    {
      id: 'o3',
      name: 'o3',
      api: 'openai-responses',
      provider: 'openai',
      reasoning: true,
      input: ['text', 'image'],
      cost_input: 2,
      cost_output: 8,
      cost_cache_read: 0.5,
      cost_cache_write: 0,
      context_window: 200000,
      max_tokens: 100000,
    },
    {
      id: 'o4-mini',
      name: 'o4-mini',
      api: 'openai-responses',
      provider: 'openai',
      reasoning: true,
      input: ['text', 'image'],
      cost_input: 1.1,
      cost_output: 4.4,
      cost_cache_read: 0.275,
      cost_cache_write: 0,
      context_window: 200000,
      max_tokens: 100000,
    },
  ],
  google: [
    {
      id: 'gemini-2.5-pro',
      name: 'Gemini 2.5 Pro',
      api: 'google-generative-ai',
      provider: 'google',
      reasoning: true,
      input: ['text', 'image'],
      cost_input: 1.25,
      cost_output: 10,
      cost_cache_read: 0.315,
      cost_cache_write: 0,
      context_window: 1048576,
      max_tokens: 65536,
    },
    {
      id: 'gemini-2.5-flash',
      name: 'Gemini 2.5 Flash',
      api: 'google-generative-ai',
      provider: 'google',
      reasoning: true,
      input: ['text', 'image'],
      cost_input: 0.15,
      cost_output: 3.5,
      cost_cache_read: 0.0375,
      cost_cache_write: 0,
      context_window: 1048576,
      max_tokens: 65536,
    },
  ],
  groq: [
    {
      id: 'llama-3.3-70b-versatile',
      name: 'Llama 3.3 70B',
      api: 'openai-completions',
      provider: 'groq',
      reasoning: false,
      input: ['text'],
      cost_input: 0.59,
      cost_output: 0.79,
      cost_cache_read: 0,
      cost_cache_write: 0,
      context_window: 131072,
      max_tokens: 32768,
    },
  ],
  openrouter: [
    {
      id: 'auto',
      name: 'Auto (Router)',
      api: 'openai-completions',
      provider: 'openrouter',
      reasoning: false,
      input: ['text'],
      cost_input: 0,
      cost_output: 0,
      cost_cache_read: 0,
      cost_cache_write: 0,
      context_window: 128000,
      max_tokens: 4096,
    },
  ],
  deepseek: [
    {
      id: 'deepseek-r1',
      name: 'DeepSeek R1',
      api: 'openai-completions',
      provider: 'deepseek',
      reasoning: true,
      input: ['text'],
      cost_input: 0.55,
      cost_output: 2.19,
      cost_cache_read: 0.14,
      cost_cache_write: 0,
      context_window: 131072,
      max_tokens: 16384,
    },
    {
      id: 'deepseek-chat',
      name: 'DeepSeek V3',
      api: 'openai-completions',
      provider: 'deepseek',
      reasoning: false,
      input: ['text'],
      cost_input: 0.27,
      cost_output: 1.1,
      cost_cache_read: 0.07,
      cost_cache_write: 0,
      context_window: 131072,
      max_tokens: 8192,
    },
  ],
  mistral: [
    {
      id: 'mistral-large-latest',
      name: 'Mistral Large',
      api: 'mistral-conversations',
      provider: 'mistral',
      reasoning: false,
      input: ['text'],
      cost_input: 2,
      cost_output: 6,
      cost_cache_read: 0.5,
      cost_cache_write: 0,
      context_window: 131072,
      max_tokens: 8192,
    },
  ],
  xai: [
    {
      id: 'grok-3',
      name: 'Grok 3',
      api: 'openai-completions',
      provider: 'xai',
      reasoning: false,
      input: ['text', 'image'],
      cost_input: 3,
      cost_output: 15,
      cost_cache_read: 0,
      cost_cache_write: 0,
      context_window: 131072,
      max_tokens: 16384,
    },
  ],
  cerebras: [
    {
      id: 'llama-3.3-70b',
      name: 'Llama 3.3 70B',
      api: 'openai-completions',
      provider: 'cerebras',
      reasoning: false,
      input: ['text'],
      cost_input: 0,
      cost_output: 0,
      cost_cache_read: 0,
      cost_cache_write: 0,
      context_window: 131072,
      max_tokens: 8192,
    },
  ],
  fireworks: [
    {
      id: 'llama-v3p3-70b-instruct',
      name: 'Llama 3.3 70B',
      api: 'openai-completions',
      provider: 'fireworks',
      reasoning: false,
      input: ['text'],
      cost_input: 0.9,
      cost_output: 0.9,
      cost_cache_read: 0,
      cost_cache_write: 0,
      context_window: 131072,
      max_tokens: 16384,
    },
  ],
  'github-copilot': [
    {
      id: 'gpt-4o',
      name: 'GPT-4o (Copilot)',
      api: 'openai-completions',
      provider: 'github-copilot',
      reasoning: false,
      input: ['text', 'image'],
      cost_input: 0,
      cost_output: 0,
      cost_cache_read: 0,
      cost_cache_write: 0,
      context_window: 128000,
      max_tokens: 16384,
    },
  ],
  huggingface: [
    {
      id: 'meta-llama/Llama-3.3-70B-Instruct',
      name: 'Llama 3.3 70B',
      api: 'openai-completions',
      provider: 'huggingface',
      reasoning: false,
      input: ['text'],
      cost_input: 0,
      cost_output: 0,
      cost_cache_read: 0,
      cost_cache_write: 0,
      context_window: 131072,
      max_tokens: 8192,
    },
  ],
}

// ── Hooks ────────────────────────────────────────────────────

/** Fetch the list of available providers with model counts. */
export function useProviders() {
  return useQuery({
    queryKey: ['engine', 'providers'],
    queryFn: async (): Promise<ProviderInfo[]> => {
      // Try the backend endpoint first; fall back to static catalog
      try {
        const res = await api.get<EngineConfigResponse>('/api/engine/config')
        return res.providers
      } catch {
        // Backend endpoint not available — use static catalog
      }

      // Enrich static catalog with model counts
      return PROVIDER_CATALOG.map((p) => ({
        ...p,
        model_count: (MODEL_CATALOG[p.id] ?? []).length,
      }))
    },
    staleTime: 5 * 60 * 1000, // 5 min — providers rarely change
  })
}

/** Fetch models for a specific provider. */
export function useModels(provider: string | null) {
  return useQuery({
    queryKey: ['engine', 'models', provider],
    queryFn: async (): Promise<ModelInfo[]> => {
      if (!provider) return []

      // Try backend endpoint first
      try {
        const res = await api.get<ProviderModelsResponse>(`/api/engine/providers/${provider}/models`)
        return res.models
      } catch {
        // Fall back to static catalog
      }

      return MODEL_CATALOG[provider] ?? []
    },
    enabled: !!provider,
    staleTime: 5 * 60 * 1000,
  })
}

/** Fetch the current engine configuration. */
export function useEngineConfig() {
  return useQuery({
    queryKey: ['engine', 'config'],
    queryFn: async () => {
      // Fetch the full config and extract engine section
      const config = await api.get<Record<string, unknown>>('/api/config')
      const engine = config.engine as Record<string, unknown> | undefined
      return {
        default_model: (engine?.default_model as string) ?? '',
        api_key_set: (engine?.api_key_set as boolean) ?? false,
      }
    },
  })
}

/** Set the default model. */
export function useSetModel() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (model: string) => {
      // Read current config, update engine.default_model, write back
      const config = await api.get<Record<string, unknown>>('/api/config')
      if (!config.engine || typeof config.engine !== 'object') {
        config.engine = {}
      }
      ;(config.engine as Record<string, unknown>).default_model = model
      return api.put('/api/config', config)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'config'] })
      qc.invalidateQueries({ queryKey: ['config'] })
    },
  })
}

/** Set the API key for the current provider. */
export function useSetApiKey() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ provider: _provider, apiKey }: { provider: string; apiKey: string }) => {
      const config = await api.get<Record<string, unknown>>('/api/config')
      if (!config.engine || typeof config.engine !== 'object') {
        config.engine = {}
      }
      ;(config.engine as Record<string, unknown>).api_key = apiKey || undefined
      return api.put('/api/config', config)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'config'] })
      qc.invalidateQueries({ queryKey: ['config'] })
    },
  })
}

/** Set per-provider options (e.g. thinking_type, reasoning_effort). */
export function useSetProviderOptions() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({
      provider: _provider,
      options,
    }: {
      provider: string
      options: Record<string, unknown>
    }) => {
      const config = await api.get<Record<string, unknown>>('/api/config')
      if (!config.engine || typeof config.engine !== 'object') {
        config.engine = {}
      }
      const engine = config.engine as Record<string, unknown>
      // Store provider options under engine.providers.<provider>
      if (!engine.providers) engine.providers = {}
      const providers = engine.providers as Record<string, unknown>
      providers[_provider] = { ...(providers[_provider] as Record<string, unknown>), ...options }
      return api.put('/api/config', config)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'config'] })
      qc.invalidateQueries({ queryKey: ['config'] })
    },
  })
}
