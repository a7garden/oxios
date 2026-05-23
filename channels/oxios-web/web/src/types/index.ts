// Agent
export interface Agent {
  id: string
  name: string
  status: string // Backend sends Debug format: "Running", "Idle", "Stopped", "Starting", "Error"
  created_at?: string
  seed_id?: string
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
  user_id?: string
  active_seed_id?: string
  created_at: string
  updated_at?: string
  message_count?: number
  metadata?: Record<string, unknown>
}

// Session detail (from GET /api/sessions/:id)
export interface SessionDetail {
  id: string
  user_id: string
  user_messages: string[]
  agent_responses: { content: string; session_id: string; seed_id: string; phase_reached: string; evaluation_passed: boolean; timestamp: string }[]
  active_seed_id?: string
  active_persona_id?: string
  created_at: string
  updated_at: string
  metadata?: Record<string, unknown>
}

// Seed
export interface Seed {
  id: string
  goal: string
  constraints_count: number
  created_at: string
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
  source?: string
  paths?: string[]
  tags?: string[]
  active?: boolean
  created_at: string
  last_active_at?: string
  interaction_count?: number
  memory_visible?: boolean
  metadata?: Record<string, unknown>
}

// Program
export interface Program {
  name: string
  enabled: boolean
  version?: string
  description?: string
  author?: string
  tools_count?: number
  has_skill_content?: boolean
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
  category?: string
  content?: string
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

// Event (SSE)
export interface OxiosEvent {
  id?: string
  type: string
  agent_id?: string
  session_id?: string
  timestamp?: string
  data?: Record<string, unknown>
  // SSE events may also carry ad-hoc fields
  [key: string]: unknown
}

// Approval
export interface Approval {
  id: string
  subject: string
  action: string
  resource: string
  reason: string
  created_at: string
  status: string
}

// Cron Job
export interface CronJob {
  id: string
  name: string
  schedule: string
  command?: string
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
  id?: string
  agent_id?: string
  agent_name?: string
  action: string
  resource?: string
  allowed?: boolean
  reason?: string | null
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
  role?: string
  description?: string
  enabled: boolean
  personality_traits?: string[]
}

// Agent Group
export interface AgentGroup {
  id: string
  name: string
  agents: string[]
  strategy?: string
}

// Workspace — matches backend TreeEntry from /api/workspace/tree
export interface TreeEntry {
  name: string
  is_dir: boolean
  size: number
}

// Paginated response
export interface PaginatedResponse<T> {
  items: T[]
  total: number
  page: number
  limit: number
}

// Status — matches backend StatusResponse
export interface SystemStatus {
  service: string
  status: string
  version: string
  channels: string[]
  uptime: string // formatted "1h 30m 5s"
  components?: {
    state_store?: { healthy: boolean; detail?: string }
    event_bus?: { healthy: boolean; detail?: string }
    memory?: { enabled: boolean; index_size: number; total_entries: number }
    agents?: { active_count: number; total_forked: number; total_completed: number; total_failed: number }
    spaces_active?: number
  }
}
