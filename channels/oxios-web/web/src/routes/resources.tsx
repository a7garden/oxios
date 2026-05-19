import { createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { LoadingCards } from '@/components/shared/loading'
import { Activity, RefreshCw } from 'lucide-react'
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip as RechartsTooltip, ResponsiveContainer } from 'recharts'
import type { ResourceSnapshot } from '@/types'

export const Route = createFileRoute('/resources')({ component: ResourcesPage })

function ResourcesPage() {
  const { data, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['resources'],
    queryFn: () => api.get<ResourceSnapshot[]>('/api/resources'),
    refetchInterval: 5000,
  })

  if (isLoading) return <LoadingCards count={4} />

  const snapshots = data ?? []
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
          <h1 className="text-2xl font-bold">Resources</h1>
          <p className="text-muted-foreground">System resource monitoring</p>
        </div>
        <button
          onClick={() => refetch()}
          disabled={isFetching}
          className="rounded-md p-2 hover:bg-muted"
        >
          <RefreshCw className={`h-4 w-4 ${isFetching ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {/* Current Stats */}
      {latest && (
        <div className="grid gap-4 md:grid-cols-3">
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm text-muted-foreground">CPU</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{latest.cpu_percent.toFixed(1)}%</div>
              <div className="mt-2 h-2 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full bg-blue-500 transition-all"
                  style={{ width: `${latest.cpu_percent}%` }}
                />
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm text-muted-foreground">Memory</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{latest.memory_percent.toFixed(1)}%</div>
              <div className="mt-2 h-2 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full bg-purple-500 transition-all"
                  style={{ width: `${latest.memory_percent}%` }}
                />
              </div>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm text-muted-foreground">Disk</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{latest.disk_percent.toFixed(1)}%</div>
              <div className="mt-2 h-2 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full bg-amber-500 transition-all"
                  style={{ width: `${latest.disk_percent}%` }}
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
            <Activity className="h-4 w-4" /> Resource History
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
                    backgroundColor: 'hsl(var(--card))',
                    border: '1px solid hsl(var(--border))',
                    borderRadius: '8px',
                    fontSize: '12px',
                  }}
                />
                <Area type="monotone" dataKey="cpu" stroke="#3b82f6" fill="#3b82f6" fillOpacity={0.1} name="CPU %" />
                <Area type="monotone" dataKey="memory" stroke="#a855f7" fill="#a855f7" fillOpacity={0.1} name="Memory %" />
                <Area type="monotone" dataKey="disk" stroke="#f59e0b" fill="#f59e0b" fillOpacity={0.1} name="Disk %" />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <p className="text-sm text-muted-foreground text-center py-8">
              Not enough data to display chart. Data is collected over time.
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
