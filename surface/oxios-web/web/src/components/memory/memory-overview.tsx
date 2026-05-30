import {
  PieChart,
  Pie,
  Cell,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip as RechartsTooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Brain, Hash, Pin, Activity } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useMemoryStats } from '@/hooks/use-memory'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'

const TIER_COLORS = { hot: '#ef4444', warm: '#eab308', cold: '#3b82f6' }

export function MemoryOverview() {
  const { t } = useTranslation()
  const {
    data: stats,
    isLoading,
    isError,
    refetch,
  } = useMemoryStats()

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!stats) return null

  const tierData = Object.entries(stats.by_tier || {})
    .filter(([, v]) => v > 0)
    .map(([name, value]) => ({
      name: t(`memory.${name}`, name),
      value,
      fill: TIER_COLORS[name as keyof typeof TIER_COLORS] || '#888',
    }))

  const typeData = Object.entries(stats.by_type || {})
    .filter(([, v]) => v > 0)
    .map(([name, value]) => ({ name: t(`memory.${name}`, name), value }))

  const statCards = [
    { label: t('memory.totalEntries'), value: stats.total, icon: Brain },
    {
      label: t('memory.vectorIndex'),
      value: stats.by_type ? Object.keys(stats.by_type).length : 0,
      icon: Hash,
    },
    { label: t('memory.pinnedEntries'), value: 0, icon: Pin },
    {
      label: t('memory.dreamStatus'),
      value: 'idle',
      icon: Activity,
    },
  ]

  return (
    <div className="space-y-6">
      <div className="grid gap-4 md:grid-cols-4">
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
          <CardContent>
            {tierData.length > 0 ? (
              <ResponsiveContainer width="100%" height={200}>
                <PieChart>
                  <Pie
                    data={tierData}
                    dataKey="value"
                    nameKey="name"
                    cx="50%"
                    cy="50%"
                    outerRadius={80}
                    label
                  >
                    {tierData.map((entry, i) => (
                      <Cell key={i} fill={entry.fill} />
                    ))}
                  </Pie>
                  <RechartsTooltip />
                  <Legend />
                </PieChart>
              </ResponsiveContainer>
            ) : (
              <p className="text-sm text-muted-foreground">
                {t('memory.noData')}
              </p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{t('memory.typeDistribution')}</CardTitle>
          </CardHeader>
          <CardContent>
            {typeData.length > 0 ? (
              <ResponsiveContainer width="100%" height={200}>
                <BarChart data={typeData}>
                  <XAxis dataKey="name" tick={{ fontSize: 11 }} />
                  <YAxis />
                  <RechartsTooltip />
                  <Bar dataKey="value" fill="#8884d8" />
                </BarChart>
              </ResponsiveContainer>
            ) : (
              <p className="text-sm text-muted-foreground">
                {t('memory.noData')}
              </p>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
