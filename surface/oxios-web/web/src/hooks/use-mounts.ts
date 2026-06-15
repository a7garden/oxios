import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { Mount } from '@/types'

// ─── Types ────────────────────────────────────────────────────

export interface CreateMountInput {
  name: string
  paths: string[]
}

export interface UpdateMountInput {
  name?: string
}

// ─── Hooks ────────────────────────────────────────────────────

/** List all Mounts with optional search. */
export function useMounts(search?: string) {
  return useQuery({
    queryKey: ['mounts', search],
    queryFn: () => {
      const url = search ? `/api/mounts?search=${encodeURIComponent(search)}` : '/api/mounts'
      return api.get<{ items: Mount[]; total: number }>(url)
    },
  })
}

/** Get a single Mount by ID. */
export function useMount(id: string | null) {
  return useQuery({
    queryKey: ['mount', id],
    queryFn: () => api.get<Mount>(`/api/mounts/${id}`),
    enabled: !!id,
  })
}

/** Create a new Mount (minimal RFC-025 input: name + paths). */
export function useCreateMount() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: CreateMountInput) => api.post<Mount>('/api/mounts', input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['mounts'] })
    },
  })
}

/** Update a Mount (rename). */
export function useUpdateMount() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, ...input }: UpdateMountInput & { id: string }) =>
      api.put<Mount>(`/api/mounts/${id}`, input),
    onSuccess: (_, vars) => {
      qc.invalidateQueries({ queryKey: ['mounts'] })
      qc.invalidateQueries({ queryKey: ['mount', vars.id] })
    },
  })
}

/** Delete a Mount. */
export function useDeleteMount() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.delete(`/api/mounts/${id}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['mounts'] })
    },
  })
}

/** Rescan a Mount — re-seed auto_meta from the filesystem (RFC-025). */
export function useRescanMount() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.post<Mount>(`/api/mounts/${id}/rescan`, {}),
    onSuccess: (_, id) => {
      qc.invalidateQueries({ queryKey: ['mounts'] })
      qc.invalidateQueries({ queryKey: ['mount', id] })
    },
  })
}
