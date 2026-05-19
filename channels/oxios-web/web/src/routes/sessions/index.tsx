import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { DataTable } from '@/components/shared/data-table'
import { LoadingTable } from '@/components/shared/loading'
import { EmptyState } from '@/components/shared/empty-state'
import { Button } from '@/components/ui/button'
import { Clock, Trash2, RefreshCw } from 'lucide-react'
import type { Session } from '@/types'

export const Route = createFileRoute('/sessions/')({
  component: SessionsListPage,
})

function SessionsListPage() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const { data, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['sessions'],
    queryFn: () => api.get<{ items: Session[]; total: number }>('/api/sessions'),
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.delete(`/api/sessions/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['sessions'] }),
  })

  if (isLoading) return <LoadingTable rows={5} />

  const sessions = data?.items ?? []

  const columns = [
    { header: 'ID', accessor: (row: Session) => <span className="font-mono text-xs">{row.id.slice(0, 12)}...</span> },
    { header: 'Agent', accessor: (row: Session) => row.agent_id ? row.agent_id.slice(0, 8) + '...' : '—' },
    { header: 'Messages', accessor: (row: Session) => row.message_count ?? 0 },
    { header: 'Created', accessor: (row: Session) => new Date(row.created_at).toLocaleString() },
    { header: 'Updated', accessor: (row: Session) => row.updated_at ? new Date(row.updated_at).toLocaleString() : '—' },
    { header: '', accessor: (row: Session) => (
      <Button
        variant="ghost"
        size="icon"
        onClick={(e) => { e.stopPropagation(); deleteMutation.mutate(row.id) }}
        disabled={deleteMutation.isPending}
      >
        <Trash2 className="h-4 w-4 text-destructive" />
      </Button>
    )},
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Sessions</h1>
          <p className="text-muted-foreground">{data?.total ?? 0} session(s)</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
          <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
        </Button>
      </div>

      {sessions.length === 0 ? (
        <EmptyState
          icon={<Clock className="h-10 w-10" />}
          title="No sessions"
          description="Sessions will appear here after agent interactions."
        />
      ) : (
        <DataTable
          columns={columns}
          data={sessions}
          keyExtractor={(row) => row.id}
          onRowClick={(row) => navigate({ to: '/sessions/$sessionId', params: { sessionId: row.id } })}
        />
      )}
    </div>
  )
}
