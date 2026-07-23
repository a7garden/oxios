// useCategorizedMemory — API hook for 5-category memory system
// Fetches and manages memories across identity/activity/context/experience/preference.

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { CategorizedMemory, MemoryCategory } from '@/types/memory-categories'

// ── Types ──

interface CategorizedMemoryResponse {
  memories: CategorizedMemory[]
}

interface CreateMemoryParams {
  category: MemoryCategory
  [key: string]: unknown
}

// ── Hooks ──

/** Fetch all categorized memories, optionally filtered by category. */
export function useCategorizedMemories(category?: MemoryCategory | null) {
  return useQuery({
    queryKey: ['categorized-memory', category ?? 'all'],
    queryFn: () =>
      api.get<CategorizedMemoryResponse>(
        `/api/memory/categorized${category ? `?category=${category}` : ''}`,
      ),
    // Graceful fallback: if the API doesn't exist yet, return empty
    retry: false,
  })
}

/** Create a new categorized memory. */
export function useCreateCategorizedMemory() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (params: CreateMemoryParams) =>
      api.post<CategorizedMemory>('/api/memory/categorized', params),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['categorized-memory'] })
    },
  })
}

/** Delete a categorized memory. */
export function useDeleteCategorizedMemory() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.delete(`/api/memory/categorized/${id}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['categorized-memory'] })
    },
  })
}

/** Get memory persona summary. */
export function useMemoryPersona() {
  return useQuery({
    queryKey: ['memory-persona'],
    queryFn: () =>
      api.get<{ summary: string; updatedAt: string; keyFacts?: string[] }>('/api/memory/persona'),
    retry: false,
  })
}
