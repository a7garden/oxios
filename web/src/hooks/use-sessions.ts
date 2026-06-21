import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { Session } from '@/types'

// ─── Types ────────────────────────────────────────────────────

export interface MoveSessionInput {
  /** Target Project ID, or null to unassign. */
  project_id: string | null
}

// ─── Hooks ────────────────────────────────────────────────────

/** List all sessions. */
export function useSessions() {
  return useQuery({
    queryKey: ['sessions'],
    queryFn: () => api.get<{ items: Session[]; total: number }>('/api/sessions'),
    refetchInterval: 10_000,
  })
}

/** RFC-025: Move a session to a different Project (drag-to-reparent). */
export function useMoveSession() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ sessionId, ...input }: MoveSessionInput & { sessionId: string }) =>
      api.patch<{ status: string; id: string; project_id: string | null }>(
        `/api/sessions/${sessionId}/project`,
        input,
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['sessions'] })
    },
  })
}
