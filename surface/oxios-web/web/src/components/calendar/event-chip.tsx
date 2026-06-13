import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'
import type { CalendarEvent } from '@/types/calendar'

interface Props {
  event: CalendarEvent
  onClick?: () => void
  compact?: boolean
}

const sourceColors: Record<string, string> = {
  agent: 'bg-info-subtle text-info',
  user: 'bg-secondary text-secondary-foreground',
  cron: 'bg-muted text-muted-foreground',
}

export function EventChip({ event, onClick, compact }: Props) {
  const { i18n } = useTranslation()
  const start = new Date(event.start)
  const time = start.toLocaleTimeString(i18n.language, { hour: '2-digit', minute: '2-digit' })
  const color = sourceColors[event.source] || sourceColors.user

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'w-full text-left rounded px-1.5 py-0.5 text-xs truncate cursor-pointer hover:opacity-80 transition-opacity',
        color,
        compact && 'text-2xs py-px',
      )}
    >
      {compact ? event.title : `${time} ${event.title}`}
    </button>
  )
}
