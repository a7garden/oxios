import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Bot } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { DataTable } from '@/components/shared/data-table'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingTable } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { StatusIndicator } from '@/components/shared/status-indicator'
import { Badge } from '@/components/ui/badge'
import { api } from '@/lib/api-client'
import type { Agent } from '@/types'

export const Route = createFileRoute('/agents/')({
  component: AgentsListPage,
})

function AgentsListPage() {
  const { t } = useTranslation()
  const navigate = useNavigate()

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['agents'],
    queryFn: () => api.get<{ items: Agent[]; total: number }>('/api/agents'),
    refetchInterval: 5000,
  })

  if (isLoading) return <LoadingTable rows={5} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const agents = data?.items ?? []

  const columns = [
    {
      header: t('agents.name'),
      accessor: (row: Agent) => (
        <div className="flex items-center gap-2">
          <Bot className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{row.name}</span>
        </div>
      ),
    },
    {
      header: t('agents.status'),
      accessor: (row: Agent) => <StatusIndicator status={row.status?.toLowerCase() ?? 'unknown'} />,
    },
    {
      header: t('agents.seed'),
      accessor: (row: Agent) =>
        row.seed_id ? (
          <Badge variant="outline">{row.seed_id.slice(0, 8)}...</Badge>
        ) : (
          <span className="text-muted-foreground">—</span>
        ),
    },
    {
      header: t('agents.created'),
      accessor: (row: Agent) => (row.created_at ? new Date(row.created_at).toLocaleString() : '—'),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('agents.title')}</h1>
          <p className="text-muted-foreground">
            {t('agents.registered', { count: data?.total ?? 0 })}
          </p>
        </div>
        <div className="flex gap-2">
          <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
        </div>
      </div>

      {agents.length === 0 ? (
        <EmptyState
          icon={<Bot className="h-10 w-10" />}
          title={t('agents.noAgents')}
          description={t('agents.noAgentsDescription')}
        />
      ) : (
        <DataTable
          columns={columns}
          data={agents}
          keyExtractor={(row) => row.id}
          onRowClick={(row) => navigate({ to: '/agents/$agentId', params: { agentId: row.id } })}
        />
      )}
    </div>
  )
}
