import { useMutation, useQuery } from '@tanstack/react-query'
import {
  AlertCircle,
  CheckCircle2,
  Database,
  FlaskConical,
  Loader2,
  RotateCcw,
  ScrollText,
  Shield,
  TriangleAlert,
} from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { api } from '@/lib/api-client'
import type { DoctorResponse } from '@/types'

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

// ─── Action Card (reusable) ──────────────────────────────────

function ActionCard({
  title,
  description,
  icon,
  onRun,
  isRunning,
  children,
}: {
  title: string
  description: string
  icon: React.ReactNode
  onRun?: () => void
  isRunning?: boolean
  children?: React.ReactNode
}) {
  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <CardTitle className="flex items-center gap-2 text-base">
            {icon}
            {title}
          </CardTitle>
          {onRun && (
            <Button size="sm" onClick={onRun} disabled={isRunning}>
              {isRunning ? (
                <Loader2 className="h-3 w-3 animate-spin mr-1" />
              ) : (
                <RotateCcw className="h-3 w-3 mr-1" />
              )}
              {isRunning ? '...' : ''}
            </Button>
          )}
        </div>
        <p className="text-xs text-muted-foreground">{description}</p>
      </CardHeader>
      {children && <CardContent className="pt-0">{children}</CardContent>}
    </Card>
  )
}

// ─── Doctor ──────────────────────────────────────────────────

function DoctorPanel() {
  const { t } = useTranslation()

  const doctorMutation = useMutation({
    mutationFn: () => api.post<DoctorResponse>('/api/system/doctor'),
  })

  const handleRun = () => {
    doctorMutation.mutate()
  }

  const data = doctorMutation.data

  return (
    <ActionCard
      title={t('systemTools.doctor')}
      description={t('systemTools.doctorDescription')}
      icon={<FlaskConical className="h-4 w-4" />}
      onRun={handleRun}
      isRunning={doctorMutation.isPending}
    >
      {data && (
        <div className="space-y-3">
          {/* Summary */}
          <div className="flex items-center gap-3">
            <Badge variant={data.issues === 0 ? 'success' : 'destructive'}>
              {data.issues === 0
                ? t('systemTools.allChecksPassed', { count: data.checks })
                : t('systemTools.issuesFound', { checks: data.checks, issues: data.issues })}
            </Badge>
          </div>

          {/* Check results */}
          <div className="space-y-1.5">
            {data.results.map((check) => (
              <div
                key={check.name}
                className="flex items-start gap-2 text-sm rounded-md bg-muted/50 px-3 py-2"
              >
                {check.status === 'pass' && (
                  <CheckCircle2 className="h-4 w-4 text-success shrink-0 mt-0.5" />
                )}
                {check.status === 'warn' && (
                  <TriangleAlert className="h-4 w-4 text-warning shrink-0 mt-0.5" />
                )}
                {check.status === 'fail' && (
                  <AlertCircle className="h-4 w-4 text-error shrink-0 mt-0.5" />
                )}
                <span className="text-muted-foreground">{check.message}</span>
              </div>
            ))}
          </div>

          {/* Action items */}
          {data.action_items.length > 0 && (
            <>
              <Separator />
              <div>
                <p className="text-xs font-medium text-destructive mb-2">
                  {t('systemTools.actionItems')}
                </p>
                <ol className="list-decimal list-inside space-y-1 text-sm text-muted-foreground">
                  {data.action_items.map((item, i) => (
                    <li key={i}>{item}</li>
                  ))}
                </ol>
              </div>
            </>
          )}
        </div>
      )}

      {doctorMutation.isError && (
        <div className="flex items-center gap-2 text-sm text-destructive mt-2">
          <AlertCircle className="h-4 w-4" />
          {(doctorMutation.error as Error)?.message || t('update.unknownError')}
        </div>
      )}
    </ActionCard>
  )
}

// ─── Audit Verify ────────────────────────────────────────────

