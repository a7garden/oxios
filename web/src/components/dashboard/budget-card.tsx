import { Link } from '@tanstack/react-router'
import { AlertTriangle, Coins } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { useBudgetList } from '@/hooks/use-budget'

/**
 * Budget overview card for the dashboard.
 *
 * Shows total token usage vs. limit, number of agents with budgets,
 * and any exhausted agents.
 */
export function BudgetCard({ className }: { className?: string }) {
  const { t } = useTranslation()
  const { data: budgetData } = useBudgetList()

  const summary = budgetData?.summary
  const agents = Array.isArray(budgetData?.agents) ? budgetData.agents : []
  const totalUsed = summary?.total_tokens_used ?? 0
  const totalLimit = summary?.total_tokens_limit ?? 0
  const usagePct = totalLimit > 0 ? Math.min((totalUsed / totalLimit) * 100, 100) : 0
  const exhausted = summary?.exhausted_agents ?? 0

  // Top consumers
  const topAgents = [...agents]
    .sort((a, b) => b.budget.tokens_used - a.budget.tokens_used)
    .slice(0, 3)

  return (
    <Card className={className}>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Coins className="h-4 w-4" />
          {t('dashboard.budget')}
        </CardTitle>
        <Link
          to="/budget"
          className="text-xs text-muted-foreground hover:text-foreground underline-offset-4 hover:underline"
        >
          {t('dashboard.viewAll')}
        </Link>
      </CardHeader>
      <CardContent className="pt-0">
        {totalLimit === 0 ? (
          <p className="text-xs text-muted-foreground py-1">{t('dashboard.noBudgetsSet')}</p>
        ) : (
          <div className="space-y-2">
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted-foreground">
                {formatTokenCount(totalUsed)} / {formatTokenCount(totalLimit)}
              </span>
              <span className="font-semibold">{usagePct.toFixed(0)}%</span>
            </div>
            <Progress value={usagePct} className="h-1.5" />
            {exhausted > 0 && (
              <div className="flex items-center gap-1.5 text-xs text-warning">
                <AlertTriangle className="h-3 w-3" />
                <span>
                  {exhausted} {t('dashboard.exhaustedAgents')}
                </span>
              </div>
            )}
            {topAgents.length > 0 && (
              <div className="pt-1 space-y-1">
                {topAgents.map((a) => {
                  const pct =
                    a.budget.token_limit > 0
                      ? (a.budget.tokens_used / a.budget.token_limit) * 100
                      : 0
                  return (
                    <div key={a.agent_id} className="flex items-center justify-between text-xs">
                      <span className="truncate max-w-[60%]">
                        {a.name ?? a.agent_id.slice(0, 8)}
                      </span>
                      <span className="text-muted-foreground">{pct.toFixed(0)}%</span>
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

function formatTokenCount(n: number): string {
  if (n < 1000) return `${n}`
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`
  return `${(n / 1_000_000).toFixed(1)}M`
}
