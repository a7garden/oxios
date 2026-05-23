import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Dna, RefreshCw } from 'lucide-react'
import { DataTable } from '@/components/shared/data-table'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingTable } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { api } from '@/lib/api-client'
import type { Seed } from '@/types'

export const Route = createFileRoute('/seeds/')({
  component: SeedsListPage,
})

function SeedsListPage() {
  const navigate = useNavigate()

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['seeds'],
    queryFn: () => api.get<{ items: Seed[]; total: number }>('/api/seeds'),
    refetchInterval: 10000,
  })

  if (isLoading) return <LoadingTable rows={5} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const seeds = data?.items ?? []

  const columns = [
    { header: 'Goal', accessor: (row: Seed) => <span className="font-medium">{row.goal}</span> },
    {
      header: 'Constraints',
      accessor: (row: Seed) => (
        <Badge variant="outline">{row.constraints_count}</Badge>
      ),
    },
    { header: 'Created', accessor: (row: Seed) => row.created_at ? new Date(row.created_at).toLocaleString() : '—' },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Seeds</h1>
          <p className="text-muted-foreground">{data?.total ?? 0} seed(s)</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
          <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
        </Button>
      </div>

      {seeds.length === 0 ? (
        <EmptyState
          icon={<Dna className="h-10 w-10" />}
          title="No seeds"
          description="Ouroboros seeds will appear here when created."
        />
      ) : (
        <DataTable
          columns={columns}
          data={seeds}
          keyExtractor={(row) => row.id}
          onRowClick={(row) => navigate({ to: '/seeds/$seedId', params: { seedId: row.id } })}
        />
      )}
    </div>
  )
}
