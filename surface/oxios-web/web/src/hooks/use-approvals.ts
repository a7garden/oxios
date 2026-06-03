import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { Approval } from '@/types'

/**
 * Pending + resolved approval requests.
 *
 * The backend returns a raw array, so we normalise to `{ items }` for
 * consistent client consumption. Refetched every 5s — the queue is
 * small (usually 0–5 items) and approvals are user-driven, so the
 * polling interval is fine.
 */
export function useApprovals() {
  return useQuery({
    queryKey: ['approvals'],
    queryFn: async () => {
      const res = await api.get<Approval[]>('/api/approvals')
      return { items: Array.isArray(res) ? res : [] }
    },
    refetchInterval: 5_000,
  })
}

/** Convenience selector: just the pending approvals. */
export function usePendingApprovals() {
  const q = useApprovals()
  const items = q.data?.items ?? []
  return { ...q, items: items.filter((a) => a.status === 'pending') }
}

/**
 * Optimistic approve mutation. The approval is removed from the pending
 * list immediately, with rollback on error.
 */
export function useApproveApproval() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.post(`/api/approvals/${id}/approve`),
    onMutate: async (id: string) => {
      await qc.cancelQueries({ queryKey: ['approvals'] })
      const prev = qc.getQueryData<{ items: Approval[] }>(['approvals'])
      if (prev) {
        qc.setQueryData<{ items: Approval[] }>(['approvals'], {
          items: prev.items.map((a) => (a.id === id ? { ...a, status: 'approved' } : a)),
        })
      }
      return { prev }
    },
    onError: (_err, _id, ctx) => {
      if (ctx?.prev) {
        qc.setQueryData(['approvals'], ctx.prev)
      }
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: ['approvals'] })
    },
  })
}

/** Optimistic reject mutation. */
export function useRejectApproval() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.post(`/api/approvals/${id}/reject`),
    onMutate: async (id: string) => {
      await qc.cancelQueries({ queryKey: ['approvals'] })
      const prev = qc.getQueryData<{ items: Approval[] }>(['approvals'])
      if (prev) {
        qc.setQueryData<{ items: Approval[] }>(['approvals'], {
          items: prev.items.map((a) => (a.id === id ? { ...a, status: 'rejected' } : a)),
        })
      }
      return { prev }
    },
    onError: (_err, _id, ctx) => {
      if (ctx?.prev) {
        qc.setQueryData(['approvals'], ctx.prev)
      }
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: ['approvals'] })
    },
  })
}
