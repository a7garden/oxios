import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { Clock, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { type Column, DataTable } from '@/components/shared/data-table'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingTable } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Button } from '@/components/ui/button'
import { api } from '@/lib/api-client'
import type { Session } from '@/types'

export const Route = createFileRoute('/sessions/')({
  component: SessionsListPage,
})

function SessionsListPage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['sessions'],
    queryFn: () => api.get<{ items: Session[]; total: number }>('/api/sessions'),
    refetchInterval: 10000,
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.delete(`/api/sessions/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['sessions'] }),
  })

  if (isLoading) return <LoadingTable rows={5} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const sessions = Array.isArray(data?.items) ? data.items : []

  const columns: Column<Session>[] = [
    {
      header: t('sessions.id'),
      mobilePriority: 'hidden',
      accessor: (row: Session) => (
        <span className="font-mono text-xs">{row.id.slice(0, 12)}...</span>
      ),
    },
    {
      header: t('sessions.title', 'Title'),
      mobilePriority: 'primary',
      accessor: (row: Session) => row.title ?? '—',
    },
    {
      header: t('sessions.agent'),
      mobilePriority: 'secondary',
      accessor: (row: Session) => (row.user_id ? `${row.user_id.slice(0, 8)}...` : '—'),
    },
    {
      header: t('sessions.messages'),
      mobilePriority: 'secondary',
      accessor: (row: Session) => row.message_count ?? 0,
    },
    {
      header: t('sessions.createdAt'),
      mobilePriority: 'hidden',
      accessor: (row: Session) => new Date(row.created_at).toLocaleString(),
    },
    {
      header: t('sessions.updatedAt'),
      mobilePriority: 'hidden',
      accessor: (row: Session) =>
        row.updated_at ? new Date(row.updated_at).toLocaleString() : '—',
    },
    {
      header: '',
      mobilePriority: 'hidden',
      accessor: (row: Session) => (
        <Button
          variant="ghost"
          size="icon"
          onClick={(e) => {
            e.stopPropagation()
            deleteMutation.mutate(row.id)
          }}
          aria-label={t('sessions.deleteSession')}
          disabled={deleteMutation.isPending}
        >
          <Trash2 className="h-4 w-4 text-destructive" />
        </Button>
      ),
    },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('sessions.title')}</h1>
          <p className="text-muted-foreground">
            {t('sessions.registered', { count: data?.total ?? 0 })}
          </p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {sessions.length === 0 ? (
        <EmptyState
          icon={<Clock className="h-10 w-10" />}
          title={t('sessions.noSessions')}
          description={t('sessions.noSessionsDescription')}
        />
      ) : (
        <DataTable
          columns={columns}
          data={sessions}
          keyExtractor={(row) => row.id}
          onRowClick={(row) =>
            navigate({ to: '/sessions/$sessionId', params: { sessionId: row.id } })
          }
        />
      )}
    </div>
  )
}
