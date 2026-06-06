import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import type { CalendarEvent } from '@/types/calendar'

interface Props {
  event: CalendarEvent
  onEdit?: () => void
  onDelete?: () => void
  onClose: () => void
}

const SOURCE_LABELS: Record<CalendarEvent['source'], string> = {
  agent: '에이전트',
  user: '사용자',
  cron: '크론',
}

function formatDateTime(iso: string): string {
  const d = new Date(iso)
  return d.toLocaleString('ko-KR', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    weekday: 'short',
  })
}

export function EventDetail({ event, onEdit, onDelete, onClose }: Props) {
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-xl">{event.title}</DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          {/* Time */}
          <div className="space-y-1">
            <p className="text-sm font-medium text-muted-foreground">시간</p>
            <p className="text-sm">
              {formatDateTime(event.start)}
              <span className="mx-2 text-muted-foreground">→</span>
              {formatDateTime(event.end)}
            </p>
          </div>

          {/* Location */}
          {event.location && (
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">장소</p>
              <p className="text-sm">{event.location}</p>
            </div>
          )}

          {/* Description */}
          {event.description && (
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">설명</p>
              <p className="text-sm whitespace-pre-wrap">{event.description}</p>
            </div>
          )}

          {/* Repeat (raw RRULE) */}
          {event.rrule && (
            <div className="space-y-1">
              <p className="text-sm font-medium text-muted-foreground">반복</p>
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
            닫기
          </Button>
          {onEdit && (
            <Button variant="outline" onClick={onEdit}>
              편집
            </Button>
          )}
          {onDelete && (
            <Button variant="destructive" onClick={onDelete}>
              삭제
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
