/** Agent history log types — persistent agent records with filtering */

// ── Agent list item (from GET /api/agents) ──
export interface AgentListItem {
  id: string
  name: string
  status: string
  created_at: string
  started_at: string | null
  completed_at: string | null
  project_id: string | null
  session_id: string | null
  error: string | null
  steps_completed: number
  steps_total: number | null
  tokens_used: number
  cost_usd: number
  model_id: string
  duration_secs: number | null
}

// ── Agent list response ──
export interface AgentListResponse {
  items: AgentListItem[]
  total: number
  page: number
  per_page: number
  total_pages: number
  stats: AgentFilteredStats
}

export interface AgentFilteredStats {
  total_cost_usd: number
  total_tokens: number
  avg_duration_secs: number
  count_running: number
  count_completed: number
  count_failed: number
}

// ── Agent global stats (from GET /api/agents/stats) ──
export interface AgentGlobalStats {
  total_agents: number
  running: number
  completed: number
  failed: number
  total_cost_usd: number
  total_tokens: number
  total_duration_secs: number
  avg_duration_secs: number
  avg_cost_usd: number
  total_sessions: number
  oldest_agent_at: string | null
  newest_agent_at: string | null
}

// ── Agent detail ──
export interface AgentDetail {
  id: string
  name: string
  status: string
  session_id: string | null
  project_id: string | null
  created_at: string
  started_at: string | null
  completed_at: string | null
  error: string | null
  steps_completed: number
  steps_total: number | null
  tokens_used: number
  cost_usd: number
  model_id: string
}

// ── Agent trace ──
export interface AgentTrace {
  agent_id: string
  steps: AgentTraceStep[]
  completed_at: string | null
}

export interface AgentTraceStep {
  index: number
  /** Step kind — tool call, memory recall, or reasoning fragment (RFC-028 SP-3a). */
  kind?: 'tool' | 'memory' | 'reasoning'
  tool_name: string | null
  action: string
  input: unknown
  output: unknown
  started_at: string
  duration_ms: number
  status: string
}

// ── Agent logs ──
export interface AgentLogs {
  agent_id: string
  entries: AgentLogEntry[]
}

export interface AgentLogEntry {
  timestamp: string
  level: string
  message: string
  metadata: Record<string, unknown> | null
}
