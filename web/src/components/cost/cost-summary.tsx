import { Bot, Coins, Cpu } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useCostSummary } from '@/hooks/use-costs'
import type { CostPeriod } from '@/types/cost'

interface Props {
  period: CostPeriod
}

export function CostSummaryCards({ period }: Props) {
  const { t } = useTranslation()
  const { data, isLoading } = useCostSummary(period)

  if (isLoading) return null

  const cost = data?.total_cost_usd ?? 0
  const tokens = data?.total_tokens ?? 0
  const agents = data?.agent_count ?? 0

  return (
    <div className="grid gap-4 sm:grid-cols-3">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">{t('cost.totalSpend')}</CardTitle>
          <Coins className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">${cost.toFixed(4)}</div>
          <p className="text-xs text-muted-foreground">{t('cost.usd')}</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">{t('cost.totalTokens')}</CardTitle>
          <Cpu className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{tokens.toLocaleString()}</div>
          <p className="text-xs text-muted-foreground">{t('cost.tokensConsumed')}</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">{t('cost.agentRuns')}</CardTitle>
          <Bot className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{agents.toLocaleString()}</div>
          <p className="text-xs text-muted-foreground">{t('cost.executions')}</p>
        </CardContent>
      </Card>
    </div>
  )
}
