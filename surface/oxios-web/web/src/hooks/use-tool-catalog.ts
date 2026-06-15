import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'

export interface ToolMeta {
  name: string
  description_key: string
  category: 'fs' | 'exec' | 'memory' | 'comms' | 'system' | 'a2a'
}

/** Fetch the tool catalog from the backend. */
export function useToolCatalog() {
  return useQuery({
    queryKey: ['tools', 'registry'],
    queryFn: async () => {
      const res = await api.get<{ tools: ToolMeta[] }>('/api/tools/registry')
      return res.tools
    },
    staleTime: 5 * 60 * 1000,
    retry: 1,
  })
}
