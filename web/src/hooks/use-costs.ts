import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type {
  CostPeriod,
  CostSummary,
  DailyCostRow,
  ModelCostRow,
  ProjectCostRow,
  QuotaSnapshot,
  SpendLimit,
} from '@/types/cost'

export function useCostSummary(period: CostPeriod = 'all') {
  return useQuery({
    queryKey: ['costs', 'summary', period],
    queryFn: () => api.get<CostSummary>(`/api/costs/summary?period=${period}`),
    refetchInterval: 30000,
  })
}

export function useCostByModel(period: CostPeriod = 'all') {
  return useQuery({
    queryKey: ['costs', 'by-model', period],
    queryFn: () =>
      api.get<{ items: ModelCostRow[] }>(`/api/costs/by-model?period=${period}`),
    refetchInterval: 30000,
  })
}

export function useCostByProject(period: CostPeriod = 'all') {
  return useQuery({
    queryKey: ['costs', 'by-project', period],
    queryFn: () =>
      api.get<{ items: ProjectCostRow[] }>(
        `/api/costs/by-project?period=${period}`,
      ),
    refetchInterval: 30000,
  })
}

export function useCostDaily(days = 30) {
  return useQuery({
    queryKey: ['costs', 'daily', days],
    queryFn: () =>
      api.get<{ items: DailyCostRow[] }>(`/api/costs/daily?days=${days}`),
    refetchInterval: 60000,
  })
}

export function useProviderQuotas() {
  return useQuery({
    queryKey: ['costs', 'providers'],
    queryFn: () =>
      api.get<{ providers: QuotaSnapshot[] }>(`/api/costs/providers`),
    refetchInterval: 120000,
    retry: false, // external API calls may fail; don't hammer
  })
}

export function useSpendLimit() {
  return useQuery({
    queryKey: ['costs', 'spend-limit'],
    queryFn: () => api.get<SpendLimit>('/api/costs/spend-limit'),
    refetchInterval: 30000,
  })
}

export function useSetSpendLimit() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (monthly_limit_usd: number | null) =>
      api.put('/api/costs/spend-limit', { monthly_limit_usd }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['costs', 'spend-limit'] })
      qc.invalidateQueries({ queryKey: ['costs', 'summary'] })
    },
  })
}
