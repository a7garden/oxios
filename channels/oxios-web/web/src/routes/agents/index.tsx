import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Bot, RefreshCw } from 'lucide-react'
import { DataTable } from '@/components/shared/data-table'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingTable } from '@/components/shared/loading'
import { StatusIndicator } from '@/components/shared/status-indicator'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { api } from '@/lib/api-client'
import type { Agent } from '@/types'

export const Route = createFileRoute('/agents/')({
  component: AgentsListPage,
})

function AgentsListPage() {
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
      header: 'Name',
      accessor: (row: Agent) => (
        <div className="flex items-center gap-2">
          <Bot className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{row.name}</span>
        </div>
      ),
    },
    { header: 'Status', accessor: (row: Agent) => <StatusIndicator status={row.status?.toLowerCase() ?? 'unknown'} /> },
    {
      header: 'Seed',
      accessor: (row: Agent) =>
        row.seed_id ? (
          <Badge variant="outline">{row.seed_id.slice(0, 8)}...</Badge>
        ) : (
          <span className="text-muted-foreground">—</span>
        ),
    },
    {
      header: 'Created',
      accessor: (row: Agent) => (row.created_at ? new Date(row.created_at).toLocaleString() : '—'),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Agents</h1>
          <p className="text-muted-foreground">{data?.total ?? 0} agent(s) registered</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
            <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
          </Button>
        </div>
      </div>

      {agents.length === 0 ? (
        <EmptyState
          icon={<Bot className="h-10 w-10" />}
          title="No agents"
          description="Agents will appear here when they are spawned."
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
