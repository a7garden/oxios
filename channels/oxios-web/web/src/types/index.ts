// Agent
export interface Agent {
  id: string
  name: string
  status: 'running' | 'idle' | 'stopped' | 'error'
  seed_id?: string
  space_id?: string
  started_at?: string
  metadata?: Record<string, unknown>
}

export interface AgentListResponse {
  items: Agent[]
  total: number
  page: number
  limit: number
}

// Session
export interface Session {
  id: string
  agent_id?: string
  space_id?: string
  created_at: string
  updated_at?: string
  message_count?: number
  metadata?: Record<string, unknown>
}

// Seed
export interface Seed {
  id: string
  name: string
  spec: Record<string, unknown>
  phase: 'interview' | 'seed' | 'execute' | 'evaluate' | 'evolve'
  created_at: string
  updated_at?: string
  evolution_log?: EvolutionEntry[]
}

export interface EvolutionEntry {
  phase: string
  timestamp: string
  summary: string
  changes?: Record<string, unknown>
}

// Space
export interface Space {
  id: string
  name: string
  tag?: string
  status: 'active' | 'archived'
  created_at: string
  metadata?: Record<string, unknown>
}

// Program
export interface Program {
  name: string
  enabled: boolean
  version?: string
  description?: string
  host_requirements?: string[]
}

// Skill
export interface Skill {
  name: string
  description?: string
  content: string
}

// Memory
export interface MemoryEntry {
  name: string
  content: string
  created_at?: string
  updated_at?: string
}

// Config
export interface OxiosConfig {
  general?: {
    default_model?: string
    max_concurrent_agents?: number
    workspace_path?: string
    [key: string]: unknown
  }
  engine?: {
    provider?: string
    model?: string
    api_key?: string
    base_url?: string
    [key: string]: unknown
  }
  [key: string]: unknown
}

// Chat
export interface ChatMessage {
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp?: string
}

export interface ChatRequest {
  message: string
  session_id?: string
  space_id?: string
}

export interface ChatResponse {
  response: string
  session_id: string
  space_id?: string
  seed_id?: string
  agent_id?: string
  phase_reached?: string
  evaluation_passed?: boolean
  exit_code?: number
  duration_ms?: number
}

export interface StreamChunk {
  type: 'token' | 'tool_call' | 'tool_result' | 'done' | 'error'
  content?: string
  tool_name?: string
  tool_args?: Record<string, unknown>
  tool_result?: unknown
  error?: string
}

// Event
export interface OxiosEvent {
  id: string
  type: string
  agent_id?: string
  session_id?: string
  timestamp: string
  data?: Record<string, unknown>
}

// Approval
export interface Approval {
  id: string
  agent_id: string
  type: string
  description: string
  created_at: string
  status: 'pending' | 'approved' | 'rejected'
}

// Cron Job
export interface CronJob {
  id: string
  name: string
  schedule: string
  command: string
  enabled: boolean
  last_run?: string
  next_run?: string
}

// Budget
export interface Budget {
  agent_id: string
  tokens_used?: number
  tokens_limit?: number
  cost_used?: number
  cost_limit?: number
}

// Resource
export interface ResourceSnapshot {
  timestamp: string
  cpu_percent: number
  memory_percent: number
  disk_percent: number
}

// Audit
export interface AuditEntry {
  id: string
  agent_id?: string
  action: string
  resource?: string
  timestamp: string
  details?: Record<string, unknown>
  hash?: string
}

// Git
export interface GitCommit {
  hash: string
  message: string
  author: string
  timestamp: string
}

// Persona
export interface Persona {
  id: string
  name: string
  description?: string
  system_prompt?: string
  active: boolean
}

// Agent Group
export interface AgentGroup {
  id: string
  name: string
  agents: string[]
  strategy?: string
}

// Workspace
export interface FileNode {
  name: string
  path: string
  type: 'file' | 'directory'
  children?: FileNode[]
  size?: number
  modified?: string
}

// Paginated response
export interface PaginatedResponse<T> {
  items: T[]
  total: number
  page: number
  limit: number
}

// Status
export interface SystemStatus {
  version: string
  uptime_ms: number
  agents_running: number
  agents_total: number
  spaces_active: number
  memory_usage_mb?: number
}