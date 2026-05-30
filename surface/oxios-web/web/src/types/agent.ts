/** Types for the Agent trace hooks (use-agent-trace.ts) */

// ── Agent detail ──
export interface AgentDetail {
  id: string
  name: string
  status: string
  seed_id: string | null
  project_id: string | null
  created_at: string
  started_at: string | null
  completed_at: string | null
  error: string | null
  steps_completed: number
  steps_total: number | null
  tokens_used: number
  cost_usd: number
}

// ── Agent trace ──
export interface AgentTrace {
  agent_id: string
  steps: AgentTraceStep[]
  completed_at: string | null
}

export interface AgentTraceStep {
  index: number
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
