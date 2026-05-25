import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, Bot, Skull } from 'lucide-react'
import { useTranslation } from 'react-i18next'
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
  const { t } = useTranslation()
  const { agentId } = Route.useParams()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  // No GET /api/agents/:id endpoint — fetch from list and filter
  const {
    data: agent,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['agent', agentId],
    queryFn: async () => {
      const res = await api.get<{ items: Agent[] }>('/api/agents')
      return res.items?.find((a) => a.id === agentId)
    },
    refetchInterval: 5000,
  })

  const killMutation = useMutation({
    mutationFn: () => api.post(`/api/agents/${agentId}/kill`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agent', agentId] })
      queryClient.invalidateQueries({ queryKey: ['agents'] })
    },
  })

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!agent) return <p className="text-muted-foreground">{t('agents.notFound')}</p>

  const details = [
    { label: t('agents.agentId'), value: agent.id },
    { label: t('agents.name'), value: agent.name },
    {
      label: t('agents.status'),
      value: <StatusIndicator status={agent.status?.toLowerCase() ?? 'unknown'} />,
    },
    { label: t('seeds.seed'), value: agent.seed_id ?? '—' },
    {
      label: t('agents.created'),
      value: agent.created_at ? new Date(agent.created_at).toLocaleString() : '—',
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate({ to: '/agents' })}
          aria-label={t('common.goBack')}
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Bot className="h-6 w-6" /> {agent.name}
          </h1>
          <p className="text-muted-foreground">{t('agents.agentDetail')}</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="destructive"
            size="sm"
            onClick={() => killMutation.mutate()}
            disabled={killMutation.isPending || agent.status?.toLowerCase() === 'stopped'}
          >
            <Skull className="h-4 w-4 mr-1" /> {t('agents.terminate')}
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t('agents.agentInformation')}</CardTitle>
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
    </div>
  )
}
