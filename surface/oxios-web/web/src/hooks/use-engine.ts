import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { EngineConfigResponse, ModelInfo, ProviderInfo, ProviderModelsResponse } from '@/types/engine'
import type { RoutingConfig, RoutingStats, FallbackHistoryResponse } from '@/types/routing'

// ── Static provider catalog ──────────────────────────────────
// Mirrors the oxi-ai provider registry so the frontend can
// display provider/model information without a dedicated API.

/** Known providers with display metadata. */
const PROVIDER_CATALOG: ProviderInfo[] = [
  { id: 'anthropic', name: 'Anthropic', category: 'major', hasKey: false, modelCount: 0 },
  { id: 'openai', name: 'OpenAI', category: 'major', hasKey: false, modelCount: 0 },
  { id: 'google', name: 'Google Gemini', category: 'major', hasKey: false, modelCount: 0 },
  { id: 'groq', name: 'Groq', category: 'open', hasKey: false, modelCount: 0 },
  { id: 'openrouter', name: 'OpenRouter', category: 'open', hasKey: false, modelCount: 0 },
  { id: 'deepseek', name: 'DeepSeek', category: 'open', hasKey: false, modelCount: 0 },
  { id: 'mistral', name: 'Mistral', category: 'open', hasKey: false, modelCount: 0 },
  { id: 'xai', name: 'xAI (Grok)', category: 'open', hasKey: false, modelCount: 0 },
  { id: 'cerebras', name: 'Cerebras', category: 'open', hasKey: false, modelCount: 0 },
  { id: 'fireworks', name: 'Fireworks', category: 'open', hasKey: false, modelCount: 0 },
  { id: 'github-copilot', name: 'GitHub Copilot', category: 'open', hasKey: false, modelCount: 0 },
  { id: 'huggingface', name: 'Hugging Face', category: 'open', hasKey: false, modelCount: 0 },
]

/** Static model catalog (key providers with popular models). */
const MODEL_CATALOG: Record<string, ModelInfo[]> = {
  anthropic: [
    { id: 'claude-sonnet-4-20250514', name: 'Claude Sonnet 4', api: 'anthropic-messages', provider: 'anthropic', reasoning: true, input: ['text', 'image'], costInput: 3, costOutput: 15, costCacheRead: 0.3, costCacheWrite: 3.75, contextWindow: 200000, maxTokens: 16384 },
    { id: 'claude-opus-4-20250514', name: 'Claude Opus 4', api: 'anthropic-messages', provider: 'anthropic', reasoning: true, input: ['text', 'image'], costInput: 15, costOutput: 75, costCacheRead: 1.5, costCacheWrite: 18.75, contextWindow: 200000, maxTokens: 32768 },
    { id: 'claude-3-5-haiku-20241022', name: 'Claude 3.5 Haiku', api: 'anthropic-messages', provider: 'anthropic', reasoning: false, input: ['text', 'image'], costInput: 1, costOutput: 5, costCacheRead: 0.1, costCacheWrite: 1.25, contextWindow: 200000, maxTokens: 8192 },
  ],
  openai: [
    { id: 'gpt-4o', name: 'GPT-4o', api: 'openai-completions', provider: 'openai', reasoning: false, input: ['text', 'image'], costInput: 2.5, costOutput: 10, costCacheRead: 1.25, costCacheWrite: 0, contextWindow: 128000, maxTokens: 16384 },
    { id: 'gpt-4.1', name: 'GPT-4.1', api: 'openai-responses', provider: 'openai', reasoning: true, input: ['text', 'image'], costInput: 2, costOutput: 8, costCacheRead: 0.5, costCacheWrite: 0, contextWindow: 1047576, maxTokens: 32768 },
    { id: 'o3', name: 'o3', api: 'openai-responses', provider: 'openai', reasoning: true, input: ['text', 'image'], costInput: 2, costOutput: 8, costCacheRead: 0.5, costCacheWrite: 0, contextWindow: 200000, maxTokens: 100000 },
    { id: 'o4-mini', name: 'o4-mini', api: 'openai-responses', provider: 'openai', reasoning: true, input: ['text', 'image'], costInput: 1.1, costOutput: 4.4, costCacheRead: 0.275, costCacheWrite: 0, contextWindow: 200000, maxTokens: 100000 },
  ],
  google: [
    { id: 'gemini-2.5-pro', name: 'Gemini 2.5 Pro', api: 'google-generative-ai', provider: 'google', reasoning: true, input: ['text', 'image'], costInput: 1.25, costOutput: 10, costCacheRead: 0.315, costCacheWrite: 0, contextWindow: 1048576, maxTokens: 65536 },
    { id: 'gemini-2.5-flash', name: 'Gemini 2.5 Flash', api: 'google-generative-ai', provider: 'google', reasoning: true, input: ['text', 'image'], costInput: 0.15, costOutput: 3.5, costCacheRead: 0.0375, costCacheWrite: 0, contextWindow: 1048576, maxTokens: 65536 },
  ],
  groq: [
    { id: 'llama-3.3-70b-versatile', name: 'Llama 3.3 70B', api: 'openai-completions', provider: 'groq', reasoning: false, input: ['text'], costInput: 0.59, costOutput: 0.79, costCacheRead: 0, costCacheWrite: 0, contextWindow: 131072, maxTokens: 32768 },
  ],
  openrouter: [
    { id: 'auto', name: 'Auto (Router)', api: 'openai-completions', provider: 'openrouter', reasoning: false, input: ['text'], costInput: 0, costOutput: 0, costCacheRead: 0, costCacheWrite: 0, contextWindow: 128000, maxTokens: 4096 },
  ],
  deepseek: [
    { id: 'deepseek-r1', name: 'DeepSeek R1', api: 'openai-completions', provider: 'deepseek', reasoning: true, input: ['text'], costInput: 0.55, costOutput: 2.19, costCacheRead: 0.14, costCacheWrite: 0, contextWindow: 131072, maxTokens: 16384 },
    { id: 'deepseek-chat', name: 'DeepSeek V3', api: 'openai-completions', provider: 'deepseek', reasoning: false, input: ['text'], costInput: 0.27, costOutput: 1.1, costCacheRead: 0.07, costCacheWrite: 0, contextWindow: 131072, maxTokens: 8192 },
  ],
  mistral: [
    { id: 'mistral-large-latest', name: 'Mistral Large', api: 'mistral-conversations', provider: 'mistral', reasoning: false, input: ['text'], costInput: 2, costOutput: 6, costCacheRead: 0.5, costCacheWrite: 0, contextWindow: 131072, maxTokens: 8192 },
  ],
  xai: [
    { id: 'grok-3', name: 'Grok 3', api: 'openai-completions', provider: 'xai', reasoning: false, input: ['text', 'image'], costInput: 3, costOutput: 15, costCacheRead: 0, costCacheWrite: 0, contextWindow: 131072, maxTokens: 16384 },
  ],
  cerebras: [
    { id: 'llama-3.3-70b', name: 'Llama 3.3 70B', api: 'openai-completions', provider: 'cerebras', reasoning: false, input: ['text'], costInput: 0, costOutput: 0, costCacheRead: 0, costCacheWrite: 0, contextWindow: 131072, maxTokens: 8192 },
  ],
  fireworks: [
    { id: 'llama-v3p3-70b-instruct', name: 'Llama 3.3 70B', api: 'openai-completions', provider: 'fireworks', reasoning: false, input: ['text'], costInput: 0.9, costOutput: 0.9, costCacheRead: 0, costCacheWrite: 0, contextWindow: 131072, maxTokens: 16384 },
  ],
  'github-copilot': [
    { id: 'gpt-4o', name: 'GPT-4o (Copilot)', api: 'openai-completions', provider: 'github-copilot', reasoning: false, input: ['text', 'image'], costInput: 0, costOutput: 0, costCacheRead: 0, costCacheWrite: 0, contextWindow: 128000, maxTokens: 16384 },
  ],
  huggingface: [
    { id: 'meta-llama/Llama-3.3-70B-Instruct', name: 'Llama 3.3 70B', api: 'openai-completions', provider: 'huggingface', reasoning: false, input: ['text'], costInput: 0, costOutput: 0, costCacheRead: 0, costCacheWrite: 0, contextWindow: 131072, maxTokens: 8192 },
  ],
}

