import { CheckCircle2, Clock, FileText, ListChecks, XCircle } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { useTokenMaxingSession, useTokenMaxingSessions } from '@/hooks/use-token-maxing'
import type { StopReason, TaskRecord, TokenMaxingSession } from '@/types/token-maxing'

/** Past sessions list — clicking a row opens the full report in a Dialog. */
export function TokenMaxingSessions() {
  const { t } = useTranslation()
  const { data, isLoading } = useTokenMaxingSessions()
  const [openId, setOpenId] = useState<string | null>(null)

  const sessions = data ?? []

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <ListChecks className="h-4 w-4" />
          {t('tokenMaxing.sessions.title')}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <p className="text-sm text-muted-foreground py-4">{t('common.loading')}</p>
        ) : sessions.length === 0 ? (
          <EmptyState
            icon={<FileText className="h-10 w-10" />}
            title={t('tokenMaxing.sessions.emptyTitle')}
            description={t('tokenMaxing.sessions.emptyDesc')}
          />
        ) : (
          <div className="space-y-2">
            {sessions
              .slice()
              .reverse()
              .map((s) => (
                <SessionRow key={s.id} session={s} onOpen={() => setOpenId(s.id)} />
              ))}
          </div>
        )}
      </CardContent>

      <SessionReportDialog sessionId={openId} onClose={() => setOpenId(null)} />
    </Card>
  )
}

function SessionRow({ session, onOpen }: { session: TokenMaxingSession; onOpen: () => void }) {
  const { t } = useTranslation()
  const ended = session.ended_at != null
  return (
    <Button
      variant="outline"
      onClick={onOpen}
      className="w-full justify-between h-auto py-3 px-4 text-left whitespace-normal"
    >
      <div className="space-y-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">{formatDateTime(session.started_at)}</span>
          <StopReasonBadge reason={session.stop_reason} ended={ended} />
          {session.manual && (
            <Badge variant="outline" className="text-xs">
              {t('tokenMaxing.status.manual')}
            </Badge>
          )}
        </div>
        <div className="flex items-center gap-4 text-xs text-muted-foreground">
          <span>
            {t('tokenMaxing.sessions.tasksAndTokens', {
              tasks: session.totals.tasks,
              tokens: session.totals.tokens.toLocaleString(),
            })}
          </span>
          <span>
            {t('tokenMaxing.sessions.drained', {
              count: session.totals.providers_fully_drained,
            })}
          </span>
          <span>
            {t('tokenMaxing.sessions.resets', {
              count: session.totals.resets_observed,
            })}
          </span>
        </div>
      </div>
    </Button>
  )
}

function StopReasonBadge({ reason, ended }: { reason: StopReason; ended: boolean }) {
  const { t } = useTranslation()
  if (!ended) {
    return (
      <Badge variant="success" className="text-xs">
        {t('tokenMaxing.sessions.inProgress')}
      </Badge>
    )
  }
  switch (reason) {
    case 'window_ended':
      return (
        <Badge variant="secondary" className="text-xs">
          {t('tokenMaxing.stopReason.windowEnded')}
        </Badge>
      )
    case 'no_work':
      return (
        <Badge variant="secondary" className="text-xs">
          {t('tokenMaxing.stopReason.noWork')}
        </Badge>
      )
    case 'cancelled':
      return (
        <Badge variant="warning" className="text-xs">
          {t('tokenMaxing.stopReason.cancelled')}
        </Badge>
      )
    default:
      return (
        <Badge variant="outline" className="text-xs">
          {t('tokenMaxing.sessions.unknownStop')}
        </Badge>
      )
  }
}

function SessionReportDialog({
  sessionId,
  onClose,
}: {
  sessionId: string | null
  onClose: () => void
}) {
  const { t } = useTranslation()
  const { data, isLoading, isError } = useTokenMaxingSession(sessionId)

  return (
    <Dialog open={sessionId != null} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-3xl max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t('tokenMaxing.report.title')}</DialogTitle>
        </DialogHeader>

        {sessionId == null ? null : isLoading ? (
          <p className="text-sm text-muted-foreground py-4">{t('common.loading')}</p>
        ) : isError || !data ? (
          <p className="text-sm text-error py-4">{t('tokenMaxing.report.loadFailed')}</p>
        ) : (
          <SessionReportBody session={data} />
        )}
      </DialogContent>
    </Dialog>
  )
}

