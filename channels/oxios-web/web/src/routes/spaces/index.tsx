import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Archive, Boxes, Play } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { DataTable } from '@/components/shared/data-table'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingTable } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
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
  const { t } = useTranslation()

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['spaces'],
    queryFn: () => api.get<{ items: Space[]; total: number }>('/api/spaces'),
    refetchInterval: 15000,
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
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const spaces = data?.items ?? []

  const columns = [
    {
      header: t('common.name'),
      accessor: (row: Space) => (
        <div className="flex items-center gap-2">
          <Boxes className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium">{row.name}</span>
          {row.tags && row.tags.length > 0 && (
            <div className="flex gap-1">
              {row.tags.map((t) => (
                <Badge key={t} variant="outline">
                  {t}
                </Badge>
              ))}
            </div>
          )}
        </div>
      ),
    },
    {
      header: t('spaces.status'),
      accessor: (row: Space) => (
        <Badge variant={row.active !== false ? 'success' : 'secondary'}>
          {row.active !== false ? t('common.active') : t('spaces.archived')}
        </Badge>
      ),
    },
    { header: t('spaces.created'), accessor: (row: Space) => new Date(row.created_at).toLocaleString() },
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
          {!row.active && (
            <Button
              variant="ghost"
              size="icon"
              onClick={() => activateMutation.mutate(row.id)}
              aria-label={t('spaces.activateSpace')}
            >
              <Play className="h-4 w-4 text-emerald-500" />
            </Button>
          )}
          {row.active !== false && (
            <Button
              variant="ghost"
              size="icon"
              onClick={() => archiveMutation.mutate(row.id)}
              aria-label={t('spaces.archiveSpace')}
            >
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
          <h1 className="text-2xl font-bold">{t('spaces.title')}</h1>
          <p className="text-muted-foreground">
            {t('spaces.count', { count: data?.total ?? 0 })}
          </p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {spaces.length === 0 ? (
        <EmptyState
          icon={<Boxes className="h-10 w-10" />}
          title={t('spaces.noSpaces')}
          description={t('spaces.description')}
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
