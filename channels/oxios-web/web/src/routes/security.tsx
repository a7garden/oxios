import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { FileWarning, RefreshCw, Shield } from 'lucide-react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'

export const Route = createFileRoute('/security')({ component: SecurityPage })

function SecurityPage() {
  const {
    data: audits,
    isLoading: auditLoading,
    isError: auditError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['audit'],
    queryFn: async () => {
      // Backend uses /api/audit, not /api/security/audit
      const res = await api.get<{ items: { timestamp: string; agent_name: string; action: string; resource: string; allowed: boolean; reason: string | null }[] }>('/api/audit')
      return res
    },
    refetchInterval: 15000,
  })

  if (auditLoading) return <LoadingCards count={4} />
  if (auditError) return <ErrorState onRetry={() => refetch()} />

  const entries = (audits?.items ?? []).map((e) => ({
    ...e,
    id: `${e.timestamp}-${e.agent_name}`,
    agent_id: e.agent_name,
  }))

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Security</h1>
          <p className="text-muted-foreground">Audit trail and access control</p>
        </div>
        <button
          type="button"
          onClick={() => refetch()}
          aria-label="Refresh"
          disabled={isFetching}
          className="rounded-md p-2 hover:bg-muted"
        >
          <RefreshCw className={`h-4 w-4 ${isFetching ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {/* Audit Trail */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FileWarning className="h-4 w-4" /> Audit Trail
          </CardTitle>
        </CardHeader>
        <CardContent>
          {entries.length === 0 ? (
            <EmptyState
              icon={<Shield className="h-8 w-8" />}
              title="No audit entries"
              description="Security audit events will appear here."
              className="py-6"
            />
          ) : (
            <div className="space-y-2">
              {entries.map((entry) => (
                <div
                  key={entry.id}
                  className="flex items-center justify-between rounded-lg border p-3"
                >
                  <div className="flex items-center gap-3">
                    <Shield className="h-4 w-4 text-muted-foreground" />
                    <div>
                      <p className="font-medium text-sm">{entry.action}</p>
                      {entry.resource && (
                        <p className="text-xs text-muted-foreground">{entry.resource}</p>
                      )}
                      {entry.agent_id && (
                        <p className="text-xs text-muted-foreground">
                          Agent: {entry.agent_id.slice(0, 8)}...
                        </p>
                      )}
                    </div>
                  </div>
                  <div className="text-right">
                    <p className="text-xs text-muted-foreground">
                      {new Date(entry.timestamp).toLocaleString()}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
