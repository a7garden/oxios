/**
 * oxiline integration hooks (first-party app module, `timeline` cargo feature).
 *
 * Mirrors the memo/email hook shape: a status query (404-tolerant for the
 * cargo-feature-gated route) + enable/disable mutations that swap the live
 * kernel slot. oxios is a read-only co-client of the store.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ApiError, api } from '@/lib/api-client'

/** `GET /api/timeline/status` response. */
export interface TimelineStatus {
  /** Persisted config flag (`[timeline].enabled`). */
  enabled: boolean
  /** Live runtime slot state — `true` once the store is open. */
  connected: boolean
  /** Configured db path (empty = oxiline default location). */
  db_path: string
}

/**
 * Connection status. Returns `null` when `/api/timeline/status` 404s — i.e. the
 * `timeline` cargo feature is not compiled in. The 404 is permanent (no retry).
 */
export function useTimelineStatus() {
  return useQuery<TimelineStatus | null>({
    queryKey: ['timeline-status'],
    queryFn: async () => {
      try {
        return await api.get<TimelineStatus>('/api/timeline/status')
      } catch (e) {
        if (e instanceof ApiError && e.status === 404) return null
        throw e
      }
    },
    retry: (_count, error) => !(error instanceof ApiError && error.status === 404),
    refetchInterval: (query) => (query.state.data ? 10_000 : false),
  })
}

/** `POST /api/timeline/enable` — open the store + live swap (no restart). */
export function useTimelineEnable() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (dbPath: string) =>
      api.post<{ connected: boolean }>('/api/timeline/enable', { db_path: dbPath }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['timeline-status'] }),
  })
}

/** `POST /api/timeline/disable` — drop the live facade (data untouched). */
export function useTimelineDisable() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () => api.post<{ connected: boolean }>('/api/timeline/disable'),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['timeline-status'] }),
  })
}
