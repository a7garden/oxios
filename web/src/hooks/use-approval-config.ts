import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { useChatStore } from '@/stores/chat'
import type { ApprovalConfig, ApprovalConfigPatch, ApprovalMode, GrantBody } from '@/types/approval'

const APPROVAL_KEY = ['approval-config'] as const

export function useApprovalConfig() {
  return useQuery({
    queryKey: APPROVAL_KEY,
    queryFn: () => api.get<ApprovalConfig>('/api/security/approval'),
  })
}

export function useUpdateApprovalConfig() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (patch: ApprovalConfigPatch) =>
      api.patch<ApprovalConfig>('/api/security/approval', patch),
    onSuccess: (data) => {
      qc.setQueryData<ApprovalConfig>(APPROVAL_KEY, data)
    },
  })
}

/** Convenience mutation for the mode-only PATCH the dropdown fires. */
export function useSetApprovalMode() {
  const update = useUpdateApprovalConfig()
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (mode: ApprovalMode) => update.mutateAsync({ mode }),
    onSuccess: (data) => {
      qc.setQueryData<ApprovalConfig>(APPROVAL_KEY, data)
      // Switching to AutoRun auto-approves any in-flight tool call server-side
      // (the gate re-evaluates). Drop the now-stale approval card immediately
      // so the UI reflects the unblocked agent instead of a dead prompt.
      if (data.mode === 'auto-run') {
        useChatStore.setState({ activeToolApproval: null })
      }
    },
  })
}

export function useAddGrant() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (body: GrantBody) =>
      api.post<ApprovalConfig>('/api/security/approval/allow-list', body),
    onSuccess: (data) => qc.setQueryData<ApprovalConfig>(APPROVAL_KEY, data),
  })
}

export function useRemoveGrant() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (key: string) =>
      api.delete<ApprovalConfig>(`/api/security/approval/allow-list/${encodeURIComponent(key)}`),
    onSuccess: (data) => qc.setQueryData<ApprovalConfig>(APPROVAL_KEY, data),
  })
}
