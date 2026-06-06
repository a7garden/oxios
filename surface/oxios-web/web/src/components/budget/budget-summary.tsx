import { useTranslation } from 'react-i18next'
import { Card, CardContent } from '@/components/ui/card'
import type { BudgetSummary } from '@/types/budget'

interface Props {
  summary: BudgetSummary
}

export function BudgetSummaryCard({ summary }: Props) {
  const { t } = useTranslation()
  const pct =
    summary.total_tokens_limit > 0
      ? Math.min(100, (summary.total_tokens_used / summary.total_tokens_limit) * 100)
      : 0

  return (
    <Card>
      <CardContent className="p-5">
        <div className="grid gap-4 sm:grid-cols-3">
          <div>
            <p className="text-sm text-muted-foreground">{t('budget.totalAgents')}</p>
            <p className="text-2xl font-bold">{summary.total_agents}</p>
          </div>
          <div>
            <p className="text-sm text-muted-foreground">{t('budget.tokens')}</p>
            <p className="text-2xl font-bold">
              {summary.total_tokens_used.toLocaleString()}
              <span className="text-sm text-muted-foreground font-normal">
                {' '}/ {summary.total_tokens_limit.toLocaleString()}
              </span>
            </p>
            <div className="mt-1.5 h-2 rounded-full bg-muted overflow-hidden">
              <div
                className={`h-full rounded-full transition-all ${pct >= 90 ? 'bg-error' : pct >= 70 ? 'bg-warning' : 'bg-primary'}`}
                style={{ width: `${pct}%` }}
              />
            </div>
          </div>
          <div>
            <p className="text-sm text-muted-foreground">{t('budget.exhaustedCount')}</p>
            <p className={`text-2xl font-bold ${summary.exhausted_agents > 0 ? 'text-error' : ''}`}>
              {summary.exhausted_agents}
            </p>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
