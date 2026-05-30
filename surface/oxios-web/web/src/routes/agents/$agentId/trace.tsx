import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { ExecutionTrace } from '@/components/agent/execution-trace'
import { useAgentTrace } from '@/hooks/use-agent-trace'

export const Route = createFileRoute('/agents/$agentId/trace')({
  component: TracePage,
})

function TracePage() {
  const { t } = useTranslation()
  const { agentId } = Route.useParams()
  const navigate = useNavigate()
  const {
    data: trace,
    isLoading,
    isError,
    refetch,
  } = useAgentTrace(agentId)

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="icon"
          onClick={() =>
            navigate({ to: '/agents/$agentId', params: { agentId } })
          }
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <h1 className="text-2xl font-bold">{t('agents.executionTrace')}</h1>
        <span className="text-sm text-muted-foreground font-mono">{agentId}</span>
      </div>
      <ExecutionTrace trace={trace} />
    </div>
  )
}
