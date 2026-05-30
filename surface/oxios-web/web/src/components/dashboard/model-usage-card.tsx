import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { useRoutingStats } from '@/hooks/use-engine'

export function ModelUsageCard() {
  const { t } = useTranslation()
  const { data } = useRoutingStats()

  if (!data || data.totalRequests === 0) {
    return null
  }

  const sorted = Object.entries(data.modelCalls)
    .sort(([, a], [, b]) => b - a)
    .slice(0, 5)

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium">{t('settings.routing.title')}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {sorted.map(([model, count]) => {
          const pct = (count / data.totalRequests) * 100
          const cost = data.modelCost[model] ?? 0
          return (
            <div key={model} className="space-y-1">
              <div className="flex justify-between text-xs">
                <span className="truncate max-w-[55%]" title={model}>
                  {model.split('/').pop()}
                </span>
                <span className="text-muted-foreground">
                  {pct.toFixed(0)}% ({count}) · ${cost.toFixed(3)}
                </span>
              </div>
              <Progress value={pct} className="h-1.5" />
            </div>
          )
        })}
        {data.totalCost > 0 && (
          <p className="pt-1 text-xs text-muted-foreground">
            ${data.totalCost.toFixed(2)} 총 비용 · {data.totalRequests}회 호출
          </p>
        )}
      </CardContent>
    </Card>
  )
}