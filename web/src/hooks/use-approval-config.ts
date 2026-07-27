import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
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
