import { Link } from '@tanstack/react-router'
import { Coins } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { useCostByModel, useCostSummary } from '@/hooks/use-costs'

/**
 * Cost overview card for the dashboard.
 *
 * Shows real dollar spend this month and top models by cost,
 * backed by agent_log_db (the actual source of truth).
 */
export function BudgetCard({ className }: { className?: string }) {
  const { t } = useTranslation()
  const { data: summary } = useCostSummary('month')
  const { data: modelData } = useCostByModel('month')

  const totalSpend = summary?.total_cost_usd ?? 0
  const agentCount = summary?.agent_count ?? 0
  const models = (modelData?.items ?? []).slice(0, 3)
  const maxModelCost = models[0]?.cost_usd ?? 0

  return (
    <Card className={className}>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Coins className="h-4 w-4" />
          {t('cost.title')}
        </CardTitle>
        <Link
          to="/budget"
          className="text-xs text-muted-foreground hover:text-foreground underline-offset-4 hover:underline"
        >
          {t('dashboard.viewAll')}
        </Link>
      </CardHeader>
      <CardContent className="pt-0">
        {totalSpend === 0 && agentCount === 0 ? (
          <p className="text-xs text-muted-foreground py-1">{t('cost.noData')}</p>
        ) : (
          <div className="space-y-2">
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted-foreground">{t('cost.period.month')}</span>
              <span className="font-semibold">${totalSpend.toFixed(4)}</span>
            </div>
            <div className="text-xs text-muted-foreground">
              {agentCount.toLocaleString()} {t('cost.executions')}
            </div>
            {models.length > 0 && (
              <div className="pt-1 space-y-1">
                {models.map((m) => {
                  const pct = maxModelCost > 0 ? (m.cost_usd / maxModelCost) * 100 : 0
                  return (
                    <div key={m.model_id} className="space-y-0.5">
                      <div className="flex items-center justify-between text-xs">
                        <span className="truncate max-w-[60%] font-mono">{m.model_id}</span>
                        <span className="text-muted-foreground">${m.cost_usd.toFixed(4)}</span>
                      </div>
                      <Progress value={pct} className="h-1" />
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
