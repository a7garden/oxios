// StatsDashboard — unified usage statistics dashboard (LobeHub-inspired)

import { useQuery } from '@tanstack/react-query'
import { Bot, Coins, DollarSign, TrendingUp } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { CostByModel } from '@/components/cost/cost-by-model'
import { CostChart } from '@/components/cost/cost-chart'
import { ProviderQuotaCards } from '@/components/cost/provider-quota-cards'
import { SpendLimitCard } from '@/components/cost/spend-limit-card'
import { StatCard } from '@/components/dashboard/stat-card'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'

interface StatsOverview {
  total_cost_usd: number
  total_tokens: number
  agent_count: number
  total_sessions?: number
  month_to_date_spend_usd?: number
  spend_limit_usd?: number | null
}

export function StatsDashboard({ className }: { className?: string }) {
  const { t } = useTranslation()
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
        <h2 className="text-lg font-semibold mb-3">{t('dashboard.statsOverview')}</h2>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <StatCard
            icon={<DollarSign className="w-4 h-4" />}
            iconClassName="text-emerald-500"
            label={t('dashboard.totalSpend')}
            value={overview ? formatCost(overview.total_cost_usd) : '—'}
            hint={
              overview ? t('dashboard.agentsCount', { count: overview.agent_count }) : undefined
            }
          />
          <StatCard
            icon={<TrendingUp className="w-4 h-4" />}
            iconClassName="text-blue-500"
            label={t('dashboard.today')}
            value={todayStats ? formatCost(todayStats.total_cost_usd) : '—'}
            hint={todayStats ? formatTokens(todayStats.total_tokens) : undefined}
          />
          <StatCard
            icon={<Coins className="w-4 h-4" />}
            iconClassName="text-amber-500"
            label={t('dashboard.totalTokens')}
            value={overview ? formatTokens(overview.total_tokens) : '—'}
            hint={t('dashboard.allTime')}
          />
          <StatCard
            icon={<Bot className="w-4 h-4" />}
            iconClassName="text-purple-500"
            label={t('dashboard.sessionsLabel')}
            value={overview ? String(overview.total_sessions ?? 0) : '—'}
            hint={t('dashboard.totalLabel')}
          />
        </div>
      </section>

      {/* Spend limit */}
      <section>
        <SpendLimitCard />
      </section>

      {/* Usage trends */}
      <section>
        <h2 className="text-lg font-semibold mb-3">{t('dashboard.usageTrends')}</h2>
        <div className="rounded-xl border bg-card p-4">
          <CostChart days={30} />
        </div>
      </section>

      {/* Cost by model */}
      <section>
        <h2 className="text-lg font-semibold mb-3">{t('dashboard.costByModelSection')}</h2>
        <CostByModel period="all" />
      </section>

      {/* Provider quotas */}
      <section>
        <h2 className="text-lg font-semibold mb-3">{t('dashboard.providerQuotas')}</h2>
        <ProviderQuotaCards />
      </section>

      {/* Top Models */}
      <section>
        <h2 className="text-lg font-semibold mb-3">Top Models</h2>
        <div className="rounded-xl border bg-card overflow-hidden">
          <TopModelsTable />
        </div>
      </section>
    </div>
  )
}

// ── Top Models mini-table ──

function TopModelsTable() {
  const { data } = useQuery({
    queryKey: ['cost-by-model'],
    queryFn: () =>
      api.get<Array<{ model: string; cost: number; tokens: number }>>(
        '/api/costs/by-model?period=30d',
      ),
  })

  if (!data || data.length === 0) {
    return <div className="p-6 text-center text-sm text-muted-foreground">No data yet</div>
  }

  const top5 = [...data].sort((a, b) => b.cost - a.cost).slice(0, 5)
  const maxCost = top5[0]?.cost ?? 1

  return (
    <div className="divide-y">
      {top5.map((row, i) => {
        const pct = (row.cost / maxCost) * 100
        return (
          <div key={row.model} className="flex items-center gap-3 px-4 py-3">
            <span className="text-xs font-mono text-muted-foreground w-5">{i + 1}</span>
            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between mb-1">
                <span className="text-sm font-medium truncate">{row.model}</span>
                <span className="text-xs text-muted-foreground tabular-nums ml-2 shrink-0">
                  ${row.cost.toFixed(2)}
                </span>
              </div>
              <div className="h-1.5 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full bg-primary transition-all"
                  style={{ width: `${pct}%` }}
                />
              </div>
              <div className="text-2xs text-muted-foreground mt-0.5 tabular-nums">
                {formatTokens(row.tokens)} tokens
              </div>
            </div>
          </div>
        )
      })}
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
