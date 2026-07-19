// StatsDashboard — unified usage statistics dashboard (LobeHub-inspired)

import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { Bot, TrendingUp, DollarSign, Coins } from 'lucide-react'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import { StatCard } from '@/components/dashboard/stat-card'
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
            hint={overview ? t('dashboard.agentsCount', { count: overview.agent_count }) : undefined}
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
