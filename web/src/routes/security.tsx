import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { FileWarning, Filter, KeyRound, Search, Shield } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ApprovalsQueue } from '@/components/dashboard/approvals-queue'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'

export const Route = createFileRoute('/security')({ component: SecurityPage })

function SecurityPage() {
  const { t } = useTranslation()
  const {
    data: audits,
    isLoading: auditLoading,
    isError: auditError,
    refetch,
    isFetching,
  } = useQuery<{
    items: {
      timestamp: string
      agent_name: string
      action: string
      resource: string
      allowed: boolean
      reason: string | null
    }[]
  }>({
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
  const [auditQuery, setAuditQuery] = useState('')
  const [auditOnlyDenied, setAuditOnlyDenied] = useState(false)
  const AUDIT_PAGE_SIZE = 20

  if (auditLoading) return <LoadingCards count={4} />
  if (auditError) return <ErrorState onRetry={() => refetch()} />

  const allEntries = (Array.isArray(audits?.items) ? audits.items : []).map((e) => ({
    ...e,
    id: `${e.timestamp}-${e.agent_name}`,
    agent_id: e.agent_name,
  }))
  const q = auditQuery.trim().toLowerCase()
  const entries = allEntries.filter((e) => {
    if (auditOnlyDenied && e.allowed) return false
    if (!q) return true
    return (
      e.action.toLowerCase().includes(q) ||
      (e.resource ?? '').toLowerCase().includes(q) ||
      (e.agent_name ?? '').toLowerCase().includes(q) ||
      (e.reason ?? '').toLowerCase().includes(q)
    )
  })
  const totalPages = Math.max(1, Math.ceil(entries.length / AUDIT_PAGE_SIZE))
  const safePage = Math.min(auditPage, totalPages)
  const pagedEntries = entries.slice((safePage - 1) * AUDIT_PAGE_SIZE, safePage * AUDIT_PAGE_SIZE)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('security.title')}</h1>
          <p className="text-muted-foreground">{t('security.subtitle')}</p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>
      <ApprovalsQueue />

      {/* Permissions */}
      {permissionsError ? (
        <ErrorState onRetry={() => refetchPermissions()} />
      ) : permissions ? (
        <div className="space-y-3">
          <div className="flex items-center gap-2 px-1">
            <Shield className="h-4 w-4 text-muted-foreground" />
            <h2 className="text-sm font-semibold">{t('security.permissions')}</h2>
            <Badge variant="outline" className="text-2xs">
              {t('security.readOnly')}
            </Badge>
          </div>
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
                        {policy.resources.length}{' '}
                        {policy.resources.length !== 1
                          ? t('security.permissions')
                          : t('security.permission')}
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
                      <p className="text-xs text-muted-foreground">{t('security.noPermissions')}</p>
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
          <div className="flex items-center justify-between gap-2 flex-wrap">
            <CardTitle className="flex items-center gap-2">
              <FileWarning className="h-4 w-4" /> {t('security.auditTrail')}
            </CardTitle>
            <div className="flex items-center gap-2">
              <div className="relative">
                <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
                <Input
                  value={auditQuery}
                  onChange={(e) => {
                    setAuditQuery(e.target.value)
                    setAuditPage(1)
                  }}
                  placeholder={t('security.searchAudit')}
                  className="pl-7 h-8 w-56"
                />
              </div>
              <Button
                size="sm"
                variant={auditOnlyDenied ? 'default' : 'outline'}
                onClick={() => {
                  setAuditOnlyDenied((v) => !v)
                  setAuditPage(1)
                }}
                className="gap-1.5"
              >
                <Filter className="h-3.5 w-3.5" />
                {t('security.onlyDenied')}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {entries.length === 0 ? (
            <EmptyState
              icon={<Shield className="h-8 w-8" />}
              title={
                q || auditOnlyDenied
                  ? t('security.noMatchingEntries')
                  : t('security.noAuditEntries')
              }
              description={t('security.noAuditEntriesDescription')}
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
                      {entry.allowed ? t('security.allow') : t('security.deny')}
                    </Badge>
                    <div>
                      <p className="font-medium text-sm">{entry.action}</p>
                      {entry.resource && (
                        <p className="text-xs text-muted-foreground">{entry.resource}</p>
                      )}
                      {entry.agent_id && (
                        <p className="text-xs text-muted-foreground">
                          {t('security.agent')}: {entry.agent_id.slice(0, 8)}...
                        </p>
                      )}
                      {entry.reason && <p className="text-xs text-warning">{entry.reason}</p>}
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
                {t('security.showingEntries', {
                  start: (auditPage - 1) * AUDIT_PAGE_SIZE + 1,
                  end: Math.min(auditPage * AUDIT_PAGE_SIZE, entries.length),
                  total: entries.length,
                })}
              </p>
              <div className="flex gap-1">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={auditPage <= 1}
                  onClick={() => setAuditPage((p) => Math.max(1, p - 1))}
                >
                  {t('common.previous')}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={auditPage >= totalPages}
                  onClick={() => setAuditPage((p) => Math.min(totalPages, p + 1))}
                >
                  {t('common.next')}
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
