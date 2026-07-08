import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Activity } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import {
  Area,
  AreaChart,
  CartesianGrid,
  Legend,
  Tooltip as RechartsTooltip,
  ReferenceLine,
  ResponsiveContainer,
  XAxis,
  YAxis,
} from 'recharts'

function getChartColor(token: string): string {
  if (typeof window === 'undefined') return '#888'
  return getComputedStyle(document.documentElement).getPropertyValue(token).trim() || '#888'
}

/** Severity color for a 0–100 utilization metric: neutral under 75%, warning 75–90%, error 90%+. */
function sevColor(pct: number): string {
  return getChartColor(pct >= 90 ? '--error' : pct >= 75 ? '--warning' : '--info')
}

import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { ResourceSnapshot } from '@/types'

export const Route = createFileRoute('/resources')({ component: ResourcesPage })

function ResourcesPage() {
  const { t } = useTranslation()
  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['resources'],
    queryFn: async () => {
      // /api/resources returns a single snapshot; /api/resources/history returns array
      const res = await api.get<{ snapshots: ResourceSnapshot[]; count: number }>(
        '/api/resources/history?last_n=30',
      )
      return Array.isArray(res?.snapshots) ? res.snapshots : []
    },
    refetchInterval: 5000,
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const snapshots = Array.isArray(data) ? data : []
  const latest = snapshots.length > 0 ? snapshots[snapshots.length - 1] : null

  const chartData = snapshots.map((s) => ({
    time: new Date(s.timestamp).toLocaleTimeString(),
    cpu: s.cpu_percent,
    memory: s.memory_percent,
    disk: s.disk_percent,
  }))

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('resources.title')}</h1>
          <p className="text-muted-foreground">{t('resources.subtitle')}</p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {/* Current Stats */}
      {latest && (
        <div className="grid gap-4 md:grid-cols-3">
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm text-muted-foreground">{t('resources.cpu')}</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{latest.cpu_percent.toFixed(1)}%</div>
              <div className="mt-2 h-2 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full transition-all"
                  style={{
                    width: `${latest.cpu_percent}%`,
                    backgroundColor: sevColor(latest.cpu_percent),
                  }}
                />
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm text-muted-foreground">
                {t('resources.memory')}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{latest.memory_percent.toFixed(1)}%</div>
              <div className="mt-2 h-2 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full transition-all"
                  style={{
                    width: `${latest.memory_percent}%`,
                    backgroundColor: sevColor(latest.memory_percent),
                  }}
                />
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm text-muted-foreground">{t('resources.disk')}</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{latest.disk_percent.toFixed(1)}%</div>
              <div className="mt-2 h-2 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full transition-all"
                  style={{
                    width: `${latest.disk_percent}%`,
                    backgroundColor: sevColor(latest.disk_percent),
                  }}
                />
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Chart */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Activity className="h-4 w-4" /> {t('resources.resourceHistory')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          {chartData.length > 1 ? (
            <ResponsiveContainer width="100%" height={300}>
              <AreaChart data={chartData}>
                <CartesianGrid strokeDasharray="3 3" className="stroke-border" />
                <XAxis dataKey="time" className="text-xs" tick={{ fontSize: 12 }} />
                <YAxis className="text-xs" tick={{ fontSize: 12 }} domain={[0, 100]} />
                <RechartsTooltip
                  contentStyle={{
                    backgroundColor: 'var(--card)',
                    border: '1px solid var(--border)',
                    borderRadius: '8px',
                    fontSize: '12px',
                    color: 'var(--foreground)',
                  }}
                />
                <Legend wrapperStyle={{ fontSize: 12 }} />
                <ReferenceLine
                  y={75}
                  stroke={getChartColor('--warning')}
                  strokeDasharray="4 4"
                  label={{
                    value: '75%',
                    position: 'right',
                    fontSize: 10,
                    fill: getChartColor('--warning'),
                  }}
                />
                <ReferenceLine
                  y={90}
                  stroke={getChartColor('--error')}
                  strokeDasharray="4 4"
                  label={{
                    value: '90%',
                    position: 'right',
                    fontSize: 10,
                    fill: getChartColor('--error'),
                  }}
                />
                <Area
                  type="monotone"
                  dataKey="cpu"
                  stroke={getChartColor('--chart-1')}
                  fill={getChartColor('--chart-1')}
                  fillOpacity={0.1}
                  name={`${t('resources.cpu')} %`}
                />
                <Area
                  type="monotone"
                  dataKey="memory"
                  stroke={getChartColor('--chart-2')}
                  fill={getChartColor('--chart-2')}
                  fillOpacity={0.1}
                  name={`${t('resources.memory')} %`}
                />
                <Area
                  type="monotone"
                  dataKey="disk"
                  stroke={getChartColor('--chart-3')}
                  fill={getChartColor('--chart-3')}
                  fillOpacity={0.1}
                  name={`${t('resources.disk')} %`}
                />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <p className="text-sm text-muted-foreground text-center py-8">
              {t('resources.notEnoughData')}
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
