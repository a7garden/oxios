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
import { useTranslation } from 'react-i18next'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { ModelUsageCard } from '@/components/dashboard/model-usage-card'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Agent, SystemStatus } from '@/types'

export const Route = createFileRoute('/')({
  component: DashboardPage,
})

function DashboardPage() {
  const { t } = useTranslation()

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
      labelKey: 'dashboard.runningAgents',
      value: status?.components?.agents?.active_count ?? 0,
      icon: <Bot className="h-4 w-4" />,
      color: 'text-emerald-500',
    },
    {
      labelKey: 'dashboard.totalAgents',
      value: status?.components?.agents?.total_forked ?? 0,
      icon: <Cpu className="h-4 w-4" />,
      color: 'text-blue-500',
    },
    {
      labelKey: 'dashboard.activeSpaces',
      value: status?.components?.spaces_active ?? 0,
      icon: <LayoutDashboard className="h-4 w-4" />,
      color: 'text-purple-500',
    },
    {
      labelKey: 'dashboard.uptime',
      value: status?.uptime ?? '-',
      icon: <Clock className="h-4 w-4" />,
      color: 'text-amber-500',
    },
  ]

  const quickLinks = [
    { labelKey: 'common.chat', href: '/chat', icon: <MessageSquare className="h-5 w-5 text-blue-500" />, descKey: 'dashboard.startConversation' },
    { labelKey: 'common.knowledge', href: '/knowledge', icon: <NotebookPen className="h-5 w-5 text-violet-500" />, descKey: 'dashboard.markdownNotesJournal' },
    { labelKey: 'common.agents', href: '/agents', icon: <Bot className="h-5 w-5 text-emerald-500" />, descKey: 'dashboard.manageRunningAgents' },
    { labelKey: 'common.sessions', href: '/sessions', icon: <Clock className="h-5 w-5 text-blue-500" />, descKey: 'dashboard.viewSessionHistory' },
    { labelKey: 'common.resources', href: '/resources', icon: <Activity className="h-5 w-5 text-amber-500" />, descKey: 'dashboard.systemResourceUsage' },
    { labelKey: 'common.memory', href: '/memory', icon: <Brain className="h-5 w-5 text-purple-500" />, descKey: 'dashboard.agentMemoryStore' },
    { labelKey: 'common.security', href: '/security', icon: <Shield className="h-5 w-5 text-red-500" />, descKey: 'dashboard.auditTrailAccessControl' },
    { labelKey: 'common.scheduler', href: '/scheduler', icon: <Calendar className="h-5 w-5 text-teal-500" />, descKey: 'dashboard.taskQueueManagement' },
  ]

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">{t('dashboard.title')}</h1>
        <p className="text-muted-foreground">{t('dashboard.subtitle')}</p>
      </div>

      {/* Stats Grid */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => (
          <Card key={stat.labelKey}>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
              <CardTitle className="text-sm font-medium text-muted-foreground">
                {t(stat.labelKey)}
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
              <Activity className="h-4 w-4" /> {t('dashboard.activeAgents')}
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
                    <Badge variant="success">{t('common.running')}</Badge>
                  </Link>
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground py-2">{t('dashboard.noActiveAgents')}</p>
            )}
          </CardContent>
        </Card>

        {/* System Health Summary */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield className="h-4 w-4" /> {t('dashboard.systemHealth')}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {status?.components?.state_store && (
                <HealthRow
                  labelKey="dashboard.stateStore"
                  healthy={status.components.state_store.healthy}
                  detail={status.components.state_store.detail}
                />
              )}
              {status?.components?.event_bus && (
                <HealthRow
                  labelKey="dashboard.eventBus"
                  healthy={status.components.event_bus.healthy}
                  detail={status.components.event_bus.detail}
                />
              )}
              {status?.components?.memory && (
                <HealthRow
                  labelKey="dashboard.memory"
                  healthy={status.components.memory.enabled}
                  detail={t('dashboard.entriesIndexed', { count: status.components.memory.index_size })}
                />
              )}
              <div className="flex items-center justify-between text-sm">
                <span className="text-muted-foreground">{t('dashboard.version')}</span>
                <span className="font-mono">{status?.version ?? 'unknown'}</span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Model Usage — routing stats (RFC-011) */}
      <ModelUsageCard />

      {/* Quick Links — 2x4 grid */}
      <div>
        <h2 className="text-lg font-semibold mb-3">{t('dashboard.quickLinks')}</h2>
        <div className="grid gap-3 grid-cols-2 md:grid-cols-4">
          {quickLinks.map((link) => (
            <Link key={link.href} to={link.href}>
              <Card className="hover:bg-accent/50 transition-colors cursor-pointer group h-full">
                <CardHeader className="flex flex-row items-center gap-3 pb-2">
                  {link.icon}
                  <div>
                    <CardTitle className="text-sm font-medium">{t(link.labelKey)}</CardTitle>
                    <p className="text-xs text-muted-foreground mt-0.5">{t(link.descKey)}</p>
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

function HealthRow({ labelKey, healthy, detail }: { labelKey: string; healthy: boolean; detail?: string | null }) {
  const { t } = useTranslation()

  return (
    <div className="flex items-center justify-between text-sm">
      <div className="flex items-center gap-2">
        <div className={`h-2 w-2 rounded-full ${healthy ? 'bg-emerald-500' : 'bg-red-500'}`} />
        <span>{t(labelKey)}</span>
      </div>
      <span className="text-xs text-muted-foreground">{detail ?? (healthy ? t('common.healthy') : t('common.unhealthy'))}</span>
    </div>
  )
}