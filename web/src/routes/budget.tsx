import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { BudgetManagement } from '@/components/budget/budget-management'
import { CostByModel } from '@/components/cost/cost-by-model'
import { CostByProject } from '@/components/cost/cost-by-project'
import { CostChart } from '@/components/cost/cost-chart'
import { CostSummaryCards } from '@/components/cost/cost-summary'
import { ProviderQuotaCards } from '@/components/cost/provider-quota-cards'
import { SpendLimitCard } from '@/components/cost/spend-limit-card'
import { PageHeader } from '@/components/shared/page-header'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useCostSummary } from '@/hooks/use-costs'
import type { CostPeriod } from '@/types/cost'

export const Route = createFileRoute('/budget')({ component: CostPage })

const PERIODS: CostPeriod[] = ['today', 'week', 'month', 'all']

function CostPage() {
  const { t } = useTranslation()
  const [period, setPeriod] = useState<CostPeriod>('month')
  const { refetch, isFetching } = useCostSummary(period)

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('cost.pageTitle')}
        subtitle={t('cost.subtitle')}
        actions={<RefreshButton onClick={() => refetch()} isFetching={isFetching} />}
      />

      {/* Spend limit + period selector */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <SpendLimitCard />
        <div className="flex items-end sm:col-span-1 lg:col-span-3">
          <Tabs value={period} onValueChange={(v) => setPeriod(v as CostPeriod)} className="w-full">
            <TabsList>
              {PERIODS.map((p) => (
                <TabsTrigger key={p} value={p}>
                  {t(`cost.period.${p}`)}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
        </div>
      </div>
      <p className="text-xs text-muted-foreground">{t('cost.spendLimitNote')}</p>
      <p className="text-xs text-muted-foreground">{t('cost.periodScopeNote')}</p>

      {/* Summary stat cards */}
      <CostSummaryCards period={period} />

      {/* Daily spend chart */}
      <CostChart days={30} />

      {/* Breakdowns */}
      <div className="grid gap-4 lg:grid-cols-2">
        <CostByModel period={period} />
        <CostByProject period={period} />
      </div>

      {/* Provider panel — all configured providers + quota data */}
      <ProviderQuotaCards />

      {/* Agent budget management — token/call rate limits */}
      <div className="space-y-2">
        <h2 className="text-lg font-semibold">{t('budget.title')}</h2>
        <BudgetManagement />
      </div>
    </div>
  )
}
