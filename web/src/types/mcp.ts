export interface McpServer {
  name: string
  command: string
  args: string[]
  env?: Record<string, string>
  enabled: boolean
  initialized: boolean
}

export interface McpTool {
  name: string
  description: string
  server: string
  arguments: Array<{
    name: string
    description?: string
    required?: boolean
    type?: string
  }>
}

export interface McpToolCallResult {
  content: Array<{
    type: string
    text?: string
    [key: string]: unknown
  }>
  is_error: boolean
}

export interface McpToolCallRequest {
  server: string
  tool: string
  arguments: Record<string, unknown>
}

export interface McpServerRegisterRequest {
  name: string
  command: string
  args?: string[]
  env?: Record<string, string>
}

export interface McpServerUpdateRequest {
  command: string
  args?: string[]
  env?: Record<string, string>
  enabled?: boolean
}
