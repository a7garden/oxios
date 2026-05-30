import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { AgentGroup, AgentGroupProgress } from '@/types/agent-group'

export function useAgentGroups() {
  return useQuery({
    queryKey: ['agent-groups'],
    queryFn: async () => {
      const res = await api.get<AgentGroup[]>('/api/agent-groups')
      return Array.isArray(res) ? res : []
    },
    refetchInterval: 5000,
  })
}

export function useAgentGroupDetail(id: string) {
  return useQuery({
    queryKey: ['agent-groups', id],
    queryFn: () => api.get<AgentGroup>(`/api/agent-groups/${id}`),
    enabled: !!id,
  })
}

export function useAgentGroupProgress(id: string) {
  return useQuery({
    queryKey: ['agent-groups', id, 'progress'],
    queryFn: () => api.get<AgentGroupProgress>(`/api/agent-groups/${id}/progress`),
    enabled: !!id,
    refetchInterval: 5000,
  })
}
