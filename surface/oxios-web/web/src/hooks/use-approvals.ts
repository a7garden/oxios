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
  const items = Array.isArray(q.data?.items) ? q.data.items : []
  return { ...q, items: items.filter((a) => a.status === 'pending') }
}

/** Literal union for the approval status field. */
type ApprovalStatus = Approval['status']

/**
 * Build an optimistic-update context for an approve/reject mutation.
 *
 * Snapshots the CURRENT list (so a later rollback can restore THIS
 * item to its pre-mutation status), applies the optimistic change via
 * the functional `setQueryData` form (so concurrent mutations don't
 * clobber each other), and returns the per-item snapshot for the
 * rollback path.
 */
async function optimisticApprovalUpdate(
  qc: ReturnType<typeof useQueryClient>,
  id: string,
  nextStatus: ApprovalStatus,
): Promise<{ prevItem: Approval | undefined }> {
  await qc.cancelQueries({ queryKey: ['approvals'] })
  const prev = qc.getQueryData<{ items: Approval[] }>(['approvals'])
  const prevItem = prev?.items.find((a) => a.id === id)
  qc.setQueryData<{ items: Approval[] }>(['approvals'], (old) => {
    if (!old) return old
    return {
      items: old.items.map((a) => (a.id === id ? { ...a, status: nextStatus } : a)),
    }
  })
  return { prevItem }
}

/**
 * Roll back a single item's optimistic change, leaving any concurrent
 * mutations' optimistic state intact. The bug being fixed: when two
 * approvals were clicked rapidly, A's `onError` would overwrite the
 * ENTIRE list with its pre-A snapshot, clobbering B's optimistic
 * state. Here we only touch the item A modified.
 */
function rollbackApprovalItem(
  qc: ReturnType<typeof useQueryClient>,
  id: string,
  prevItem: Approval | undefined,
) {
  if (!prevItem) return
  qc.setQueryData<{ items: Approval[] }>(['approvals'], (old) => {
    if (!old) return old
    return {
      items: old.items.map((a) => (a.id === id ? prevItem : a)),
    }
  })
}

/**
 * Optimistic approve mutation. The approval is removed from the pending
 * list immediately, with rollback on error.
 */
export function useApproveApproval() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.post(`/api/approvals/${id}/approve`),
    onMutate: async (id: string) => optimisticApprovalUpdate(qc, id, 'approved'),
    onError: (_err, id, ctx) => {
      rollbackApprovalItem(qc, id, ctx?.prevItem)
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
    onMutate: async (id: string) => optimisticApprovalUpdate(qc, id, 'rejected'),
    onError: (_err, id, ctx) => {
      rollbackApprovalItem(qc, id, ctx?.prevItem)
    },
    onSettled: () => {
      qc.invalidateQueries({ queryKey: ['approvals'] })
    },
  })
}