// ── Hooks ────────────────────────────────────────────────────

/** Fetch the list of available providers with model counts. */
export function useProviders() {
  return useQuery({
    queryKey: ['engine', 'providers'],
    queryFn: async (): Promise<ProviderInfo[]> => {
      try {
        const res = await api.get<{ providers: ProviderInfo[] }>('/api/engine/providers')
        return res.providers
      } catch {
        // Fall back to static catalog
      }
      return PROVIDER_CATALOG.map((p) => ({
        ...p,
        modelCount: (MODEL_CATALOG[p.id] ?? []).length,
      }))
    },
    staleTime: 5 * 60 * 1000,
  })
}

/** Fetch models for a specific provider. */
export function useModels(provider: string | null) {
  return useQuery({
    queryKey: ['engine', 'models', provider],
    queryFn: async (): Promise<ModelInfo[]> => {
      if (!provider) return []
      try {
        const res = await api.get<ProviderModelsResponse>(`/api/engine/models?provider=${provider}`)
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

/** Fetch the current engine configuration (from /api/engine/config, includes routing). */
export function useEngineConfig() {
  return useQuery({
    queryKey: ['engine', 'config'],
    queryFn: () => api.get<EngineConfigResponse>('/api/engine/config'),
    staleTime: 30 * 1000,
  })
}

/** Set the default model. */
export function useSetModel() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (model: string) =>
      api.put('/api/engine/model', { model_id: model }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'config'] })
    },
  })
}

/** Set the API key for a provider. */
export function useSetApiKey() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ provider, apiKey }: { provider: string; apiKey: string }) =>
      api.put('/api/engine/api-key', { provider, api_key: apiKey }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'config'] })
    },
  })
}

/** Set per-provider options. */
export function useSetProviderOptions() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ provider, options }: { provider: string; options: Record<string, unknown> }) =>
      api.put('/api/engine/provider-options', { provider, options }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'config'] })
    },
  })
}

// ── Routing hooks (RFC-011) ───────────────────────────────────

/** Extract routing config from the engine config response. */
export function useRoutingConfig() {
  const { data } = useEngineConfig()
  return {
    data: data?.routing as RoutingConfig | undefined,
  }
}

/** Update routing configuration. */
export function useSetRouting() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (body: Partial<RoutingConfig>) => api.put('/api/engine/routing', body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['engine', 'config'] })
    },
  })
}

/** Fetch routing statistics (model usage counts + costs). */
export function useRoutingStats() {
  return useQuery<RoutingStats>({
    queryKey: ['engine', 'routing', 'stats'],
    queryFn: () => api.get('/api/engine/routing/stats'),
    refetchInterval: 30000,
    retry: 1,
  })
}

/** Fetch recent fallback history. */
export function useFallbackHistory(limit = 20) {
  return useQuery<FallbackHistoryResponse>({
    queryKey: ['engine', 'routing', 'fallbacks', limit],
    queryFn: () => api.get(`/api/engine/routing/fallbacks?limit=${limit}`),
  })
}