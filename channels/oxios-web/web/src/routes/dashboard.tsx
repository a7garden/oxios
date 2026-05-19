import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Activity, Bot, Boxes, Clock, Cpu, Zap } from 'lucide-react'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Agent, SystemStatus } from '@/types'

export const Route = createFileRoute('/dashboard')({
  component: DashboardPage,
})

function DashboardPage() {
  const { data: status, isLoading: statusLoading } = useQuery({
    queryKey: ['status'],
    queryFn: () => api.get<SystemStatus>('/api/status'),
    refetchInterval: 10000,
  })

  const { data: agents } = useQuery({
    queryKey: ['agents'],
    queryFn: () => api.get<{ items: Agent[] }>('/api/agents'),
    refetchInterval: 5000,
  })

  if (statusLoading) return <LoadingCards count={4} />

  const stats = [
    {
      label: 'Running Agents',
      value: status?.agents_running ?? 0,
      icon: <Bot className="h-4 w-4" />,
      color: 'text-emerald-500',
    },
    {
      label: 'Total Agents',
      value: status?.agents_total ?? 0,
      icon: <Cpu className="h-4 w-4" />,
      color: 'text-blue-500',
    },
    {
      label: 'Active Spaces',
      value: status?.spaces_active ?? 0,
      icon: <Boxes className="h-4 w-4" />,
      color: 'text-purple-500',
    },
    {
      label: 'Uptime',
      value: status?.uptime_ms ? `${Math.floor(status.uptime_ms / 3600000)}h` : '-',
      icon: <Clock className="h-4 w-4" />,
      color: 'text-amber-500',
    },
  ]

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Dashboard</h1>
        <p className="text-muted-foreground">Oxios Agent OS overview</p>
      </div>

      {/* Stats Grid */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => (
          <Card key={stat.label}>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-muted-foreground">
                {stat.label}
              </CardTitle>
              <div className={stat.color}>{stat.icon}</div>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{stat.value}</div>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Active Agents */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Activity className="h-4 w-4" /> Active Agents
          </CardTitle>
        </CardHeader>
        <CardContent>
          {agents?.items?.length ? (
            <div className="space-y-2">
              {agents.items
                .filter((a) => a.status === 'running')
                .map((agent) => (
                  <div
                    key={agent.id}
                    className="flex items-center justify-between rounded-lg border p-3"
                  >
                    <div className="flex items-center gap-3">
                      <Bot className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="font-medium">{agent.name}</p>
                        <p className="text-xs text-muted-foreground">
                          ID: {agent.id.slice(0, 8)}...
                        </p>
                      </div>
                    </div>
                    <Badge variant="success">Running</Badge>
                  </div>
                ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">No active agents</p>
          )}
        </CardContent>
      </Card>

      {/* Version */}
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Zap className="h-4 w-4" />
        <span>Version {status?.version ?? 'unknown'}</span>
      </div>
    </div>
  )
}
