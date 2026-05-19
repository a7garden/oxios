import { createFileRoute } from '@tanstack/react-router'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { LoadingCards } from '@/components/shared/loading'
import { EmptyState } from '@/components/shared/empty-state'
import { Timer, CheckCircle, XCircle, RefreshCw } from 'lucide-react'
import type { Approval } from '@/types'

export const Route = createFileRoute('/approvals')({ component: ApprovalsPage })

function ApprovalsPage() {
  const queryClient = useQueryClient()

  const { data, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['approvals'],
    queryFn: () => api.get<{ items: Approval[] }>('/api/approvals'),
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

  const items = data?.items ?? []
  const pending = items.filter((a) => a.status === 'pending')
  const resolved = items.filter((a) => a.status !== 'pending')

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Approvals</h1>
          <p className="text-muted-foreground">Human-in-the-loop approval queue</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
          <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
        </Button>
      </div>

      {/* Pending */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Timer className="h-4 w-4" /> Pending ({pending.length})
          </CardTitle>
        </CardHeader>
        <CardContent>
          {pending.length === 0 ? (
            <p className="text-sm text-muted-foreground">No pending approvals.</p>
          ) : (
            <div className="space-y-3">
              {pending.map((approval) => (
                <div key={approval.id} className="flex items-center justify-between rounded-lg border p-4">
                  <div>
                    <p className="font-medium">{approval.description}</p>
                    <p className="text-xs text-muted-foreground mt-1">
                      Type: {approval.type} • Agent: {approval.agent_id.slice(0, 8)}... • {new Date(approval.created_at).toLocaleString()}
                    </p>
                  </div>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      className="text-emerald-600"
                      onClick={() => approveMutation.mutate(approval.id)}
                      disabled={approveMutation.isPending}
                    >
                      <CheckCircle className="h-4 w-4 mr-1" /> Approve
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      className="text-red-600"
                      onClick={() => rejectMutation.mutate(approval.id)}
                      disabled={rejectMutation.isPending}
                    >
                      <XCircle className="h-4 w-4 mr-1" /> Reject
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
            <CardTitle>Resolved</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {resolved.map((approval) => (
                <div key={approval.id} className="flex items-center justify-between rounded-lg border p-3">
                  <div>
                    <p className="text-sm">{approval.description}</p>
                    <p className="text-xs text-muted-foreground">{approval.type} • {new Date(approval.created_at).toLocaleString()}</p>
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
