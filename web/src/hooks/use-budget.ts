import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { BudgetListResponse, SetBudgetRequest } from '@/types/budget'

export function useBudgetList() {
  return useQuery({
    queryKey: ['budgets'],
    queryFn: () => api.get<BudgetListResponse>('/api/budget'),
    refetchInterval: 10000,
  })
}

export function useBudgetSet() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ agentId, ...body }: { agentId: string } & SetBudgetRequest) =>
      api.post(`/api/budget/${agentId}`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['budgets'] }),
  })
}

export function useBudgetDelete() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (agentId: string) => api.delete(`/api/budget/${agentId}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['budgets'] }),
  })
}

export function useBudgetReset() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (agentId: string) => api.post(`/api/budget/${agentId}/reset`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['budgets'] }),
  })
}
