import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { Activity } from 'lucide-react'
import { TraceStepCard } from './trace-step'
import type { AgentTrace } from '@/types/agent'

interface ExecutionTraceProps {
  trace: AgentTrace | null | undefined
  isLoading?: boolean
}

export function ExecutionTrace({ trace, isLoading }: ExecutionTraceProps) {
  const { t } = useTranslation()

  if (isLoading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="h-16 bg-muted animate-pulse rounded-lg" />
        ))}
      </div>
    )
  }

  if (!trace || !trace.steps?.length) {
    return (
      <EmptyState
        icon={<Activity className="h-10 w-10" />}
        title={t('agents.noTrace')}
        description={t('agents.noTraceDescription')}
      />
    )
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <span className="text-sm text-muted-foreground">
          {t('agents.steps', { count: trace.steps.length })}
        </span>
      </div>
      <div className="space-y-2">
        {trace.steps.map((step) => (
          <TraceStepCard key={step.index} step={step} />
        ))}
      </div>
    </div>
  )
}
