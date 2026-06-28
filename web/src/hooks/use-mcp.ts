import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type {
  McpServer,
  McpServerRegisterRequest,
  McpServerUpdateRequest,
  McpTool,
  McpToolCallRequest,
  McpToolCallResult,
} from '@/types/mcp'

export function useMcpServers() {
  return useQuery({
    queryKey: ['mcp-servers'],
    queryFn: async () => {
      const res = await api.get<McpServer[]>('/api/mcp/servers')
      return Array.isArray(res) ? res : []
    },
    refetchInterval: 10000,
  })
}

export function useMcpTools() {
  return useQuery({
    queryKey: ['mcp-tools'],
    queryFn: async () => {
      const res = await api.get<McpTool[]>('/api/mcp/tools')
      return Array.isArray(res) ? res : []
    },
    refetchInterval: 30000,
  })
}

export function useMcpRegisterServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (req: McpServerRegisterRequest) => {
      return api.post<{ status: string; name: string }>('/api/mcp/servers', req)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['mcp-servers'] })
      queryClient.invalidateQueries({ queryKey: ['mcp-tools'] })
    },
  })
}

export function useMcpDeleteServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (name: string) => {
      return api.delete(`/api/mcp/servers/${encodeURIComponent(name)}`)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['mcp-servers'] })
      queryClient.invalidateQueries({ queryKey: ['mcp-tools'] })
    },
  })
}

export function useMcpToggleServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (name: string) => {
      return api.post<{ status: string; name: string; enabled: boolean }>(
        `/api/mcp/servers/${encodeURIComponent(name)}/toggle`,
      )
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['mcp-servers'] })
    },
  })
}

export function useMcpRefreshServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (name: string) => {
      return api.post(`/api/mcp/servers/${encodeURIComponent(name)}/refresh`)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['mcp-servers'] })
      queryClient.invalidateQueries({ queryKey: ['mcp-tools'] })
    },
  })
}

export function useMcpCallTool() {
  return useMutation({
    mutationFn: async (req: McpToolCallRequest) => {
      return api.post<McpToolCallResult>('/api/mcp/tools', req)
    },
  })
}

export function useMcpUpdateServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ name, body }: { name: string; body: McpServerUpdateRequest }) => {
      return api.put<{ status: string; name: string }>(
        `/api/mcp/servers/${encodeURIComponent(name)}`,
        body,
      )
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['mcp-servers'] })
      queryClient.invalidateQueries({ queryKey: ['mcp-tools'] })
    },
  })
}
