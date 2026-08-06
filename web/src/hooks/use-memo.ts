/**
 * oximemo integration hooks (first-party app module, `memo` cargo feature).
 *
 * Mirrors the email hook shape: a status query (404-tolerant for the
 * cargo-feature-gated route) + enable/disable mutations that swap the live
 * kernel slot. oxios is a co-client of the vault — disable never touches data.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ApiError, api } from '@/lib/api-client'

/** `GET /api/memo/status` response. */
export interface MemoStatus {
  /** Persisted config flag (`[memo].enabled`). */
  enabled: boolean
  /** Live runtime slot state — `true` once the vault is open. */
  connected: boolean
  /** Configured vault path (empty = oximemo default location). */
  vault_path: string
}

/**
 * Connection status. Returns `null` when `/api/memo/status` 404s — i.e. the
 * `memo` cargo feature is not compiled in. The 404 is permanent (no retry) so
 * it never spams the console; the card renders a "not compiled" notice.
 */
export function useMemoStatus() {
  return useQuery<MemoStatus | null>({
    queryKey: ['memo-status'],
    queryFn: async () => {
      try {
        return await api.get<MemoStatus>('/api/memo/status')
      } catch (e) {
        if (e instanceof ApiError && e.status === 404) return null
        throw e
      }
    },
    retry: (_count, error) => !(error instanceof ApiError && error.status === 404),
    // Poll only while compiled in (data !== null); stop once we know it's absent.
    refetchInterval: (query) => (query.state.data ? 10_000 : false),
  })
}

/** `POST /api/memo/enable` — open the vault + live swap (no restart). */
export function useMemoEnable() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (vaultPath: string) =>
      api.post<{ connected: boolean }>('/api/memo/enable', { vault_path: vaultPath }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['memo-status'] }),
  })
}

/** `POST /api/memo/disable` — drop the live facade (data untouched). */
export function useMemoDisable() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () => api.post<{ connected: boolean }>('/api/memo/disable'),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['memo-status'] }),
  })
}
