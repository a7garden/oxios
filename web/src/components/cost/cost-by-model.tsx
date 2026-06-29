import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { useCostByModel } from '@/hooks/use-costs'
import { formatUsd } from '@/lib/utils'
import type { CostPeriod } from '@/types/cost'

interface Props {
  period: CostPeriod
}

export function CostByModel({ period }: Props) {
  const { t } = useTranslation()
  const { data, isLoading } = useCostByModel(period)

  const items = data?.items ?? []
  const maxCost = items.length > 0 ? Math.max(...items.map((i) => i.cost_usd)) : 0

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{t('cost.spendByModel')}</CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <p className="text-sm text-muted-foreground py-4">{t('common.loading')}</p>
        ) : items.length === 0 ? (
          <EmptyState title={t('cost.noData')} />
        ) : (
          <div className="space-y-3">
            {items.slice(0, 10).map((row) => {
              const pct = maxCost > 0 ? (row.cost_usd / maxCost) * 100 : 0
              return (
                <div key={row.model_id} className="space-y-1">
                  <div className="flex items-center justify-between text-sm">
                    <span className="font-mono text-xs truncate max-w-[60%]">{row.model_id}</span>
                    <div className="flex items-center gap-3 text-xs text-muted-foreground">
                      <span>{formatUsd(row.cost_usd)}</span>
                      <span>{(row.tokens / 1000).toFixed(1)}k</span>
                    </div>
                  </div>
                  <Progress value={pct} className="h-1.5" />
                </div>
              )
            })}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
