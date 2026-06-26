/** Cost & spend types — dollar-based views over agent_log_db. */

export interface CostSummary {
  total_cost_usd: number
  total_tokens: number
  agent_count: number
  period: string
  spend_limit_usd: number | null
  month_to_date_spend_usd: number
  month_to_date_tokens: number
}

export interface ModelCostRow {
  model_id: string
  cost_usd: number
  tokens: number
  agent_count: number
}

export interface ProjectCostRow {
  project_id: string
  cost_usd: number
  tokens: number
  agent_count: number
}

export interface DailyCostRow {
  date: string
  cost_usd: number
  tokens: number
  agent_count: number
}

/** Provider quota snapshot — the "subscription quota" axis. */
export interface RateWindow {
  name: string
  used: number | null
  limit: number | null
  remaining_percent: number | null
  resets_at: string | null
}

export interface QuotaSnapshot {
  provider: string
  credit_balance_usd: number | null
  period_spend_usd: number | null
  period_start: string | null
  plan: string | null
  rate_windows: RateWindow[]
  fetched_at: string
  error: string | null
}

export interface SpendLimit {
  monthly_limit_usd: number | null
  month_to_date_spend_usd: number
  month_to_date_tokens: number
}

export type CostPeriod = 'today' | 'week' | 'month' | 'all'
