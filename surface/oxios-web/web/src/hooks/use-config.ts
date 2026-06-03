import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'

/** A single dot-path change between two config snapshots. */
export interface ConfigDiffEntry {
  path: string
  before: unknown
  after: unknown
  /** Classified by the backend as hot-reloadable. */
  hotReload: boolean
  /** Optional restart scope. */
  scope?: string
}

export interface ConfigPatchResponse {
  config: Record<string, unknown>
  hot_reload: {
    applied_immediately: string[]
    requires_restart: string[]
    total_changed: number
  }
}

/** Walks two JSON values in parallel and yields a flat list of differences. */
export function diffConfigs(before: Record<string, unknown>, after: Record<string, unknown>, prefix = ''): ConfigDiffEntry[] {
  const out: ConfigDiffEntry[] = []
  const keys = new Set([...Object.keys(before ?? {}), ...Object.keys(after ?? {})])
  for (const k of keys) {
    const path = prefix ? `${prefix}.${k}` : k
    const b = before?.[k]
    const a = after?.[k]
    if (a !== null && typeof a === 'object' && !Array.isArray(a) && b !== null && typeof b === 'object' && !Array.isArray(b)) {
      out.push(...diffConfigs(b as Record<string, unknown>, a as Record<string, unknown>, path))
    } else if (!deepEqual(b, a)) {
      // Default: assume hot-reloadable. Backend re-classifies authoritatively.
      out.push({ path, before: b, after: a, hotReload: true })
    }
  }
  return out
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true
  if (typeof a !== typeof b) return false
  if (a === null || b === null) return a === b
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false
    return a.every((v, i) => deepEqual(v, b[i]))
  }
  if (typeof a === 'object' && typeof b === 'object') {
    const ka = Object.keys(a as object)
    const kb = Object.keys(b as object)
    if (ka.length !== kb.length) return false
    return ka.every((k) => deepEqual((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]))
  }
  return false
}

export function useConfig() {
  return useQuery({
    queryKey: ['config'],
    queryFn: () => api.get<Record<string, unknown>>('/api/config'),
  })
}

export interface SaveConfigOpts {
  /** Local representation of the new config (used for optimistic diffing). */
  nextConfig: Record<string, unknown>
  /** Last-known server config. */
  currentConfig: Record<string, unknown>
  /** Optional pre-computed diff (skips client-side diffing). */
  precomputedDiff?: ConfigDiffEntry[]
}

/**
 * Save the config via PATCH. Falls back to PUT if the server is too old
 * to support PATCH (returns 404/405).
 */
export function useSaveConfig() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (next: Record<string, unknown>) => {
      try {
        return await api.patch<ConfigPatchResponse>('/api/config', next)
      } catch (err) {
        // Fallback for older servers without PATCH support.
        return await api.put<Record<string, unknown>>('/api/config', next)
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] })
    },
  })
}
