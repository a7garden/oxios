import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, Bot, RotateCw, Skull } from 'lucide-react'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { StatusIndicator } from '@/components/shared/status-indicator'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Agent } from '@/types'

export const Route = createFileRoute('/agents/$agentId')({
  component: AgentDetailPage,
})

function AgentDetailPage() {
  const { agentId } = Route.useParams()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const {
    data: agent,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['agent', agentId],
    queryFn: () => api.get<Agent>(`/api/agents/${agentId}`),
    refetchInterval: 5000,
  })

  const killMutation = useMutation({
    mutationFn: () => api.post(`/api/agents/${agentId}/kill`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agent', agentId] })
      queryClient.invalidateQueries({ queryKey: ['agents'] })
    },
  })

  const restartMutation = useMutation({
    mutationFn: () => api.post(`/api/agents/${agentId}/restart`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agent', agentId] })
      queryClient.invalidateQueries({ queryKey: ['agents'] })
    },
  })

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!agent) return <p className="text-muted-foreground">Agent not found.</p>

  const details = [
    { label: 'ID', value: agent.id },
    { label: 'Name', value: agent.name },
    { label: 'Status', value: <StatusIndicator status={agent.status} /> },
    { label: 'Seed ID', value: agent.seed_id ?? '—' },
    { label: 'Space ID', value: agent.space_id ?? '—' },
    {
      label: 'Started At',
      value: agent.started_at ? new Date(agent.started_at).toLocaleString() : '—',
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate({ to: '/agents' })}
          aria-label="Go back"
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Bot className="h-6 w-6" /> {agent.name}
          </h1>
          <p className="text-muted-foreground">Agent Detail</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => restartMutation.mutate()}
            disabled={restartMutation.isPending || agent.status !== 'running'}
          >
            <RotateCw className="h-4 w-4 mr-1" /> Restart
          </Button>
          <Button
            variant="destructive"
            size="sm"
            onClick={() => killMutation.mutate()}
            disabled={killMutation.isPending || agent.status === 'stopped'}
          >
            <Skull className="h-4 w-4 mr-1" /> Kill
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Agent Information</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-3 md:grid-cols-2">
            {details.map((d) => (
              <div
                key={d.label}
                className="flex items-center justify-between rounded-lg border p-3"
              >
                <span className="text-sm text-muted-foreground">{d.label}</span>
                <span className="text-sm font-medium">{d.value}</span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {agent.metadata && Object.keys(agent.metadata).length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Metadata</CardTitle>
          </CardHeader>
          <CardContent>
            <pre className="rounded-lg bg-muted p-4 text-xs overflow-x-auto">
              {JSON.stringify(agent.metadata, null, 2)}
            </pre>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