function SessionReportBody({ session }: { session: TokenMaxingSession }) {
  const { t } = useTranslation()
  return (
    <div className="space-y-6">
      <SessionMeta session={session} />

      <div>
        <h3 className="text-sm font-medium mb-2">{t('tokenMaxing.report.totals')}</h3>
        <div className="grid gap-3 sm:grid-cols-4">
          <Stat
            label={t('tokenMaxing.report.tasks')}
            value={session.totals.tasks.toLocaleString()}
          />
          <Stat
            label={t('tokenMaxing.report.tokens')}
            value={session.totals.tokens.toLocaleString()}
          />
          <Stat
            label={t('tokenMaxing.report.drained')}
            value={session.totals.providers_fully_drained.toLocaleString()}
          />
          <Stat
            label={t('tokenMaxing.report.resets')}
            value={session.totals.resets_observed.toLocaleString()}
          />
        </div>
      </div>

      <div>
        <h3 className="text-sm font-medium mb-2">{t('tokenMaxing.report.perProvider')}</h3>
        {session.providers.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t('tokenMaxing.report.noProviderData')}</p>
        ) : (
          <div className="space-y-2">
            {session.providers.map((p) => (
              <div key={p.provider} className="rounded-md border p-3 space-y-1">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium">{p.provider}</span>
                  <span className="text-xs text-muted-foreground">
                    {t('tokenMaxing.report.tasksAndTokens', {
                      tasks: p.tasks_run,
                      tokens: p.tokens_consumed.toLocaleString(),
                    })}
                  </span>
                </div>
                {p.models_used.length > 0 && (
                  <p className="text-xs text-muted-foreground">
                    {t('tokenMaxing.report.models', {
                      models: p.models_used.join(', '),
                    })}
                  </p>
                )}
                {p.windows_drained.length > 0 && (
                  <p className="text-xs text-muted-foreground">
                    {t('tokenMaxing.report.windowsDrained', {
                      count: p.windows_drained.length,
                    })}
                  </p>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      <div>
        <h3 className="text-sm font-medium mb-2">{t('tokenMaxing.report.taskList')}</h3>
        {session.tasks.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t('tokenMaxing.report.noTasks')}</p>
        ) : (
          <div className="space-y-2">
            {session.tasks.map((task, i) => (
              <TaskRow key={`${task.provider}-${i}`} task={task} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function SessionMeta({ session }: { session: TokenMaxingSession }) {
  const { t } = useTranslation()
  return (
    <div className="grid gap-2 sm:grid-cols-2 text-sm">
      <MetaRow label={t('tokenMaxing.report.startedAt')}>
        {formatDateTime(session.started_at)}
      </MetaRow>
      <MetaRow label={t('tokenMaxing.report.endedAt')}>
        {session.ended_at ? formatDateTime(session.ended_at) : '—'}
      </MetaRow>
      <MetaRow label={t('tokenMaxing.report.window')}>
        {session.window
          ? `${formatDateTime(session.window.start)} → ${formatDateTime(session.window.end)}`
          : t('tokenMaxing.status.manual')}
      </MetaRow>
      <MetaRow label={t('tokenMaxing.report.stopReason')}>
        <StopReasonBadge reason={session.stop_reason} ended={session.ended_at != null} />
      </MetaRow>
    </div>
  )
}

function MetaRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className="text-muted-foreground">{label}</span>
      <span>{children}</span>
    </div>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border p-3 space-y-1">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="text-lg font-semibold">{value}</p>
    </div>
  )
}

function TaskRow({ task }: { task: TaskRecord }) {
  const { t } = useTranslation()
  return (
    <div className="rounded-md border p-3 space-y-1">
      <div className="flex items-center gap-2 flex-wrap">
        {task.success ? (
          <CheckCircle2 className="h-4 w-4 text-success" />
        ) : (
          <XCircle className="h-4 w-4 text-error" />
        )}
        <Badge variant="outline" className="text-xs">
          {t(`tokenMaxing.report.source.${task.source}`)}
        </Badge>
        <span className="text-xs text-muted-foreground">
          {task.provider}/{task.model}
        </span>
        <span className="text-xs text-muted-foreground">
          {t('tokenMaxing.report.tokensFmt', {
            tokens: task.tokens.toLocaleString(),
          })}
        </span>
        <span className="text-xs text-muted-foreground flex items-center gap-1">
          <Clock className="h-3 w-3" />
          {t('tokenMaxing.report.duration', {
            seconds: task.duration_secs.toFixed(1),
          })}
        </span>
      </div>
      <p className="text-sm font-medium">{task.goal}</p>
      {task.summary && (
        <p className="text-xs text-muted-foreground whitespace-pre-wrap">{task.summary}</p>
      )}
    </div>
  )
}

function formatDateTime(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleString()
}
