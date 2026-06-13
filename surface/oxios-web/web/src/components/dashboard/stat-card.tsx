import { Minus, TrendingDown, TrendingUp } from 'lucide-react'
import { useMemo } from 'react'
import { Area, AreaChart, ResponsiveContainer } from 'recharts'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn, cssVarToRgb } from '@/lib/utils'

export type SparkColor = 'info' | 'success' | 'warning' | 'primary' | 'error' | 'accent' | 'muted'

/**
 * CSS custom property names for each spark color.
 * Resolved at runtime via cssVarToRgb() because Recharts SVG
 * attributes don't support CSS variables directly.
 */
const COLOR_VARS: Record<SparkColor, { stroke: string; fill: string }> = {
  info: { stroke: '--color-info', fill: '--color-info-muted' },
  success: { stroke: '--color-success', fill: '--color-success-muted' },
  warning: { stroke: '--color-warning', fill: '--color-warning-muted' },
  primary: { stroke: '--color-primary', fill: '--color-primary' },
  error: { stroke: '--color-error', fill: '--color-error-muted' },
  accent: { stroke: '--color-info', fill: '--color-info-muted' },
  muted: { stroke: '--color-muted-foreground', fill: '--color-muted-foreground' },
}

/**
 * Resolve spark color tokens to computed RGB strings.
 * Memoized per sparkColor so we don't thrash getComputedStyle.
 */
function useSparkColors(sparkColor: SparkColor) {
  return useMemo(() => {
    const vars = COLOR_VARS[sparkColor]
    const stroke = cssVarToRgb(vars.stroke)
    const fillBase = cssVarToRgb(vars.fill)
    // Convert fill to a semi-transparent version for the area fill
    // e.g. 'rgb(59 130 246)' → 'rgb(59 130 246 / 0.18)'
    const fill = fillBase.replace('rgb(', 'rgba(').replace(')', ' / 0.18)')
    return { stroke, fill }
  }, [sparkColor])
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
  /** Optional native `title` attribute — rendered on the outer card. */
  title?: string
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
  sparkColor = 'info',
  hint,
  href,
  onClick,
  title,
}: StatCardProps) {
  const colors = useSparkColors(sparkColor)
  const hasSparkline = Array.isArray(sparkline) && sparkline.length > 1

  const series = hasSparkline ? sparkline.map((v, i) => ({ i, v })) : []

  const deltaDir: 'up' | 'down' | 'flat' =
    typeof delta !== 'number' ? 'flat' : Math.abs(delta) < 0.5 ? 'flat' : delta > 0 ? 'up' : 'down'

  const cardInner = (
    <Card
      className={cn(
        'relative h-full overflow-hidden',
        (href || onClick) &&
          'cursor-pointer transition-all hover:bg-accent/60 hover:shadow-sm hover:-translate-y-px focus-visible:ring-2 focus-visible:ring-ring',
      )}
      onClick={onClick}
      title={title}
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
                      deltaDir === 'up' && 'text-success',
                      deltaDir === 'down' && 'text-error',
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
              <ResponsiveContainer width={80} height={40}>
                <AreaChart data={series} margin={{ top: 2, right: 0, bottom: 2, left: 0 }}>
                  <defs>
                    <linearGradient id={`spark-${sparkColor}`} x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor={colors.stroke} stopOpacity={0.5} />
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
