import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { MemoryStats, MemoryDetail, DreamReport, DreamStatus, SemanticSearchResult } from '@/types/memory'

// ── Stats ──
export function useMemoryStats() {
  return useQuery({
    queryKey: ['memory', 'stats'],
    queryFn: () => api.get<MemoryStats>('/api/memory/stats'),
    staleTime: 30_000,
  })
}

// ── List by tier ──
export function useMemoryList(tier?: string, type?: string) {
  const params: Record<string, string> = {}
  if (tier) params.tier = tier
  if (type) params.type = type
  return useQuery({
    queryKey: ['memory', 'list', tier ?? '', type ?? ''],
    queryFn: () => api.get<{ items: MemoryDetail[] }>('/api/memory', params),
  })
}

// ── Detail ──
export function useMemoryDetail(id: string | null) {
  return useQuery({
    queryKey: ['memory', 'detail', id],
    queryFn: () => api.get<MemoryDetail>(`/api/memory/${id}`),
    enabled: !!id,
  })
}

// ── Pin toggle ──
export function useMemoryPin() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, pinned }: { id: string; pinned: boolean }) =>
      api.put(`/api/memory/${id}/pin`, { pinned }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['memory'] })
    },
  })
}

// ── Tier change ──
export function useMemoryTierChange() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, tier }: { id: string; tier: string }) =>
      api.put(`/api/memory/${id}/tier`, { tier }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['memory'] })
    },
  })
}

// ── Delete ──
export function useMemoryDelete() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.delete(`/api/memory/${id}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['memory'] })
    },
  })
}

// ── Dream reports ──
export function useDreamReports() {
  return useQuery({
    queryKey: ['memory', 'dream', 'reports'],
    queryFn: () => api.get<DreamReport[]>('/api/memory/dream/reports'),
    staleTime: 60_000,
  })
}

// ── Dream status ──
export function useDreamStatus() {
  return useQuery({
    queryKey: ['memory', 'dream', 'status'],
    queryFn: () => api.get<DreamStatus>('/api/memory/dream/status'),
    staleTime: 30_000,
  })
}

// ── Semantic search ──
export function useMemorySemanticSearch() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ query, type, limit }: { query: string; type?: string; limit?: number }) =>
      api.post<{ count: number; entries: SemanticSearchResult[]; engine: string }>('/api/memory/semantic', {
        query,
        memory_type: type,
        limit,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['memory', 'search'] })
    },
  })
}
