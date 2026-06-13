import { Link } from '@tanstack/react-router'
import { Bot } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ErrorState } from '@/components/shared/error-state'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { Agent } from '@/types'
import { LiveActivityFeed } from './live-activity-feed'

const MAX_AGENTS_VISIBLE = 8

export interface AgentsActivityCardProps {
  runningAgents: Agent[]
  isAgentsError: boolean
  onRetryAgents: () => void
}

/**
 * Combined "Agents & Activity" card.
 *
 * Layout (desktop, ≥1024px): side-by-side, agents list (2/5) on the
 * left, live activity feed (3/5) on the right. Uses viewport-relative
 * min-height instead of the old hardcoded 300px.
 *
 * Layout (mobile/tablet, <1024px): tabbed view. Toggling tabs only
 * toggles CSS `hidden` classes, so the activity feed's pause state,
 * filter selection, and scroll position are preserved (no remount).
 */
export function AgentsActivityCard({
  runningAgents,
  isAgentsError,
  onRetryAgents,
}: AgentsActivityCardProps) {
  const { t } = useTranslation()
  const [tab, setTab] = useState<'agents' | 'activity'>('agents')

  return (
    <Card className="flex h-full flex-col">
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Bot className="h-4 w-4" />
          {t('dashboard.agentsAndActivity')}
        </CardTitle>
        {/* Mobile/tablet tab switcher */}
        <div className="flex lg:hidden items-center rounded-md border bg-muted/30 p-0.5">
          <button
            type="button"
            onClick={() => setTab('agents')}
            className={cn(
              'px-2.5 py-0.5 text-xs rounded-sm transition-colors',
              tab === 'agents'
                ? 'bg-background text-foreground shadow-sm'
                : 'text-muted-foreground hover:text-foreground',
            )}
            aria-pressed={tab === 'agents'}
          >
            {t('dashboard.agentsTab')}
          </button>
          <button
            type="button"
            onClick={() => setTab('activity')}
            className={cn(
              'px-2.5 py-0.5 text-xs rounded-sm transition-colors',
              tab === 'activity'
                ? 'bg-background text-foreground shadow-sm'
                : 'text-muted-foreground hover:text-foreground',
            )}
            aria-pressed={tab === 'activity'}
          >
            {t('dashboard.activityTab')}
          </button>
        </div>
      </CardHeader>
      <CardContent className="flex-1 pt-0 min-h-[200px] lg:min-h-[320px]">
        <div className="flex h-full gap-4">
          {/* Agents list — hidden by default, shown on its tab or always on desktop. */}
          <div className={cn('w-2/5 min-w-0', tab === 'activity' ? 'hidden' : 'block', 'lg:block')}>
            <AgentsList agents={runningAgents} isError={isAgentsError} onRetry={onRetryAgents} />
          </div>
          {/* Live activity feed (bare — parent Card wraps both). */}
          <div
            className={cn(
              'flex-1 min-w-0',
              tab === 'agents' ? 'hidden' : 'block',
              'lg:block lg:h-full',
            )}
          >
            <LiveActivityFeed variant="bare" />
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

function AgentsList({
  agents,
  isError,
  onRetry,
}: {
  agents: Agent[]
  isError: boolean
  onRetry: () => void
}) {
  const { t } = useTranslation()
  if (isError) {
    return <ErrorState onRetry={onRetry} />
  }
  if (agents.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-2 py-8 text-center">
        <Bot className="h-8 w-8 text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">{t('dashboard.noActiveAgents')}</p>
        <Link
          to="/seeds"
          className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
        >
          {t('dashboard.onboarding.seed')}
        </Link>
      </div>
    )
  }
  return (
    <div className="h-full overflow-y-auto pr-1">
      <div className="space-y-1.5">
        {agents.slice(0, MAX_AGENTS_VISIBLE).map((agent) => (
          <Link
            key={agent.id}
            to="/agents/$agentId"
            params={{ agentId: agent.id }}
            className="flex items-center justify-between rounded-md border px-3 py-2 hover:bg-accent hover:border-primary/20 hover:shadow-sm transition-all"
          >
            <div className="flex items-center gap-2 min-w-0">
              <Bot className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
              <span className="text-sm font-medium truncate">{agent.name}</span>
            </div>
            <span className="text-2xs text-muted-foreground font-mono">{agent.id.slice(0, 6)}</span>
          </Link>
        ))}
        {agents.length > MAX_AGENTS_VISIBLE && (
          <Link
            to="/agents"
            className="block text-center text-xs text-muted-foreground hover:text-foreground pt-1"
          >
            {t('dashboard.viewAllCount', { count: agents.length })}
          </Link>
        )}
      </div>
    </div>
  )
}
