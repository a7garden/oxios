import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import {
  Activity,
  AlertTriangle,
  Bot,
  Brain,
  Calendar,
  Clock,
  Cpu,
  LayoutDashboard,
  MessageSquare,
  NotebookPen,
  Shield,
  Zap,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ApprovalsQueue } from '@/components/dashboard/approvals-queue'
import { LiveActivityFeed } from '@/components/dashboard/live-activity-feed'
import { ModelUsageCard } from '@/components/dashboard/model-usage-card'
import { StatCard } from '@/components/dashboard/stat-card'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Card, CardHeader, CardTitle } from '@/components/ui/card'
import { useAgentCountHistory } from '@/hooks/use-agent-count-history'
import { useApprovals } from '@/hooks/use-approvals'
import { computeDelta, seriesFromSnapshots, useResourceHistory } from '@/hooks/use-resource-history'
import { useTokenRate } from '@/hooks/use-token-rate'
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
    refetchInterval: 10_000,
  })

  const {
    data: agents,
    isError: agentsError,
    refetch: refetchAgents,
  } = useQuery({
    queryKey: ['agents'],
    queryFn: () => api.get<{ items: Agent[] }>('/api/agents'),
    refetchInterval: 5_000,
  })

  // Resource history (last 30 samples) → used for sparkline + CPU card
  const { data: snapshots } = useResourceHistory(30, 10_000)
  const cpuSeries = seriesFromSnapshots(snapshots ?? [], 'cpu_percent')
  const cpuDelta = computeDelta(cpuSeries)

  // Token rate from the SSE stream
  const { tokensPerMin, history: tokenHistory } = useTokenRate()
  const tokenDelta = computeDelta(tokenHistory)

  // Pending approvals (for KPI count)
  const { data: approvals } = useApprovals()
  const pendingApprovals = (approvals?.items ?? []).filter((a) => a.status === 'pending')

  // Derived data for KPI cards. Computed before the early returns so
  // hook order is stable (React requires hooks to be called in the
  // same order on every render).
  const allAgents = agents?.items ?? []
  const runningAgents = allAgents.filter((a) => a.status?.toLowerCase() === 'running')
  // `total_forked` is the authoritative total from the status endpoint.
  // If the field is missing, fall back to null (UI shows "?" with a
  // tooltip) instead of silently using the polled running-agents list,
  // which would be misleading.
  const totalForked: number | null =
    typeof status?.components?.agents?.total_forked === 'number'
      ? status.components.agents.total_forked
      : null
  const { totalSeries, runningSeries } = useAgentCountHistory(totalForked, runningAgents.length)

  if (statusLoading) return <LoadingCards count={5} />
  if (statusError) return <ErrorState onRetry={() => refetchStatus()} />

  const quickLinks = [
    {
      labelKey: 'common.chat',
      href: '/chat',
      icon: <MessageSquare className="h-5 w-5 text-info" />,
      descKey: 'dashboard.startConversation',
    },
    {
      labelKey: 'common.knowledge',
      href: '/knowledge',
      icon: <NotebookPen className="h-5 w-5 text-violet-500" />,
      descKey: 'dashboard.markdownNotesJournal',
    },
    {
      labelKey: 'common.agents',
      href: '/agents',
      icon: <Bot className="h-5 w-5 text-success" />,
      descKey: 'dashboard.manageRunningAgents',
    },
    {
      labelKey: 'common.sessions',
      href: '/sessions',
      icon: <Clock className="h-5 w-5 text-info" />,
      descKey: 'dashboard.viewSessionHistory',
    },
    {
      labelKey: 'common.resources',
      href: '/resources',
      icon: <Activity className="h-5 w-5 text-warning" />,
      descKey: 'dashboard.systemResourceUsage',
    },
    {
      labelKey: 'common.memory',
      href: '/memory',
      icon: <Brain className="h-5 w-5 text-purple-500" />,
      descKey: 'dashboard.agentMemoryStore',
    },
    {
      labelKey: 'common.security',
      href: '/security',
      icon: <Shield className="h-5 w-5 text-error" />,
      descKey: 'dashboard.auditTrailAccessControl',
    },
    {
      labelKey: 'common.scheduler',
      href: '/scheduler',
      icon: <Calendar className="h-5 w-5 text-teal-500" />,
      descKey: 'dashboard.taskQueueManagement',
    },
  ]

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">{t('dashboard.title')}</h1>
        <p className="text-muted-foreground">{t('dashboard.subtitle')}</p>
      </div>

      {/* KPI stat row (5 cards) */}
      <div className="grid gap-3 grid-cols-2 md:grid-cols-3 lg:grid-cols-5">
        <StatCard
          label={t('dashboard.totalAgents')}
          value={totalForked ?? '?'}
          icon={<Bot className="h-4 w-4" />}
          iconClassName="text-info"
          sparkline={totalSeries}
          sparkColor="blue"
          hint={t('dashboard.forkedTotal')}
          {...(totalForked === null
            ? {
                title: t('dashboard.totalForkedUnavailable'),
              }
            : {})}
        />
        <StatCard
          label={t('dashboard.runningAgents')}
          value={runningAgents.length}
          icon={<Activity className="h-4 w-4" />}
          iconClassName="text-success"
          sparkline={runningSeries}
          sparkColor="emerald"
          href="/agents"
        />
        <StatCard
          label={t('dashboard.tokensPerMin')}
          value={formatTokensPerMin(tokensPerMin)}
          icon={<Zap className="h-4 w-4" />}
          iconClassName="text-violet-500"
          delta={tokenHistory.length > 1 ? tokenDelta : undefined}
          sparkline={tokenHistory}
          sparkColor="violet"
          hint={t('dashboard.lastWindow')}
        />
        <StatCard
          label={t('dashboard.cpu')}
          value={
            cpuSeries.length > 0 ? `${(cpuSeries[cpuSeries.length - 1] ?? 0).toFixed(0)}%` : '—'
          }
          icon={<Cpu className="h-4 w-4" />}
          iconClassName="text-warning"
          delta={cpuSeries.length > 1 ? cpuDelta : undefined}
          sparkline={cpuSeries}
          sparkColor="amber"
          href="/resources"
        />
        <StatCard
          label={t('dashboard.pendingApprovals')}
          value={pendingApprovals.length}
          icon={<AlertTriangle className="h-4 w-4" />}
          iconClassName={pendingApprovals.length > 0 ? 'text-error' : 'text-muted-foreground'}
          sparkColor={pendingApprovals.length > 0 ? 'red' : 'cyan'}
          hint={
            pendingApprovals.length > 0 ? t('dashboard.needsAttention') : t('dashboard.allClear')
          }
          href="/approvals"
        />
      </div>

      {/* Two-column row: Live Activity Feed + Active Agents list */}
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <LiveActivityFeed />
        </div>
        <div>
          <ActiveAgentsCard
            agents={runningAgents}
            isError={agentsError}
            onRetry={() => refetchAgents()}
          />
        </div>
      </div>

      {/* Approvals Queue (hidden when 0 pending) */}
      <ApprovalsQueue />

      {/* System Health (lightweight, secondary) */}
      <SystemHealthCard status={status} />

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

