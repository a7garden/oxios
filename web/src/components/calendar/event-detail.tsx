import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import type { CalendarEvent } from '@/types/calendar'

interface Props {
  event: CalendarEvent
  onEdit?: () => void
  onDelete?: () => void
  onClose: () => void
}

export function EventDetail({ event, onEdit, onDelete, onClose }: Props) {
  const { t, i18n } = useTranslation()

  const SOURCE_LABELS: Record<CalendarEvent['source'], string> = {
    agent: t('calendar.sourceAgent'),
    user: t('calendar.sourceUser'),
    cron: t('calendar.sourceCron'),
  }

  function formatDateTime(iso: string): string {
    const d = new Date(iso)
    return d.toLocaleString(i18n.language, {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      weekday: 'short',
    })
  }
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-xl">{event.title}</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          {/* Time */}
          <div className="space-y-1">
            <p className="text-sm font-medium text-muted-foreground">{t('calendar.time')}</p>
            <p className="text-sm">
              {formatDateTime(event.start)}
              <span className="mx-2 text-muted-foreground">→</span>
              {formatDateTime(event.end)}
            </p>
          </div>

          {/* Location */}
          {event.location && (
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">{t('calendar.location')}</p>
              <p className="text-sm">{event.location}</p>
            </div>
          )}

          {/* Description */}
          {event.description && (
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">
                {t('calendar.description')}
              </p>
              <p className="text-sm whitespace-pre-wrap">{event.description}</p>
            </div>
          )}

          {/* Repeat (raw RRULE) */}
          {event.rrule && (
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">{t('calendar.repeat')}</p>
              <p className="text-sm font-mono text-xs">{event.rrule}</p>
            </div>
          )}

          {/* Source badge */}
          <div className="flex items-center gap-2">
            <Badge variant="outline">{SOURCE_LABELS[event.source]}</Badge>
          </div>
        </div>

        {/* Actions */}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t('calendar.close')}
          </Button>
          {onEdit && (
            <Button variant="outline" onClick={onEdit}>
              {t('calendar.edit')}
            </Button>
          )}
          {onDelete && (
            <Button variant="destructive" onClick={onDelete}>
              {t('calendar.delete')}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
