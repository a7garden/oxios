import { useMutation, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Skull, ExternalLink } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { api } from '@/lib/api-client'
import { AgentHeader } from '@/components/agent/agent-header'
import { AgentBudgetBar } from '@/components/agent/agent-budget-bar'
import { ExecutionTrace } from '@/components/agent/execution-trace'
import { AgentLogs as AgentLogsComponent } from '@/components/agent/agent-logs'
import { useAgentDetail, useAgentTrace, useAgentLogs } from '@/hooks/use-agent-trace'

export const Route = createFileRoute('/agents/$agentId')({
  component: AgentDetailPage,
})

function AgentDetailPage() {
  const { t } = useTranslation()
  const { agentId } = Route.useParams()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const {
    data: agent,
    isLoading,
    isError,
    refetch,
  } = useAgentDetail(agentId)
  const { data: trace, isLoading: traceLoading } = useAgentTrace(agentId)
  const { data: logs } = useAgentLogs(agentId)

  const killMutation = useMutation({
    mutationFn: () => api.post(`/api/agents/${agentId}/kill`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] })
      queryClient.invalidateQueries({ queryKey: ['agents', 'detail', agentId] })
      navigate({ to: '/agents' })
    },
  })

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!agent) return <p className="text-muted-foreground">{t('agents.notFound')}</p>

  return (
    <div className="space-y-6">
      <AgentHeader agent={agent} onBack={() => navigate({ to: '/agents' })}>
        <Button
          variant="destructive"
          size="sm"
          onClick={() => {
            if (confirm(t('agents.terminateConfirm'))) killMutation.mutate()
          }}
          disabled={killMutation.isPending || agent.status?.toLowerCase() === 'stopped'}
        >
          <Skull className="h-4 w-4 mr-1" /> {t('agents.terminate')}
        </Button>
      </AgentHeader>

      {/* Meta info */}
      <Card>
        <CardContent className="pt-4 space-y-3">
          <div className="grid gap-2 md:grid-cols-3 text-sm">
            {agent.seed_id && (
              <div className="flex items-center gap-1">
                <span className="text-muted-foreground">{t('agents.seed')}:</span>
                <Button
                  variant="link"
                  className="h-auto p-0 text-xs"
                  onClick={() =>
                    navigate({
                      to: '/seeds/$seedId',
                      params: { seedId: agent.seed_id! },
                    })
                  }
                >
                  {agent.seed_id.slice(0, 8)}...
                </Button>
              </div>
            )}
            {agent.space_id && (
              <div className="flex items-center gap-1">
                <span className="text-muted-foreground">{t('agents.space')}:</span>
                <span className="text-xs font-mono">{agent.space_id.slice(0, 8)}...</span>
              </div>
            )}
            <div className="flex items-center gap-1">
              <span className="text-muted-foreground">{t('agents.created')}:</span>
              <span>{new Date(agent.created_at).toLocaleString()}</span>
            </div>
          </div>
          <AgentBudgetBar agent={agent} />
        </CardContent>
      </Card>

      {/* Tabs */}
      <Tabs defaultValue="trace" className="space-y-4">
        <TabsList>
          <TabsTrigger value="trace">{t('agents.trace')}</TabsTrigger>
          <TabsTrigger value="logs">{t('agents.logs')}</TabsTrigger>
        </TabsList>
        <TabsContent value="trace">
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-lg font-semibold">{t('agents.executionTrace')}</h3>
            <Button
              variant="ghost"
              size="sm"
              onClick={() =>
                navigate({
                  to: '/agents/$agentId/trace',
                  params: { agentId },
                })
              }
            >
              <ExternalLink className="h-4 w-4 mr-1" /> {t('agents.traceFullscreen')}
            </Button>
          </div>
          <ExecutionTrace trace={trace} isLoading={traceLoading} />
        </TabsContent>
        <TabsContent value="logs">
          <AgentLogsComponent logs={logs} />
        </TabsContent>
      </Tabs>
    </div>
  )
}
