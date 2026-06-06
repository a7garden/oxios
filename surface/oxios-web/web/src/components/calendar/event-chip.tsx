import { cn } from '@/lib/utils'
import type { CalendarEvent } from '@/types/calendar'

interface Props {
  event: CalendarEvent
  onClick?: () => void
  compact?: boolean
}

const sourceColors: Record<string, string> = {
  agent: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300',
  user: 'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-300',
  cron: 'bg-gray-100 text-gray-800 dark:bg-gray-900/30 dark:text-gray-300',
}

export function EventChip({ event, onClick, compact }: Props) {
  const start = new Date(event.start)
  const time = start.toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' })
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
