import { Minus, TrendingDown, TrendingUp } from 'lucide-react'
import { Area, AreaChart, ResponsiveContainer } from 'recharts'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'

export type SparkColor = 'blue' | 'emerald' | 'amber' | 'violet' | 'red' | 'cyan'

const COLOR_MAP: Record<SparkColor, { stroke: string; fill: string }> = {
  blue: { stroke: 'rgb(59 130 246)', fill: 'rgb(59 130 246 / 0.18)' },
  emerald: { stroke: 'rgb(16 185 129)', fill: 'rgb(16 185 129 / 0.18)' },
  amber: { stroke: 'rgb(245 158 11)', fill: 'rgb(245 158 11 / 0.18)' },
  violet: { stroke: 'rgb(139 92 246)', fill: 'rgb(139 92 246 / 0.18)' },
  red: { stroke: 'rgb(239 68 68)', fill: 'rgb(239 68 68 / 0.18)' },
  cyan: { stroke: 'rgb(6 182 212)', fill: 'rgb(6 182 212 / 0.18)' },
}

export interface StatCardProps {
  /** Visible card label (i18n key or raw string). */
  label: string
  /** Big number / text to display. */
  value: string | number
  /** Optional icon (e.g. lucide-react component) shown in the header. */
  icon?: React.ReactNode
  /** Tailwind color class for the icon, e.g. "text-emerald-500". */
  iconClassName?: string
  /**
   * Percent change vs. start of the sparkline window. Positive = up arrow,
   * negative = down arrow, near-zero = minus.
   */
  delta?: number
  /** Time series for the mini sparkline. Empty/undefined hides the chart. */
  sparkline?: number[]
  /** Sparkline accent color. */
  sparkColor?: SparkColor
  /** Optional extra info, e.g. a unit or hint, shown next to the value. */
  hint?: string
  /** Optional URL — clicking the card navigates here. */
  href?: string
  /** Click handler. Either `href` or `onClick` may be set. */
  onClick?: () => void
}

/**
 * Compact KPI card with a mini sparkline + delta indicator.
 *
 * Designed for the dashboard stat row. Renders as a Card; when `href`
 * is provided, wraps in an anchor with hover affordance.
 */
export function StatCard({
  label,
  value,
  icon,
  iconClassName,
  delta,
  sparkline,
  sparkColor = 'blue',
  hint,
  href,
  onClick,
}: StatCardProps) {
  const colors = COLOR_MAP[sparkColor]
  const hasSparkline = Array.isArray(sparkline) && sparkline.length > 1

  const series = hasSparkline ? sparkline.map((v, i) => ({ i, v })) : []

  const deltaDir: 'up' | 'down' | 'flat' =
    typeof delta !== 'number' ? 'flat' : Math.abs(delta) < 0.5 ? 'flat' : delta > 0 ? 'up' : 'down'

  const cardInner = (
    <Card
      className={cn(
        'relative h-full overflow-hidden',
        (href || onClick) &&
          'cursor-pointer transition-colors hover:bg-accent/40 focus-visible:ring-2 focus-visible:ring-ring',
      )}
      onClick={onClick}
    >
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          {label}
        </CardTitle>
        {icon && (
          <div className={cn('shrink-0', iconClassName ?? 'text-muted-foreground')}>{icon}</div>
        )}
      </CardHeader>
      <CardContent>
        <div className="flex items-end justify-between gap-2">
          <div className="min-w-0">
            <div className="text-2xl font-bold leading-none truncate">{value}</div>
            {(hint || typeof delta === 'number') && (
              <div className="mt-1.5 flex items-center gap-1.5 text-xs text-muted-foreground">
                {typeof delta === 'number' && (
                  <span
                    className={cn(
                      'inline-flex items-center gap-0.5 font-medium',
                      deltaDir === 'up' && 'text-emerald-600 dark:text-emerald-400',
                      deltaDir === 'down' && 'text-red-600 dark:text-red-400',
                      deltaDir === 'flat' && 'text-muted-foreground',
                    )}
                    aria-label={
                      deltaDir === 'up'
                        ? `up ${delta.toFixed(1)}%`
                        : deltaDir === 'down'
                          ? `down ${delta.toFixed(1)}%`
                          : 'no change'
                    }
                  >
                    {deltaDir === 'up' && <TrendingUp className="h-3 w-3" />}
                    {deltaDir === 'down' && <TrendingDown className="h-3 w-3" />}
                    {deltaDir === 'flat' && <Minus className="h-3 w-3" />}
                    {deltaDir !== 'flat' && `${delta > 0 ? '+' : ''}${delta.toFixed(0)}%`}
                  </span>
                )}
                {hint && <span className="truncate">{hint}</span>}
              </div>
            )}
          </div>
          {hasSparkline && (
            <div className="h-10 w-20 shrink-0" aria-hidden="true">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={series} margin={{ top: 2, right: 0, bottom: 2, left: 0 }}>
                  <defs>
                    <linearGradient id={`spark-${sparkColor}`} x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor={colors.stroke} stopOpacity={0.4} />
                      <stop offset="100%" stopColor={colors.stroke} stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <Area
                    type="monotone"
                    dataKey="v"
                    stroke={colors.stroke}
                    strokeWidth={1.5}
                    fill={`url(#spark-${sparkColor})`}
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
  )

  if (href) {
    return (
      <a href={href} className="block h-full focus:outline-none">
        {cardInner}
      </a>
    )
  }
  return cardInner
}
