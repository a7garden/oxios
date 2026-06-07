import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import {
  AlertTriangle,
  ArrowRight,
  Brain,
  Cpu,
  Dna,
  HardDrive,
  MessageSquare,
  NotebookPen,
  Sparkles,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { AgentStatusCard } from '@/components/dashboard/agent-status-card'
import { AgentsActivityCard } from '@/components/dashboard/agents-activity-card'
import { ApprovalsQueue } from '@/components/dashboard/approvals-queue'
import { StatCard } from '@/components/dashboard/stat-card'
import { SystemHealthCard } from '@/components/dashboard/system-health-card'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
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

  // Resource history (last 30 samples) → sparklines
  const { data: snapshots } = useResourceHistory(30, 10_000)
  const cpuSeries = seriesFromSnapshots(snapshots ?? [], 'cpu_percent')
  const memSeries = seriesFromSnapshots(snapshots ?? [], 'memory_percent')
  const cpuDelta = computeDelta(cpuSeries)
  const memDelta = computeDelta(memSeries)

  // Token rate from the SSE stream
  const { tokensPerMin, history: tokenHistory } = useTokenRate()
  const tokenDelta = computeDelta(tokenHistory)

  // Pending approvals
  const { data: approvals } = useApprovals()
  const pendingApprovals = (approvals?.items ?? []).filter((a) => a.status === 'pending')

  // Derived data — computed before early returns for stable hook order
  const allAgents = agents?.items ?? []
  const runningAgents = allAgents.filter((a) => a.status?.toLowerCase() === 'running')
  const totalForked: number | null =
    typeof status?.components?.agents?.total_forked === 'number'
      ? status.components.agents.total_forked
      : null
  const totalFailed: number =
    typeof status?.components?.agents?.total_failed === 'number'
      ? status.components.agents.total_failed
      : 0
  const { runningSeries } = useAgentCountHistory(totalForked, runningAgents.length, {
    trackTotal: false,
  })

  // Determine empty state — no running agents AND no pending approvals
  const isEmpty = runningAgents.length === 0 && pendingApprovals.length === 0

  if (statusLoading) return <LoadingCards count={6} />
  if (statusError) return <ErrorState onRetry={() => refetchStatus()} />

  return (
    <div className="space-y-6">
      {/* Title */}
      <div>
        <h1 className="text-2xl font-bold">{t('dashboard.title')}</h1>
        <p className="text-muted-foreground">{t('dashboard.subtitle')}</p>
      </div>

      {/* KPI Row — 5 cards (no duplicate "Running" count)
       *  AgentStatus already shows running/total/failed as a fraction.
       *  Replaced the old "Running Agents" card with "Tokens/min" moved
       *  earlier and added a "Memory" card (engine, not storage) for
       *  system coverage. */}
      <div className="grid gap-3 grid-cols-2 md:grid-cols-3 xl:grid-cols-5">
        <AgentStatusCard
          total={totalForked}
          running={runningAgents.length}
          failed={totalFailed}
          runningSeries={runningSeries}
        />
        <StatCard
          label={t('dashboard.tokensPerMin')}
          value={formatTokensPerMin(tokensPerMin)}
          icon={<Sparkles className="h-4 w-4" />}
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
          label={t('dashboard.ram')}
          value={
            memSeries.length > 0 ? `${(memSeries[memSeries.length - 1] ?? 0).toFixed(0)}%` : '—'
          }
          icon={<HardDrive className="h-4 w-4" />}
          iconClassName="text-rose-500"
          delta={memSeries.length > 1 ? memDelta : undefined}
          sparkline={memSeries}
          sparkColor="rose"
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

      {/* Row 2: Agents & Activity (2/3) + Right column (1/3) */}
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <AgentsActivityCard
            runningAgents={runningAgents}
            isAgentsError={agentsError}
            onRetryAgents={() => refetchAgents()}
          />
        </div>
        <div className="space-y-4">
          <SystemHealthCard status={status} />
          <ApprovalsQueue />
        </div>
      </div>

      {/* Row 3: Onboarding quick-start (only when system is idle) */}
      {isEmpty && <OnboardingQuickStart />}
    </div>
  )
}

/**
 * Onboarding quick-start card.
 *
 * Shown when there are no running agents AND no pending approvals.
 * Provides clear CTAs for the most common first actions.
 * Hidden automatically once the system has activity.
 */
function OnboardingQuickStart() {
  const { t } = useTranslation()

  const items = [
    {
      icon: <MessageSquare className="h-5 w-5" />,
      labelKey: 'dashboard.onboarding.chat',
      descKey: 'dashboard.onboarding.chatDesc',
      href: '/chat',
      color: 'text-blue-500',
      bg: 'bg-blue-500/10',
    },
    {
      icon: <Dna className="h-5 w-5" />,
      labelKey: 'dashboard.onboarding.seed',
      descKey: 'dashboard.onboarding.seedDesc',
      href: '/seeds',
      color: 'text-emerald-500',
      bg: 'bg-emerald-500/10',
    },
    {
      icon: <NotebookPen className="h-5 w-5" />,
      labelKey: 'dashboard.onboarding.knowledge',
      descKey: 'dashboard.onboarding.knowledgeDesc',
      href: '/knowledge',
      color: 'text-amber-500',
      bg: 'bg-amber-500/10',
    },
    {
      icon: <Brain className="h-5 w-5" />,
      labelKey: 'dashboard.onboarding.memory',
      descKey: 'dashboard.onboarding.memoryDesc',
      href: '/memory',
      color: 'text-violet-500',
      bg: 'bg-violet-500/10',
    },
  ]

  return (
    <div className="rounded-xl border border-dashed border-primary/20 bg-primary/[0.02] p-6">
      <div className="mb-4">
        <h2 className="text-lg font-semibold">{t('dashboard.onboarding.title')}</h2>
        <p className="text-sm text-muted-foreground">{t('dashboard.onboarding.subtitle')}</p>
      </div>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {items.map((item) => (
          <Link
            key={item.href}
            to={item.href}
            className="group flex items-start gap-3 rounded-lg border bg-card p-4 transition-colors hover:bg-accent/40 hover:border-primary/20"
          >
            <div className={`rounded-lg p-2 ${item.bg}`}>
              <div className={item.color}>{item.icon}</div>
            </div>
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-1 text-sm font-medium">
                {t(item.labelKey)}
                <ArrowRight className="h-3 w-3 opacity-0 transition-opacity group-hover:opacity-100" />
              </div>
              <p className="mt-0.5 text-xs text-muted-foreground line-clamp-2">
                {t(item.descKey)}
              </p>
            </div>
          </Link>
        ))}
      </div>
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
