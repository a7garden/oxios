import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { CalendarEvent } from '@/types/calendar'

// ─── Date helpers (self-contained, no external deps) ────────────────────

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  )
}

/** `YYYY-MM-DD` key in the date's own local time (no TZ shift). */
function formatDateKey(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

/** Build a 6×7 grid (42 cells) for the month containing `viewAnchor`. */
function buildMonthGrid(viewAnchor: Date): Date[] {
  const first = new Date(viewAnchor.getFullYear(), viewAnchor.getMonth(), 1)
  const start = new Date(first)
  start.setDate(start.getDate() - first.getDay()) // back to Sunday
  const cells: Date[] = []
  for (let i = 0; i < 42; i++) {
    const d = new Date(start)
    d.setDate(start.getDate() + i)
    cells.push(d)
  }
  return cells
}

function formatMonthYear(d: Date, locale: string): string {
  return d.toLocaleDateString(locale, { year: 'numeric', month: 'long' })
}

/** Group events by their start-date key. */
function groupByDate(events: CalendarEvent[]): Map<string, CalendarEvent[]> {
  const map = new Map<string, CalendarEvent[]>()
  for (const ev of events) {
    const key = formatDateKey(new Date(ev.start))
    const arr = map.get(key)
    if (arr) arr.push(ev)
    else map.set(key, [ev])
  }
  return map
}

/** Source → indicator dot color. */
const SOURCE_COLOR: Record<CalendarEvent['source'], string> = {
  agent: 'bg-info',
  cron: 'bg-warning',
  user: 'bg-primary',
}

// ─── Component ────────────────────────────────────────────────────────────

interface MiniCalendarProps {
  events: CalendarEvent[]
  /** First day-of-month anchor controlling which month is displayed. */
  viewAnchor: Date
  /** Called when the user navigates to a different month. */
  onViewAnchorChange: (date: Date) => void
  /** Currently selected day (controls highlight). */
  selectedDate: Date
  /** Called when a day cell is clicked. */
  onSelectDate: (date: Date) => void
}

/**
 * Compact month calendar for the Notification Center schedule tab.
 *
 * Fully controlled: the parent owns `viewAnchor` (month) and `selectedDate`
 * (day) so the event query range stays in sync with the displayed month.
 */
export function MiniCalendar({
  events,
  viewAnchor,
  onViewAnchorChange,
  selectedDate,
  onSelectDate,
}: MiniCalendarProps) {
  const { t, i18n } = useTranslation()

  const dayLabels = [
    t('calendar.daySun'),
    t('calendar.dayMon'),
    t('calendar.dayTue'),
    t('calendar.dayWed'),
    t('calendar.dayThu'),
    t('calendar.dayFri'),
    t('calendar.daySat'),
  ]

  const eventsByDate = useMemo(() => groupByDate(events), [events])
  const cells = useMemo(() => buildMonthGrid(viewAnchor), [viewAnchor])
  const today = new Date()
  const inMonth = (d: Date) => d.getMonth() === viewAnchor.getMonth()

  const shiftMonth = (delta: number) => {
    onViewAnchorChange(new Date(viewAnchor.getFullYear(), viewAnchor.getMonth() + delta, 1))
  }

  return (
    <div className="select-none">
      {/* Header: month label + nav */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-medium">{formatMonthYear(viewAnchor, i18n.language)}</span>
        <div className="flex items-center gap-0.5">
          <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => shiftMonth(-1)}>
            <ChevronLeft className="h-3.5 w-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2 text-xs"
            onClick={() => {
              const now = new Date()
              onViewAnchorChange(now)
              onSelectDate(now)
            }}
          >
            {t('calendar.today')}
          </Button>
          <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => shiftMonth(1)}>
            <ChevronRight className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>

      {/* Day-of-week headers */}
      <div className="grid grid-cols-7 gap-px mb-1">
        {dayLabels.map((d) => (
          <div key={d} className="text-center text-2xs font-medium text-muted-foreground/70 py-1">
            {d}
          </div>
        ))}
      </div>

      {/* Day grid */}
      <div className="grid grid-cols-7 gap-px">
        {cells.map((date) => {
          const key = formatDateKey(date)
          const dayEvents = eventsByDate.get(key) ?? []
          const isToday = isSameDay(date, today)
          const isSelected = isSameDay(date, selectedDate)
          const dim = !inMonth(date)
          return (
            <button
              key={key}
              type="button"
              onClick={() => onSelectDate(date)}
              className={cn(
                'relative flex flex-col items-center justify-center aspect-square rounded-md text-xs transition-colors',
                'hover:bg-accent/60 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
                dim && 'text-muted-foreground/40',
                isSelected && 'bg-primary/15 text-primary font-medium',
                !isSelected && isToday && 'ring-1 ring-inset ring-primary/40',
              )}
            >
              <span>{date.getDate()}</span>
              {dayEvents.length > 0 && (
                <span className="mt-0.5 flex gap-0.5">
                  {dayEvents.slice(0, 3).map((ev) => (
                    <span
                      key={ev.uid}
                      className={cn('h-1 w-1 rounded-full', SOURCE_COLOR[ev.source])}
                    />
                  ))}
                </span>
              )}
            </button>
          )
        })}
      </div>
    </div>
  )
}
