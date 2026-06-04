import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type {
  MemoryStats,
  MemoryDetail,
  DreamReport,
  DreamStatus,
  SemanticSearchResult,
  MemoryMapResponse,
} from '@/types/memory'

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

// ── Map (RFC-T1-B) ──
export interface MemoryMapFilters {
  tier?: string
  mem_type?: string
  limit?: number
}

/**
 * Fetch 2D coordinates + top neighbors for the memory map.
 *
 * The backend caches per (5-minute epoch, id-set) and falls back to
 * a recompute if either changes, so we keep `staleTime` short but
 * accept the cache hit on the server.
 */
export function useMemoryMap(filters: MemoryMapFilters = {}) {
  const params: Record<string, string> = {}
  if (filters.tier) params.tier = filters.tier
  if (filters.mem_type) params.mem_type = filters.mem_type
  if (filters.limit) params.limit = String(filters.limit)
  return useQuery({
    queryKey: ['memory', 'map', filters.tier ?? '', filters.mem_type ?? '', filters.limit ?? 500],
    queryFn: () => api.get<MemoryMapResponse>('/api/memory/map', params),
    staleTime: 30_000,
  })
}
