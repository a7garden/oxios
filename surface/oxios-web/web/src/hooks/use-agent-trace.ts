import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { AgentDetail, AgentTrace, AgentLogs } from '@/types/agent'

// ── Agent detail ──
export function useAgentDetail(id: string | null) {
  return useQuery({
    queryKey: ['agents', 'detail', id],
    queryFn: () => api.get<AgentDetail>(`/api/agents/${id}`),
    enabled: !!id,
    refetchInterval: 10_000,
  })
}

// ── Agent trace ──
export function useAgentTrace(id: string | null) {
  return useQuery({
    queryKey: ['agents', 'trace', id],
    queryFn: () => api.get<AgentTrace>(`/api/agents/${id}/trace`),
    enabled: !!id,
    refetchInterval: (query) => {
      // Poll every 5s if trace is incomplete (no completed_at)
      const data = query.state.data as AgentTrace | undefined
      return data?.completed_at ? false : 5_000
    },
  })
}

// ── Agent logs ──
export function useAgentLogs(id: string | null) {
  return useQuery({
    queryKey: ['agents', 'logs', id],
    queryFn: () => api.get<AgentLogs>(`/api/agents/${id}/logs`),
    enabled: !!id,
    refetchInterval: 5_000,
  })
}

// ── Seed agents ──
export function useSeedAgents(seedId: string | null) {
  return useQuery({
    queryKey: ['seeds', 'agents', seedId],
    queryFn: () => api.get<{ agents: { id: string; name: string; status: string; steps_completed: number; created_at: string }[] }>(`/api/seeds/${seedId}/agents`),
    enabled: !!seedId,
    refetchInterval: 10_000,
  })
}