function ActiveAgentsCard({
  agents,
  isError,
  onRetry,
}: {
  agents: Agent[]
  isError: boolean
  onRetry: () => void
}) {
  const { t } = useTranslation()
  return (
    <Card className="h-full">
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <LayoutDashboard className="h-4 w-4" />
          {t('dashboard.activeAgents')}
        </CardTitle>
        <span className="text-xs text-muted-foreground">{agents.length}</span>
      </CardHeader>
      <div className="px-6 pb-6 pt-0">
        {isError ? (
          <ErrorState onRetry={onRetry} />
        ) : agents.length === 0 ? (
          <p className="text-sm text-muted-foreground py-3">{t('dashboard.noActiveAgents')}</p>
        ) : (
          <div className="space-y-1.5">
            {agents.slice(0, 5).map((agent) => (
              <Link
                key={agent.id}
                to="/agents/$agentId"
                params={{ agentId: agent.id }}
                className="flex items-center justify-between rounded-md border px-3 py-2 hover:bg-accent/40 transition-colors"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <Bot className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                  <span className="text-sm font-medium truncate">{agent.name}</span>
                </div>
                <span className="text-2xs text-muted-foreground font-mono">
                  {agent.id.slice(0, 6)}
                </span>
              </Link>
            ))}
            {agents.length > 5 && (
              <Link
                to="/agents"
                className="block text-center text-xs text-muted-foreground hover:text-foreground pt-1"
              >
                {t('dashboard.viewAllCount', { count: agents.length })}
              </Link>
            )}
          </div>
        )}
      </div>
    </Card>
  )
}

function SystemHealthCard({ status }: { status: SystemStatus | undefined }) {
  const { t } = useTranslation()
  if (!status) return null
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Shield className="h-4 w-4" />
          {t('dashboard.systemHealth')}
        </CardTitle>
        <span className="text-xs font-mono text-muted-foreground">{status.version}</span>
      </CardHeader>
      <div className="px-6 pb-6 pt-0">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 text-sm">
          {status.components?.state_store && (
            <HealthRow
              labelKey="dashboard.stateStore"
              healthy={status.components.state_store.healthy}
              detail={status.components.state_store.detail}
            />
          )}
          {status.components?.event_bus && (
            <HealthRow
              labelKey="dashboard.eventBus"
              healthy={status.components.event_bus.healthy}
              detail={status.components.event_bus.detail}
            />
          )}
          {status.components?.memory && (
            <HealthRow
              labelKey="dashboard.memory"
              healthy={status.components.memory.enabled}
              detail={t('dashboard.entriesIndexed', { count: status.components.memory.index_size })}
            />
          )}
          <div className="flex items-center gap-2 text-muted-foreground">
            <Clock className="h-3.5 w-3.5" />
            <span className="truncate">{status.uptime}</span>
          </div>
        </div>
      </div>
    </Card>
  )
}

function HealthRow({
  labelKey,
  healthy,
  detail,
}: {
  labelKey: string
  healthy: boolean
  detail?: string | null
}) {
  const { t } = useTranslation()
  return (
    <div className="flex items-center gap-2 text-muted-foreground">
      <div className={`h-2 w-2 rounded-full ${healthy ? 'bg-success' : 'bg-error'}`} />
      <span className="text-foreground">{t(labelKey)}</span>
      {detail && <span className="text-xs truncate">· {detail}</span>}
    </div>
  )
}

function formatTokensPerMin(n: number): string {
  if (n <= 0) return '0'
  if (n < 1000) return `${Math.round(n)}`
  if (n < 10_000) return `${(n / 1000).toFixed(1)}k`
  if (n < 1_000_000) return `${Math.round(n / 1000)}k`
  return `${(n / 1_000_000).toFixed(1)}M`
}
