import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { FileWarning, KeyRound, RefreshCw, Shield } from 'lucide-react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
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
      const res = await api.get<{
        items: {
          timestamp: string
          agent_name: string
          action: string
          resource: string
          allowed: boolean
          reason: string | null
        }[]
      }>('/api/audit')
      return res
    },
    refetchInterval: 15000,
  })

  const {
    data: permissions,
    isError: permissionsError,
    refetch: refetchPermissions,
  } = useQuery({
    queryKey: ['permissions'],
    queryFn: () =>
      api.get<{
        roles: string[]
        policies: { name: string; effect: string; resources: string[] }[]
      }>('/api/security/permissions'),
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

      {/* Permissions */}
      {permissionsError ? (
        <ErrorState onRetry={() => refetchPermissions()} />
      ) : permissions ? (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <KeyRound className="h-4 w-4" /> Roles & Policies
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div>
              <p className="text-sm font-medium mb-1">Roles</p>
              <div className="flex gap-2 flex-wrap">
                {permissions.roles.map((role) => (
                  <Badge key={role} variant="outline">
                    {role}
                  </Badge>
                ))}
              </div>
            </div>
            <div>
              <p className="text-sm font-medium mb-2">Policies</p>
              <div className="space-y-1">
                {permissions.policies.map((policy) => (
                  <div key={policy.name} className="flex items-center gap-2 text-sm">
                    <Badge variant={policy.effect === 'allow' ? 'success' : 'destructive'}>
                      {policy.effect}
                    </Badge>
                    <span>{policy.name}</span>
                    {policy.resources.length > 0 && (
                      <span className="text-muted-foreground">({policy.resources.join(', ')})</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          </CardContent>
        </Card>
      ) : null}

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