function AuditVerifyPanel() {
  const { t } = useTranslation()

  const auditMutation = useMutation({
    mutationFn: () =>
      api.post<{ valid: boolean; entries_checked: number; message: string }>(
        '/api/system/audit-verify',
      ),
  })

  return (
    <ActionCard
      title={t('systemTools.auditVerify')}
      description={t('systemTools.auditVerifyDescription')}
      icon={<Shield className="h-4 w-4" />}
      onRun={() => auditMutation.mutate()}
      isRunning={auditMutation.isPending}
    >
      {auditMutation.data && (
        <div
          className={`flex items-center gap-2 text-sm rounded-md px-3 py-2 ${
            auditMutation.data.valid
              ? 'bg-success-subtle text-success'
              : 'bg-error-subtle text-error'
          }`}
        >
          {auditMutation.data.valid ? (
            <CheckCircle2 className="h-4 w-4" />
          ) : (
            <AlertCircle className="h-4 w-4" />
          )}
          {auditMutation.data.message}
        </div>
      )}
    </ActionCard>
  )
}

// ─── Backup ──────────────────────────────────────────────────

function BackupPanel() {
  const { t } = useTranslation()

  const backupMutation = useMutation({
    mutationFn: () =>
      api.post<{ success: boolean; path: string; size_bytes: number; message: string }>(
        '/api/system/backup',
      ),
  })

  return (
    <ActionCard
      title={t('systemTools.backup')}
      description={t('systemTools.backupDescription')}
      icon={<Database className="h-4 w-4" />}
      onRun={() => backupMutation.mutate()}
      isRunning={backupMutation.isPending}
    >
      {backupMutation.data?.success && (
        <div className="flex items-center gap-2 text-sm rounded-md bg-success-subtle text-success px-3 py-2">
          <CheckCircle2 className="h-4 w-4" />
          <span>
            {backupMutation.data.message}
            <span className="ml-2 text-xs opacity-70">
              ({formatBytes(backupMutation.data.size_bytes)})
            </span>
          </span>
        </div>
      )}
      {backupMutation.isError && (
        <div className="flex items-center gap-2 text-sm text-destructive mt-2">
          <AlertCircle className="h-4 w-4" />
          {(backupMutation.error as Error)?.message || t('update.unknownError')}
        </div>
      )}
    </ActionCard>
  )
}

// ─── Log Viewer ──────────────────────────────────────────────

function LogPanel() {
  const { t } = useTranslation()
  const [showLog, setShowLog] = useState(false)

  const {
    data: logData,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: ['system-log'],
    queryFn: () => api.get<{ lines: string[]; total: number }>('/api/system/log'),
    enabled: showLog,
  })

  return (
    <ActionCard
      title={t('systemTools.log')}
      description={t('systemTools.logDescription')}
      icon={<ScrollText className="h-4 w-4" />}
      onRun={() => {
        if (showLog) {
          refetch()
        } else {
          setShowLog(true)
        }
      }}
      isRunning={isLoading}
    >
      {showLog && logData && (
        <div className="rounded-md bg-zinc-950 text-zinc-100 dark:bg-zinc-900 p-3 max-h-80 overflow-y-auto font-mono text-xs leading-relaxed">
          {logData.lines.length === 0 ? (
            <span className="text-zinc-500">{t('systemTools.noLogEntries')}</span>
          ) : (
            logData.lines.map((line, i) => (
              <div key={i} className="whitespace-pre-wrap break-all">
                <span className="text-zinc-600 select-none mr-2">{String(i + 1).padStart(3)}</span>
                {line}
              </div>
            ))
          )}
        </div>
      )}
      {showLog && logData && (
        <p className="text-xs text-muted-foreground mt-2">
          {t('systemTools.showingLines', { shown: logData.lines.length, total: logData.total })}
        </p>
      )}
    </ActionCard>
  )
}

// ─── Main Component ──────────────────────────────────────────

export function SystemToolsPanel() {
  const { t } = useTranslation()

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-lg font-semibold">{t('systemTools.title')}</h3>
        <p className="text-sm text-muted-foreground">{t('systemTools.subtitle')}</p>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <DoctorPanel />
        <AuditVerifyPanel />
        <BackupPanel />
        <LogPanel />
      </div>
    </div>
  )
}
