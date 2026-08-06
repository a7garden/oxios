/**
 * Timeline settings card (oxiline first-party app module).
 *
 * Status + live Connect/Disconnect toggle — read-only context-in. Mirrors the
 * memo card: `GET /api/timeline/status` (404 when the `timeline` cargo feature
 * is absent → "not compiled" notice) and `POST /api/timeline/enable|disable`
 * performing a live kernel swap. oxios shares oxiline's store but never owns it.
 */
import { CheckCircle2, Loader2, Plug, PlugZap, XCircle } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useTimelineDisable, useTimelineEnable, useTimelineStatus } from '@/hooks/use-timeline'
import { SectionCard } from './section-card'

export function TimelineSectionCard() {
  const { t } = useTranslation()
  const { data: status, isLoading } = useTimelineStatus()
  const enableMut = useTimelineEnable()
  const disableMut = useTimelineDisable()
  const [dbPath, setDbPath] = useState('')

  // Feature not compiled in (route 404s) → informational notice, no controls.
  if (!status) {
    return (
      <SectionCard
        title={t('settings.sectionTimeline')}
        description={t('settings.timelineDescription')}
        sectionId="timeline"
      >
        <p className="text-muted-foreground px-4 py-3 text-sm">
          {isLoading ? '…' : t('settings.timelineNotCompiled')}
        </p>
      </SectionCard>
    )
  }

  const connected = status.connected
  const busy = enableMut.isPending || disableMut.isPending

  const onEnable = () =>
    enableMut.mutate(dbPath, {
      onSuccess: () => toast.success(t('settings.timelineConnected')),
      onError: (e) => toast.error(e instanceof Error ? e.message : String(e)),
    })
  const onDisable = () =>
    disableMut.mutate(undefined, {
      onSuccess: () => toast.success(t('settings.timelineDisconnected')),
      onError: (e) => toast.error(e instanceof Error ? e.message : String(e)),
    })

  return (
    <SectionCard
      title={t('settings.sectionTimeline')}
      description={t('settings.timelineDescription')}
      sectionId="timeline"
    >
      <div className="flex items-center justify-between gap-4 px-4 py-3">
        <div className="min-w-0 space-y-1">
          <div className="flex items-center gap-2">
            <span
              className={`h-2 w-2 shrink-0 rounded-full ${
                connected ? 'bg-success' : 'bg-muted-foreground/40'
              }`}
            />
            {connected ? (
              <Badge className="border-success/40 bg-success/15 text-success">
                <CheckCircle2 className="mr-1 h-3 w-3" />
                {t('settings.timelineConnectedBadge')}
              </Badge>
            ) : (
              <Badge variant="secondary">
                <XCircle className="mr-1 h-3 w-3" />
                {t('settings.timelineDisconnectedBadge')}
              </Badge>
            )}
          </div>
          {status.db_path && (
            <p className="truncate font-mono text-xs text-muted-foreground">{status.db_path}</p>
          )}
        </div>
        {connected ? (
          <Button variant="outline" size="sm" onClick={onDisable} disabled={busy}>
            {disableMut.isPending ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <PlugZap className="h-3.5 w-3.5" />
            )}
            <span className="ml-1.5">{t('settings.timelineDisconnect')}</span>
          </Button>
        ) : (
          <Button size="sm" onClick={onEnable} disabled={busy}>
            {enableMut.isPending ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Plug className="h-3.5 w-3.5" />
            )}
            <span className="ml-1.5">{t('settings.timelineConnect')}</span>
          </Button>
        )}
      </div>
      {!connected && (
        <div className="space-y-2 px-4 py-3">
          <label className="text-xs text-muted-foreground">{t('settings.timelineDbPath')}</label>
          <Input
            value={dbPath}
            onChange={(e) => setDbPath(e.target.value)}
            placeholder={t('settings.timelineDbPathPlaceholder')}
            className="font-mono text-xs"
          />
          <p className="text-xs text-muted-foreground">{t('settings.timelineDbPathHint')}</p>
        </div>
      )}
    </SectionCard>
  )
}
