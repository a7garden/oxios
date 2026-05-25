import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Dna } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { DataTable } from '@/components/shared/data-table'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingTable } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Badge } from '@/components/ui/badge'
import { api } from '@/lib/api-client'
import type { Seed } from '@/types'

export const Route = createFileRoute('/seeds/')({
  component: SeedsListPage,
})

function SeedsListPage() {
  const { t } = useTranslation()
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
    {
      header: t('seeds.goal'),
      accessor: (row: Seed) => <span className="font-medium">{row.goal}</span>,
    },
    {
      header: t('seeds.constraints'),
      accessor: (row: Seed) => <Badge variant="outline">{row.constraints_count}</Badge>,
    },
    {
      header: t('seeds.created'),
      accessor: (row: Seed) => (row.created_at ? new Date(row.created_at).toLocaleString() : '—'),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('seeds.title')}</h1>
          <p className="text-muted-foreground">
            {t('seeds.registered', { count: data?.total ?? 0 })}
          </p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {seeds.length === 0 ? (
        <EmptyState
          icon={<Dna className="h-10 w-10" />}
          title={t('seeds.noSeeds')}
          description={t('seeds.noSeedsDescription')}
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
