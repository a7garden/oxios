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
  space_id?: string
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
  space_id?: string
  user_messages: { content: string; timestamp: string }[]
  agent_responses: {
    content: string
    session_id: string
    seed_id: string
    phase_reached: string
    evaluation_passed: boolean
    timestamp: string
  }[]
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

// Skill (RFC-009 unified model)
export type SkillSource = 'bundled' | 'managed' | 'workspace'
export type SkillStatus = 'ready' | 'needs_setup' | 'disabled'
export type SkillFormat = 'oxios' | 'openclaw' | 'claude_code' | 'agent_skills'

export interface SkillRequirements {
  bins: string[]
  anyBins: string[]
  env: string[]
  config: string[]
}

export interface SkillInstallSpec {
  kind: 'brew' | 'node' | 'go' | 'uv' | 'download'
  label?: string
  bins: string[]
}

export interface Skill {
  name: string
  description: string
  author?: string
  version?: string
  emoji?: string
  homepage?: string
  source: SkillSource
  bundled: boolean
  status: SkillStatus
  eligible: boolean
  always: boolean
  user_invocable: boolean
  file_path: string
  requirements: SkillRequirements
  missing: SkillRequirements
  os: string[]
  install: SkillInstallSpec[]
  config_checks: Array<{ path: string; satisfied: boolean }>
  format: SkillFormat
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
  id: string
  role: 'user' | 'assistant' | 'system' | 'tool'
  content: string
  timestamp?: string
  // Tool call fields (role === 'tool')
  toolName?: string
  toolArgs?: Record<string, unknown>
  toolResult?: unknown
  toolDurationMs?: number
  // Completion metadata (last assistant message)
  metadata?: {
    phase?: string
    evaluation_passed?: boolean
    seed_id?: string
    duration_ms?: number
    tool_calls?: ToolCallSummary[]
  }
}

export interface ToolCallSummary {
  tool_name: string
  input: string
  output: string
  duration_ms: number
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
  session_id?: string
  space_id?: string
  phase?: string
  evaluation_passed?: string
  seed_id?: string
  duration_ms?: number
  tool_calls?: ToolCallSummary[]
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

// Budget — moved to types/budget.ts

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
    agents?: {
      active_count: number
      total_forked: number
      total_completed: number
      total_failed: number
    }
    spaces_active?: number
  }
}

// ClawHub / Marketplace (RFC-010)

export interface ClawHubSearchResult {
  score: number
  slug: string
  displayName: string
  summary?: string
  version?: string
  updatedAt?: number
}

export interface ClawHubSkillDetail {
  skill: {
    slug: string
    displayName: string
    summary?: string
    tags?: Record<string, string>
    createdAt: number
    updatedAt: number
  } | null
  latestVersion?: {
    version: string
    createdAt: number
    changelog?: string
  }
  metadata?: {
    os?: string[]
    systems?: string[]
  }
  owner?: {
    handle?: string
    displayName?: string
    image?: string
  }
}
