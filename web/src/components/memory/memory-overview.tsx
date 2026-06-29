import { Activity, Brain, Hash } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useMemoryStats } from '@/hooks/use-memory'

const TIER_COLORS: Record<string, string> = {
  hot: '#ef4444',
  warm: '#eab308',
  cold: '#3b82f6',
}

/**
 * One horizontal distribution row: label · bar (scaled to `max`) · count.
 *
 * NOTE: this replaces recharts `<BarChart>`/`<PieChart>` here. recharts 3.x's
 * Bar/Pie components are mis-bundled by rolldown (vite v8) — they render fine in
 * dev but throw `TypeError: t is not a function` in the production bundle.
 * AreaChart (used elsewhere) is unaffected; only Bar/Pie break. A dependency-
 * free bar avoids the defective code path and renders identically in every build.
 */
function DistBar({
  label,
  value,
  max,
  color,
}: {
  label: string
  value: number
  max: number
  color: string
}) {
  const pct = max > 0 ? Math.round((value / max) * 100) : 0
  return (
    <div className="flex items-center gap-3 text-sm">
      <span className="w-20 shrink-0 truncate text-muted-foreground" title={label}>
        {label}
      </span>
      <div className="h-2.5 flex-1 overflow-hidden rounded-full bg-muted">
        <div
          className="h-full rounded-full transition-all"
          style={{ width: `${pct}%`, backgroundColor: color }}
        />
      </div>
      <span className="w-8 shrink-0 text-right font-medium tabular-nums">{value}</span>
    </div>
  )
}

export function MemoryOverview() {
  const { t } = useTranslation()
  const { data: stats, isLoading, isError, refetch } = useMemoryStats()

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!stats) return null

  const tierEntries = Object.entries(stats.by_tier || {})
    .filter(([, v]) => v > 0)
    .map(([name, value]) => [name, value] as [string, number])
  const typeEntries = Object.entries(stats.by_type || {})
    .filter(([, v]) => v > 0)
    .map(([name, value]) => [name, value] as [string, number])
  const tierMax = Math.max(1, ...tierEntries.map(([, v]) => v))
  const typeMax = Math.max(1, ...typeEntries.map(([, v]) => v))

  const statCards = [
    { label: t('memory.totalEntries'), value: stats.total, icon: Brain },
    {
      label: t('memory.vectorIndex'),
      value: stats.by_type ? Object.keys(stats.by_type).length : 0,
      icon: Hash,
    },

    { label: t('memory.dreamStatus'), value: 'idle', icon: Activity },
  ]

  return (
    <div className="space-y-6">
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {statCards.map((s) => (
          <Card key={s.label}>
            <CardContent className="p-4 flex items-center gap-3">
              <s.icon className="h-8 w-8 text-muted-foreground" />
              <div>
                <p className="text-2xl font-bold">{s.value}</p>
                <p className="text-xs text-muted-foreground">{s.label}</p>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
      <div className="grid gap-6 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>{t('memory.tierDistribution')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {tierEntries.length > 0 ? (
              tierEntries.map(([name, value]) => (
                <DistBar
                  key={name}
                  label={t(`memory.${name}`, name)}
                  value={value}
                  max={tierMax}
                  color={TIER_COLORS[name] ?? '#888'}
                />
              ))
            ) : (
              <p className="text-sm text-muted-foreground">{t('memory.noData')}</p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{t('memory.typeDistribution')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {typeEntries.length > 0 ? (
              typeEntries.map(([name, value]) => (
                <DistBar
                  key={name}
                  label={t(`memory.${name}`, name)}
                  value={value}
                  max={typeMax}
                  color="#8884d8"
                />
              ))
            ) : (
              <p className="text-sm text-muted-foreground">{t('memory.noData')}</p>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
