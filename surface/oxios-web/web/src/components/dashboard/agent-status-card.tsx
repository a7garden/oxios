import { Link } from '@tanstack/react-router'
import { Activity, Bot } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Area, AreaChart, ResponsiveContainer } from 'recharts'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cssVarToRgb } from '@/lib/utils'

export interface AgentStatusCardProps {
  /** Cumulative forked count from /api/status. `null` = unknown. */
  total: number | null
  /** Currently running agent count (from /api/agents). */
  running: number
  /** Failed count (cumulative, from /api/status). */
  failed: number
  /** Sparkline time-series for the running count. */
  runningSeries?: number[]
}

/**
 * Agent Status KPI card.
 *
 * Designed for the dashboard's hero KPI row. Unlike `StatCard` (single
 * value + label + sparkline), this card shows a "3 / 12" running/total
 * fraction plus an optional failed count. The sparkline tracks the
 * running count — the cumulative total is monotonic, so a sparkline
 * for it would be misleading.
 *
 * When `total` is `null`, the cumulative count is shown as "?" with
 * a tooltip explaining the missing data.
 *
 * Click → `/agents`.
 */
export function AgentStatusCard({ total, running, failed, runningSeries }: AgentStatusCardProps) {
  const { t } = useTranslation()
  const hasSparkline = Array.isArray(runningSeries) && runningSeries.length > 1
  const series = hasSparkline ? runningSeries.map((v, i) => ({ i, v })) : []
  const showFailed = failed > 0
  const totalUnknown = total === null
  const totalLabel = totalUnknown ? '?' : String(total)

  // Resolve success color at runtime for SVG (Recharts)
  const successColor = useMemo(() => cssVarToRgb('--color-success'), [])

  return (
    <Link to="/agents" className="block h-full focus:outline-none">
      <Card
        className="relative h-full overflow-hidden cursor-pointer transition-all hover:bg-accent/60 hover:shadow-sm hover:-translate-y-px focus-visible:ring-2 focus-visible:ring-ring"
        title={totalUnknown ? t('dashboard.totalForkedUnavailable') : undefined}
      >
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
            {t('dashboard.agentsStatus')}
          </CardTitle>
          <Bot className="h-4 w-4 shrink-0 text-info" />
        </CardHeader>
        <CardContent>
          <div className="flex items-end justify-between gap-2">
            <div className="min-w-0">
              <div className="text-2xl font-bold leading-none">
                <span className="text-foreground">{running}</span>
                <span className="text-muted-foreground"> / </span>
                <span className={totalUnknown ? 'text-muted-foreground' : 'text-foreground'}>
                  {totalLabel}
                </span>
              </div>
              <div className="mt-1.5 flex items-center gap-1.5 text-xs text-muted-foreground">
                <Activity className="h-3 w-3" />
                <span>{t('dashboard.agentsRunning')}</span>
                {showFailed && (
                  <>
                    <span className="text-border">·</span>
                    <span className="text-error font-medium">
                      {failed} {t('dashboard.agentsFailed')}
                    </span>
                  </>
                )}
              </div>
            </div>
            {hasSparkline && (
              <div className="h-10 w-20 shrink-0" aria-hidden="true">
                <ResponsiveContainer width={80} height={40}>
                  <AreaChart data={series} margin={{ top: 2, right: 0, bottom: 2, left: 0 }}>
                    <defs>
                      <linearGradient id="spark-agent-status" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor={successColor} stopOpacity={0.5} />
                        <stop offset="100%" stopColor={successColor} stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <Area
                      type="monotone"
                      dataKey="v"
                      stroke={successColor}
                      strokeWidth={1.5}
                      fill="url(#spark-agent-status)"
                      isAnimationActive={false}
                      dot={false}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </Link>
  )
}
