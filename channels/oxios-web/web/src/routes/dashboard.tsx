import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { Activity, Bot, Boxes, Brain, Clock, Cpu, FileText, Zap } from 'lucide-react'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Agent, SystemStatus } from '@/types'

export const Route = createFileRoute('/dashboard')({
  component: DashboardPage,
})

function DashboardPage() {
  const {
    data: status,
    isLoading: statusLoading,
    isError: statusError,
    refetch: refetchStatus,
  } = useQuery({
    queryKey: ['status'],
    queryFn: () => api.get<SystemStatus>('/api/status'),
    refetchInterval: 10000,
  })

  const {
    data: agents,
    isError: agentsError,
    refetch: refetchAgents,
  } = useQuery({
    queryKey: ['agents'],
    queryFn: () => api.get<{ items: Agent[] }>('/api/agents'),
    refetchInterval: 5000,
  })

  if (statusLoading) return <LoadingCards count={4} />
  if (statusError) return <ErrorState onRetry={() => refetchStatus()} />

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
          {agentsError ? (
            <ErrorState onRetry={() => refetchAgents()} />
          ) : agents?.items?.length ? (
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

      {/* Quick Links */}
      <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
        <Link to="/knowledge/">
          <Card className="hover:bg-accent/50 transition-colors cursor-pointer group">
            <CardHeader className="flex flex-row items-center gap-3 pb-2">
              <Brain className="h-5 w-5 text-violet-500" />
              <div>
                <CardTitle className="text-sm font-medium">Knowledge</CardTitle>
                <p className="text-xs text-muted-foreground mt-0.5">
                  Markdown notes, journal, chat
                </p>
              </div>
            </CardHeader>
          </Card>
        </Link>
        <Link to="/sessions/">
          <Card className="hover:bg-accent/50 transition-colors cursor-pointer group">
            <CardHeader className="flex flex-row items-center gap-3 pb-2">
              <Clock className="h-5 w-5 text-blue-500" />
              <div>
                <CardTitle className="text-sm font-medium">Sessions</CardTitle>
                <p className="text-xs text-muted-foreground mt-0.5">
                  View agent session history
                </p>
              </div>
            </CardHeader>
          </Card>
        </Link>
        <Link to="/workspace/">
          <Card className="hover:bg-accent/50 transition-colors cursor-pointer group">
            <CardHeader className="flex flex-row items-center gap-3 pb-2">
              <FileText className="h-5 w-5 text-emerald-500" />
              <div>
                <CardTitle className="text-sm font-medium">Workspace</CardTitle>
                <p className="text-xs text-muted-foreground mt-0.5">
                  Browse agent workspace files
                </p>
              </div>
            </CardHeader>
          </Card>
        </Link>
      </div>

      {/* Version */}
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Zap className="h-4 w-4" />
        <span>Version {status?.version ?? 'unknown'}</span>
      </div>
    </div>
  )
}
