/** Budget data types aligned with backend FullBudgetInfo response. */

export interface BudgetData {
  token_limit: number
  tokens_used: number
  tokens_remaining: number
  calls_limit: number
  calls_used: number
  calls_remaining: number
  window_secs: number
  window_remaining_secs: number
  is_exhausted: boolean
}

export interface AgentBudget {
  agent_id: string
  name?: string
  budget: BudgetData
}

export interface BudgetSummary {
  total_agents: number
  total_tokens_used: number
  total_tokens_limit: number
  exhausted_agents: number
}

export interface BudgetListResponse {
  agents: AgentBudget[]
  summary: BudgetSummary
  total?: number
  page?: number
  limit?: number
}

export interface SetBudgetRequest {
  token_budget: number
  calls_budget: number
  window_secs: number
}
