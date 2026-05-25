import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { FileWarning, KeyRound, Shield } from 'lucide-react'
import { useState } from 'react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
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

  const [auditPage, setAuditPage] = useState(1)
  const AUDIT_PAGE_SIZE = 20

  if (auditLoading) return <LoadingCards count={4} />
  if (auditError) return <ErrorState onRetry={() => refetch()} />

  const entries = (audits?.items ?? []).map((e) => ({
    ...e,
    id: `${e.timestamp}-${e.agent_name}`,
    agent_id: e.agent_name,
  }))

  const totalPages = Math.ceil(entries.length / AUDIT_PAGE_SIZE)
  const pagedEntries = entries.slice(
    (auditPage - 1) * AUDIT_PAGE_SIZE,
    auditPage * AUDIT_PAGE_SIZE,
  )

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Security</h1>
          <p className="text-muted-foreground">Audit trail and access control</p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {/* Permissions */}
      {permissionsError ? (
        <ErrorState onRetry={() => refetchPermissions()} />
      ) : permissions ? (
        <div className="space-y-3">
          {permissions.policies
            .slice()
            .sort((a, b) => b.resources.length - a.resources.length)
            .map((policy) => {
              const roleName = policy.name.replace('-default', '')
              return (
                <Card key={policy.name}>
                  <CardContent className="pt-4">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <KeyRound className="h-4 w-4 text-muted-foreground" />
                        <span className="font-medium">{roleName}</span>
                      </div>
                      <span className="text-xs text-muted-foreground">
                        {policy.resources.length} permission{policy.resources.length !== 1 ? 's' : ''}
                      </span>
                    </div>
                    {policy.resources.length > 0 ? (
                      <div className="flex gap-1.5 flex-wrap">
                        {policy.resources.map((resource) => (
                          <Badge key={resource} variant="secondary" className="text-xs">
                            {resource}
                          </Badge>
                        ))}
                      </div>
                    ) : (
                      <p className="text-xs text-muted-foreground">No permissions</p>
                    )}
                  </CardContent>
                </Card>
              )
            })}
        </div>
      ) : null}

      {/* Audit Trail */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FileWarning className="h-4 w-4" /> Audit Trail
          </CardTitle>
        </CardHeader>
        <CardContent>
          {pagedEntries.length === 0 ? (
            <EmptyState
              icon={<Shield className="h-8 w-8" />}
              title="No audit entries"
              description="Security audit events will appear here."
              className="py-6"
            />
          ) : (
            <div className="space-y-2">
              {pagedEntries.map((entry) => (
                <div
                  key={entry.id}
                  className="flex items-center justify-between rounded-lg border p-3"
                >
                  <div className="flex items-center gap-3">
                    <Badge variant={entry.allowed ? 'success' : 'destructive'} className="shrink-0">
                      {entry.allowed ? 'Allow' : 'Deny'}
                    </Badge>
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
                      {entry.reason && (
                        <p className="text-xs text-amber-600">{entry.reason}</p>
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
          {/* Pagination */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between pt-3 border-t mt-3">
              <p className="text-xs text-muted-foreground">
                Showing {(auditPage - 1) * AUDIT_PAGE_SIZE + 1}–
                {Math.min(auditPage * AUDIT_PAGE_SIZE, entries.length)} of {entries.length}
              </p>
              <div className="flex gap-1">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={auditPage <= 1}
                  onClick={() => setAuditPage((p) => Math.max(1, p - 1))}
                >
                  Previous
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={auditPage >= totalPages}
                  onClick={() => setAuditPage((p) => Math.min(totalPages, p + 1))}
                >
                  Next
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
