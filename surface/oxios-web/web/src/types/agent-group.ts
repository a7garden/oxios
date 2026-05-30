/** Agent Group types aligned with kernel OxiosAgentGroup. */

export type GroupStatus = 'Pending' | 'Running' | 'Completed' | 'Failed'

export interface GroupAgent {
  id: string
  seed: {
    id: string
    goal: string
    generation: number
  }
  status: GroupStatus
  result: string | null
}

export interface AgentGroup {
  id: string
  parent_seed_id: string
  agents: GroupAgent[]
  created_at?: string
}

export interface AgentGroupProgress {
  id: string
  status: GroupStatus
  total_agents: number
  completed: number
  failed: number
  pending: number
  running: number
  completion_pct: number
  combined_results: string
}
