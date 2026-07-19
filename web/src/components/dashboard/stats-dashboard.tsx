// StatsDashboard — unified usage statistics dashboard (LobeHub-inspired)

'use client'

import { useQuery } from '@tanstack/react-query'
import { Bot, TrendingUp, DollarSign, Coins } from 'lucide-react'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import { CostByModel } from '@/components/cost/cost-by-model'
import { CostChart } from '@/components/cost/cost-chart'
import { ProviderQuotaCards } from '@/components/cost/provider-quota-cards'
import { SpendLimitCard } from '@/components/cost/spend-limit-card'

interface StatsOverview {
  total_cost_usd: number
  total_tokens: number
  agent_count: number
  total_sessions?: number
  month_to_date_spend_usd?: number
  spend_limit_usd?: number | null
}

export function StatsDashboard({ className }: { className?: string }) {
  const { data: overview } = useQuery({
    queryKey: ['stats-overview'],
    queryFn: () => api.get<StatsOverview>('/api/costs/summary?period=all'),
  })

  const { data: todayStats } = useQuery({
    queryKey: ['stats-today'],
    queryFn: () => api.get<StatsOverview>('/api/costs/summary?period=today'),
  })

  return (
    <div className={cn('space-y-6', className)}>
      {/* Overview cards */}
      <section>
        <h2 className="text-lg font-semibold mb-3">Overview</h2>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <StatCard
            icon={<DollarSign className="w-4 h-4" />}
            label="Total Spend"
            value={overview ? formatCost(overview.total_cost_usd) : '—'}
            sublabel={overview ? `${overview.agent_count} agents` : ''}
            color="text-emerald-500"
          />
          <StatCard
            icon={<TrendingUp className="w-4 h-4" />}
            label="Today"
            value={todayStats ? formatCost(todayStats.total_cost_usd) : '—'}
            sublabel={todayStats ? formatTokens(todayStats.total_tokens) : ''}
            color="text-blue-500"
          />
          <StatCard
            icon={<Coins className="w-4 h-4" />}
            label="Total Tokens"
            value={overview ? formatTokens(overview.total_tokens) : '—'}
            sublabel="all time"
            color="text-amber-500"
          />
          <StatCard
            icon={<Bot className="w-4 h-4" />}
            label="Sessions"
            value={overview ? String(overview.total_sessions ?? 0) : '—'}
            sublabel="total"
            color="text-purple-500"
          />
        </div>
      </section>

      {/* Spend limit */}
      <section>
        <SpendLimitCard />
      </section>

      {/* Usage trends */}
      <section>
        <h2 className="text-lg font-semibold mb-3">Usage Trends (30 days)</h2>
        <div className="rounded-xl border bg-card p-4">
          <CostChart days={30} />
        </div>
      </section>

      {/* Cost by model */}
      <section>
        <h2 className="text-lg font-semibold mb-3">Cost by Model</h2>
        <CostByModel period="all" />
      </section>

      {/* Provider quotas */}
      <section>
        <h2 className="text-lg font-semibold mb-3">Provider Quotas</h2>
        <ProviderQuotaCards />
      </section>
    </div>
  )
}

function StatCard({
  icon,
  label,
  value,
  sublabel,
  color,
}: {
  icon: React.ReactNode
  label: string
  value: string
  sublabel?: string
  color?: string
}) {
  return (
    <div className="rounded-xl border bg-card p-4">
      <div className="flex items-center gap-2 mb-2">
        <div className={cn('shrink-0', color)}>{icon}</div>
        <span className="text-xs text-muted-foreground font-medium">{label}</span>
      </div>
      <div className="text-xl font-semibold tabular-nums">{value}</div>
      {sublabel && <p className="text-xs text-muted-foreground mt-0.5">{sublabel}</p>}
    </div>
  )
}

function formatCost(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  if (usd < 1) return `$${usd.toFixed(3)}`
  return `$${usd.toFixed(2)}`
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`
  return String(n)
}
