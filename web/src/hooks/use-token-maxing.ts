import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type {
  MaxerStatus,
  StartRequest,
  StartResponse,
  StopResponse,
  TokenMaxingProvidersResponse,
  TokenMaxingSession,
} from '@/types/token-maxing'

/** Live status — polled aggressively while a session is running.
 *  The backend's `MaxerStatus.running` flag decides polling cadence via
 *  `refetchInterval`; the panel uses this single hook.
 */
export function useTokenMaxingStatus() {
  return useQuery({
    queryKey: ['token-maxing', 'status'],
    queryFn: () => api.get<MaxerStatus>('/api/token-maxing/status'),
    refetchInterval: (query) => {
      const running = query.state.data?.running ?? false
      return running ? 5000 : 15000
    },
  })
}

/** Eligibility + per-provider availability verdict (RFC-031 §4). */
export function useTokenMaxingProviders() {
  return useQuery({
    queryKey: ['token-maxing', 'providers'],
    queryFn: () => api.get<TokenMaxingProvidersResponse>('/api/token-maxing/providers'),
    refetchInterval: 10000,
  })
}

/** Past sessions (most-recent last). */
export function useTokenMaxingSessions() {
  return useQuery({
    queryKey: ['token-maxing', 'sessions'],
    queryFn: () => api.get<TokenMaxingSession[]>('/api/token-maxing/sessions'),
    refetchInterval: 10000,
  })
}

/** One session's full report. */
export function useTokenMaxingSession(id: string | null) {
  return useQuery({
    queryKey: ['token-maxing', 'session', id],
    queryFn: () => api.get<TokenMaxingSession>(`/api/token-maxing/sessions/${id}`),
    enabled: !!id,
    // Reports are immutable — once fetched, never refetch.
    refetchInterval: false,
    staleTime: Infinity,
  })
}

/** Launch a session. Body is `{ window }` for scheduled runs, `{ manual: true }` for manual. */
export function useTokenMaxingStart() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (body: StartRequest) => api.post<StartResponse>('/api/token-maxing/start', body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['token-maxing'] })
    },
  })
}

/** Graceful stop after the in-flight task. */
export function useTokenMaxingStop() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () => api.post<StopResponse>('/api/token-maxing/stop'),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['token-maxing'] })
    },
  })
}
