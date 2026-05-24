import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import {
  Activity,
  Bot,
  Brain,
  Calendar,
  Clock,
  Cpu,
  LayoutDashboard,
  MessageSquare,
  NotebookPen,
  Shield,
} from 'lucide-react'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Agent, SystemStatus } from '@/types'

export const Route = createFileRoute('/')({
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

  const runningAgents = agents?.items?.filter((a) => a.status?.toLowerCase() === 'running') ?? []

  const stats = [
    {
      label: 'Running Agents',
      value: status?.components?.agents?.active_count ?? 0,
      icon: <Bot className="h-4 w-4" />,
      color: 'text-emerald-500',
    },
    {
      label: 'Total Agents',
      value: status?.components?.agents?.total_forked ?? 0,
      icon: <Cpu className="h-4 w-4" />,
      color: 'text-blue-500',
    },
    {
      label: 'Active Spaces',
      value: status?.components?.spaces_active ?? 0,
      icon: <LayoutDashboard className="h-4 w-4" />,
      color: 'text-purple-500',
    },
    {
      label: 'Uptime',
      value: status?.uptime ?? '-',
      icon: <Clock className="h-4 w-4" />,
      color: 'text-amber-500',
    },
  ]

  const quickLinks = [
    { label: 'Chat', href: '/chat', icon: <MessageSquare className="h-5 w-5 text-blue-500" />, desc: 'Start a conversation' },
    { label: 'Knowledge', href: '/knowledge', icon: <NotebookPen className="h-5 w-5 text-violet-500" />, desc: 'Markdown notes & journal' },
    { label: 'Agents', href: '/agents', icon: <Bot className="h-5 w-5 text-emerald-500" />, desc: 'Manage running agents' },
    { label: 'Sessions', href: '/sessions', icon: <Clock className="h-5 w-5 text-blue-500" />, desc: 'View session history' },
    { label: 'Resources', href: '/resources', icon: <Activity className="h-5 w-5 text-amber-500" />, desc: 'System resource usage' },
    { label: 'Memory', href: '/memory', icon: <Brain className="h-5 w-5 text-purple-500" />, desc: 'Agent memory store' },
    { label: 'Security', href: '/security', icon: <Shield className="h-5 w-5 text-red-500" />, desc: 'Audit trail & access control' },
    { label: 'Scheduler', href: '/scheduler', icon: <Calendar className="h-5 w-5 text-teal-500" />, desc: 'Task queue management' },
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

      {/* Two-column: Active Agents + System Health */}
      <div className="grid gap-6 lg:grid-cols-2">
        {/* Active Agents */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Activity className="h-4 w-4" /> Active Agents
              {runningAgents.length > 0 && (
                <Badge variant="success" className="ml-1">{runningAgents.length}</Badge>
              )}
            </CardTitle>
          </CardHeader>
          <CardContent>
            {agentsError ? (
              <ErrorState onRetry={() => refetchAgents()} />
            ) : runningAgents.length > 0 ? (
              <div className="space-y-2">
                {runningAgents.map((agent) => (
                  <Link
                    key={agent.id}
                    to="/agents/$agentId"
                    params={{ agentId: agent.id }}
                    className="flex items-center justify-between rounded-lg border p-3 hover:bg-accent/50 transition-colors"
                  >
                    <div className="flex items-center gap-3">
                      <Bot className="h-4 w-4 text-muted-foreground" />
                      <div>
                        <p className="font-medium text-sm">{agent.name}</p>
                        <p className="text-xs text-muted-foreground">
                          ID: {agent.id.slice(0, 8)}...
                        </p>
                      </div>
                    </div>
                    <Badge variant="success">Running</Badge>
                  </Link>
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground py-2">No active agents</p>
            )}
          </CardContent>
        </Card>

        {/* System Health Summary */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield className="h-4 w-4" /> System Health
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {status?.components?.state_store && (
                <HealthRow
                  label="State Store"
                  healthy={status.components.state_store.healthy}
                  detail={status.components.state_store.detail}
                />
              )}
              {status?.components?.event_bus && (
                <HealthRow
                  label="Event Bus"
                  healthy={status.components.event_bus.healthy}
                  detail={status.components.event_bus.detail}
                />
              )}
              {status?.components?.memory && (
                <HealthRow
                  label="Memory"
                  healthy={status.components.memory.enabled}
                  detail={`${status.components.memory.index_size} entries indexed`}
                />
              )}
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">Version</span>
                <span className="font-mono">{status?.version ?? 'unknown'}</span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Quick Links — 2x4 grid */}
      <div>
        <h2 className="text-lg font-semibold mb-3">Quick Links</h2>
        <div className="grid gap-3 grid-cols-2 md:grid-cols-4">
          {quickLinks.map((link) => (
            <Link key={link.href} to={link.href}>
              <Card className="hover:bg-accent/50 transition-colors cursor-pointer group h-full">
                <CardHeader className="flex flex-row items-center gap-3 pb-2">
                  {link.icon}
                  <div>
                    <CardTitle className="text-sm font-medium">{link.label}</CardTitle>
                    <p className="text-xs text-muted-foreground mt-0.5">{link.desc}</p>
                  </div>
                </CardHeader>
              </Card>
            </Link>
          ))}
        </div>
      </div>
    </div>
  )
}

function HealthRow({ label, healthy, detail }: { label: string; healthy: boolean; detail?: string | null }) {
  return (
    <div className="flex items-center justify-between text-sm">
      <div className="flex items-center gap-2">
        <div className={`h-2 w-2 rounded-full ${healthy ? 'bg-emerald-500' : 'bg-red-500'}`} />
        <span>{label}</span>
      </div>
      <span className="text-xs text-muted-foreground">{detail ?? (healthy ? 'Healthy' : 'Unhealthy')}</span>
    </div>
  )
}
