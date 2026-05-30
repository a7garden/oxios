import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { A2AAgentCard, A2AMessage, A2ATopology } from '@/types/a2a'

export function useA2AAgents() {
  return useQuery({
    queryKey: ['a2a', 'agents'],
    queryFn: async () => {
      const res = await api.get<{ agents: A2AAgentCard[] }>('/api/a2a/agents')
      return res.agents ?? []
    },
    refetchInterval: 10000,
  })
}

export function useA2AMessages() {
  return useQuery({
    queryKey: ['a2a', 'messages'],
    queryFn: async () => {
      const res = await api.get<{ messages: A2AMessage[] }>('/api/a2a/messages')
      return res.messages ?? []
    },
    refetchInterval: 10000,
  })
}

export function useA2ATopology() {
  return useQuery({
    queryKey: ['a2a', 'topology'],
    queryFn: () => api.get<A2ATopology>('/api/a2a/topology'),
    refetchInterval: 10000,
  })
}
