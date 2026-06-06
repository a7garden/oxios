import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { CheckCircle, Timer, XCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Approval } from '@/types'

export const Route = createFileRoute('/approvals')({ component: ApprovalsPage })

function ApprovalsPage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['approvals'],
    queryFn: async () => {
      // Backend returns raw array
      const res = await api.get<Approval[]>('/api/approvals')
      return { items: Array.isArray(res) ? res : [] }
    },
    refetchInterval: 5000,
  })

  const approveMutation = useMutation({
    mutationFn: (id: string) => api.post(`/api/approvals/${id}/approve`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['approvals'] }),
  })

  const rejectMutation = useMutation({
    mutationFn: (id: string) => api.post(`/api/approvals/${id}/reject`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['approvals'] }),
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = data?.items ?? []
  const pending = items.filter((a) => a.status === 'pending')
  const resolved = items.filter((a) => a.status !== 'pending')

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('approvals.title')}</h1>
          <p className="text-muted-foreground">{t('approvals.subtitle')}</p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {/* Pending */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Timer className="h-4 w-4" /> {t('approvals.pendingWithCount', { count: pending.length })}
          </CardTitle>
        </CardHeader>
        <CardContent>
          {pending.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('approvals.noPending')}</p>
          ) : (
            <div className="space-y-3">
              {pending.map((approval) => (
                <div
                  key={approval.id}
                  className="flex items-center justify-between rounded-lg border p-4"
                >
                  <div>
                    <p className="font-medium">{approval.reason || approval.action}</p>
                    <p className="text-xs text-muted-foreground mt-1">
                      {approval.action} • {approval.subject} • {approval.resource} •{' '}
                      {new Date(approval.created_at).toLocaleString()}
                    </p>
                  </div>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      className="text-success"
                      onClick={() => approveMutation.mutate(approval.id)}
                      disabled={approveMutation.isPending}
                    >
                      <CheckCircle className="h-4 w-4 mr-1" /> {t('approvals.approve')}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      className="text-error"
                      onClick={() => rejectMutation.mutate(approval.id)}
                      disabled={rejectMutation.isPending}
                    >
                      <XCircle className="h-4 w-4 mr-1" /> {t('approvals.reject')}
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Resolved */}
      {resolved.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>{t('approvals.resolved')}</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {resolved.map((approval) => (
                <div
                  key={approval.id}
                  className="flex items-center justify-between rounded-lg border p-3"
                >
                  <div>
                    <p className="text-sm">{approval.reason || approval.action}</p>
                    <p className="text-xs text-muted-foreground">
                      {approval.action} • {new Date(approval.created_at).toLocaleString()}
                    </p>
                  </div>
                  <Badge variant={approval.status === 'approved' ? 'success' : 'destructive'}>
                    {approval.status}
                  </Badge>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  )
}