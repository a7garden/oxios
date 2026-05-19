import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Archive, Boxes, Play, RefreshCw } from 'lucide-react'
import { DataTable } from '@/components/shared/data-table'
import { EmptyState } from '@/components/shared/empty-state'
import { LoadingTable } from '@/components/shared/loading'
import { StatusIndicator } from '@/components/shared/status-indicator'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { api } from '@/lib/api-client'
import type { Space } from '@/types'

export const Route = createFileRoute('/spaces/')({
  component: SpacesListPage,
})

function SpacesListPage() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const { data, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['spaces'],
    queryFn: () => api.get<{ items: Space[]; total: number }>('/api/spaces'),
  })

  const activateMutation = useMutation({
    mutationFn: (id: string) => api.post(`/api/spaces/${id}/activate`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['spaces'] }),
  })

  const archiveMutation = useMutation({
    mutationFn: (id: string) => api.post(`/api/spaces/${id}/archive`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['spaces'] }),
  })

  if (isLoading) return <LoadingTable rows={5} />

  const spaces = data?.items ?? []

  const columns = [
    {
      header: 'Name',
      accessor: (row: Space) => (
        <div className="flex items-center gap-2">
          <Boxes className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{row.name}</span>
          {row.tag && <Badge variant="outline">{row.tag}</Badge>}
        </div>
      ),
    },
    { header: 'Status', accessor: (row: Space) => <StatusIndicator status={row.status} /> },
    { header: 'Created', accessor: (row: Space) => new Date(row.created_at).toLocaleString() },
    {
      header: '',
      accessor: (row: Space) => (
        // biome-ignore lint/a11y/useSemanticElements: wrapper div for button group is intentional
        <div
          role="group"
          className="flex gap-1"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          {row.status !== 'active' && (
            <Button variant="ghost" size="icon" onClick={() => activateMutation.mutate(row.id)}>
              <Play className="h-4 w-4 text-emerald-500" />
            </Button>
          )}
          {row.status !== 'archived' && (
            <Button variant="ghost" size="icon" onClick={() => archiveMutation.mutate(row.id)}>
              <Archive className="h-4 w-4 text-amber-500" />
            </Button>
          )}
        </div>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Spaces</h1>
          <p className="text-muted-foreground">{data?.total ?? 0} space(s)</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
          <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
        </Button>
      </div>

      {spaces.length === 0 ? (
        <EmptyState
          icon={<Boxes className="h-10 w-10" />}
          title="No spaces"
          description="Spaces are created during agent interactions."
        />
      ) : (
        <DataTable
          columns={columns}
          data={spaces}
          keyExtractor={(row) => row.id}
          onRowClick={(row) => navigate({ to: '/spaces/$spaceId', params: { spaceId: row.id } })}
        />
      )}
    </div>
  )
}
