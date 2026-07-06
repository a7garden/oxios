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

/** Walks two JSON values in parallel and yields a flat list of differences.
 *
 * Only iterates keys from `after` (the proposed payload), not `before` (the
 * current config). This matches PATCH semantics: keys absent from the
 * payload are "no change" — the server deep-merges and preserves them.
 *
 * Previously, this function iterated keys from **both** sides, which caused
 * every section not present in the form (browser, budget, calendar, …) to
 * appear as a phantom "deleted" change, since `buildPayload()` only
 * includes sections with form fields. This resulted in 60+ false-positive
 * diff entries requiring a daemon restart. */
export function diffConfigs(
  before: Record<string, unknown>,
  after: Record<string, unknown>,
  prefix = '',
): ConfigDiffEntry[] {
  const out: ConfigDiffEntry[] = []
  // Only iterate `after` keys — missing keys in `after` mean "no change"
  // under PATCH semantics, so they must not appear in the diff.
  const keys = Object.keys(after ?? {})
  for (const k of keys) {
    const path = prefix ? `${prefix}.${k}` : k
    const b = before?.[k]
    const a = after?.[k]
    if (
      a !== null &&
      typeof a === 'object' &&
      !Array.isArray(a) &&
      b !== null &&
      typeof b === 'object' &&
      !Array.isArray(b)
    ) {
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
    return ka.every((k) =>
      deepEqual((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]),
    )
  }
  return false
}

export function useConfig() {
  return useQuery({
    queryKey: ['config'],
    queryFn: () => api.get<Record<string, unknown>>('/api/config'),
  })
}

/** Backend hot-reload classification metadata (single source of truth). */
export interface ConfigMeta {
  /** Top-level section keys whose fields are hot-reloadable. */
  hot_reloadable_sections: string[]
  /** Dotted field paths that always require restart, even in hot-reloadable sections. */
  always_restart_fields: string[]
}

/**
 * Fetches the backend's authoritative hot-reload classification.
 *
 * This is the **single source of truth** for the frontend's pre-save
 * Diff Preview badges and SaveDock counts. The previous approach
 * maintained a parallel `hotReload` boolean per field in `field-defs.ts`
 * that silently drifted from the backend's actual propagation logic.
 */
export function useConfigMeta() {
  return useQuery({
    queryKey: ['config-meta'],
    queryFn: () => api.get<ConfigMeta>('/api/config/meta'),
    staleTime: Infinity, // classification changes only on daemon restart
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
 * Save the config via PATCH. Falls back to PUT only if the route
 * configuration on the server is misconfigured (PATCH is 405'd and
 * removed). Today's server implements both PATCH and PUT with the
 * same deep-merge semantics, so a successful PATCH is the normal
 * path and the PUT fallback is purely a defensive retry.
 */
export function useSaveConfig() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (next: Record<string, unknown>) => {
      try {
        return await api.patch<ConfigPatchResponse>('/api/config', next)
      } catch (_err) {
        // Defensive fallback: if PATCH is removed entirely (405/404
        // on a misconfigured server) we retry with PUT, which is
        // currently an alias for PATCH with identical semantics.
        return await api.put<Record<string, unknown>>('/api/config', next)
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] })
    },
  })
}
