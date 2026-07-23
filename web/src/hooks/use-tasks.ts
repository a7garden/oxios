// use-tasks — React Query hooks for task API (RFC-043)

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type {
  CreateTaskParams,
  ListTasksParams,
  SetScheduleParams,
  SetVerifyParams,
  Task,
  TaskStatus,
} from '@/types/task'

// ── List ──

export function useTasks(params?: ListTasksParams) {
  const query = new URLSearchParams()
  if (params?.statuses?.length) query.set('statuses', params.statuses.join(','))
  if (params?.assigneeAgentId) query.set('assignee', params.assigneeAgentId)
  if (params?.parentTaskId) query.set('parent', params.parentTaskId)
  if (params?.limit) query.set('limit', String(params.limit))
  if (params?.offset) query.set('offset', String(params.offset))

  const qs = query.toString()
  return useQuery({
    queryKey: ['tasks', qs],
    queryFn: () => api.get<{ tasks: Task[]; count: number }>(`/api/tasks${qs ? `?${qs}` : ''}`),
  })
}

// ── Get ──

export function useTask(id: string | null) {
  return useQuery({
    queryKey: ['task', id],
    queryFn: () => api.get<Task>(`/api/tasks/${id}`),
    enabled: !!id,
  })
}

// ── Create ──

export function useCreateTask() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (params: CreateTaskParams) => api.post<Task>('/api/tasks', params),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tasks'] }),
  })
}

// ── Delete ──

export function useDeleteTask() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.delete(`/api/tasks/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tasks'] }),
  })
}

// ── Update status ──

export function useUpdateTaskStatus() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, status }: { id: string; status: TaskStatus }) =>
      api.put(`/api/tasks/${id}/status`, { status }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tasks'] }),
  })
}

// ── Set schedule ──

export function useSetTaskSchedule() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, ...params }: { id: string } & SetScheduleParams) =>
      api.put(`/api/tasks/${id}/schedule`, params),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tasks'] }),
  })
}

// ── Set verify ──

export function useSetTaskVerify() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, ...params }: { id: string } & SetVerifyParams) =>
      api.put(`/api/tasks/${id}/verify`, params),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tasks'] }),
  })
}

// ── Run task ──

export function useRunTask() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, prompt }: { id: string; prompt?: string }) =>
      api.post(`/api/tasks/${id}/run`, { prompt }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['tasks'] }),
  })
}
