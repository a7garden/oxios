import { Calendar as CalendarIcon, ChevronLeft, ChevronRight, List } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { CalendarEvent } from '@/types/calendar'
import { EventChip } from './event-chip'

// ─── Props ──────────────────────────────────────────────────────────────

interface CalendarViewProps {
  events: CalendarEvent[]
  onEventClick?: (uid: string) => void
  onDateClick?: (date: Date) => void
}

type ViewMode = 'month' | 'week' | 'agenda'

// ─── Constants ──────────────────────────────────────────────────────────

const DAY_LABELS = ['일', '월', '화', '수', '목', '금', '토']

// ─── Date helpers (no external deps) ────────────────────────────────────

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  )
}

function addDays(d: Date, n: number): Date {
  const r = new Date(d)
  r.setDate(r.getDate() + n)
  return r
}

function startOfWeek(d: Date): Date {
  const day = d.getDay() // 0=Sun
  return addDays(d, -day)
}

function formatDateKey(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

function formatMonthYear(d: Date): string {
  return d.toLocaleDateString('ko-KR', { year: 'numeric', month: 'long' })
}

/** Build an array of 42 cells (6 weeks × 7 days) for a month grid. */
function buildMonthGrid(year: number, month: number): Date[] {
  const first = new Date(year, month, 1)
  const start = startOfWeek(first)
  const cells: Date[] = []
  for (let i = 0; i < 42; i++) {
    cells.push(addDays(start, i))
  }
  return cells
}

/** Build 7 days for a week grid starting on Sunday. */
function buildWeekGrid(anchor: Date): Date[] {
  const start = startOfWeek(anchor)
  const cells: Date[] = []
  for (let i = 0; i < 7; i++) {
    cells.push(addDays(start, i))
  }
  return cells
}

/** Group events by date key. */
function groupByDate(events: CalendarEvent[]): Map<string, CalendarEvent[]> {
  const map = new Map<string, CalendarEvent[]>()
  for (const ev of events) {
    const key = formatDateKey(new Date(ev.start))
    const arr = map.get(key) ?? []
    arr.push(ev)
    map.set(key, arr)
  }
  // Sort each group by start time
  for (const arr of map.values()) {
    arr.sort((a, b) => new Date(a.start).getTime() - new Date(b.start).getTime())
  }
  return map
}

// ─── Month View ─────────────────────────────────────────────────────────

function MonthView({
  year,
  month,
  eventsByDate,
  onEventClick,
  onDateClick,
}: {
  year: number
  month: number
  eventsByDate: Map<string, CalendarEvent[]>
  onEventClick?: (uid: string) => void
  onDateClick?: (date: Date) => void
}) {
  const cells = useMemo(() => buildMonthGrid(year, month), [year, month])
  const today = new Date()
  const MAX_VISIBLE_CHIPS = 3

  return (
    <div className="grid grid-cols-7 border-t border-l">
      {/* Day headers */}
      {DAY_LABELS.map((label) => (
        <div
          key={label}
          className="border-b border-r px-2 py-1.5 text-center text-xs font-medium text-muted-foreground bg-muted/30"
        >
          {label}
        </div>
      ))}

      {/* Cells */}
      {cells.map((date, i) => {
        const key = formatDateKey(date)
        const dayEvents = eventsByDate.get(key) ?? []
        const isCurrentMonth = date.getMonth() === month
        const today_ = isSameDay(date, today)
        const overflow = dayEvents.length - MAX_VISIBLE_CHIPS

        return (
          <div
            key={i}
            onClick={() => onDateClick?.(date)}
            className={cn(
              'min-h-[80px] border-b border-r px-1 py-0.5 cursor-pointer hover:bg-muted/40 transition-colors',
              !isCurrentMonth && 'bg-muted/20',
            )}
          >
            {/* Day number */}
            <div className="flex items-center justify-center mb-0.5">
              <span
                className={cn(
                  'inline-flex items-center justify-center h-6 w-6 rounded-full text-xs font-medium',
                  today_ && 'bg-primary text-primary-foreground',
                  !today_ && isCurrentMonth && 'text-foreground',
                  !today_ && !isCurrentMonth && 'text-muted-foreground',
                )}
              >
                {date.getDate()}
              </span>
            </div>

            {/* Event chips */}
            <div className="space-y-0.5 overflow-hidden">
              {dayEvents.slice(0, MAX_VISIBLE_CHIPS).map((ev) => (
                <EventChip
                  key={ev.uid}
                  event={ev}
                  compact
                  onClick={() => {
                    onEventClick?.(ev.uid)
                  }}
                />
              ))}
              {overflow > 0 && (
                <span className="text-2xs text-muted-foreground pl-1">+{overflow}개 더</span>
              )}
            </div>
          </div>
        )
      })}
    </div>
  )
}

// ─── Week View ──────────────────────────────────────────────────────────

function WeekView({
  anchorDate,
  eventsByDate,
  onEventClick,
}: {
  anchorDate: Date
  eventsByDate: Map<string, CalendarEvent[]>
  onEventClick?: (uid: string) => void
}) {
  const days = useMemo(() => buildWeekGrid(anchorDate), [anchorDate])
  const today = new Date()
  const hours = useMemo(() => Array.from({ length: 24 }, (_, i) => i), [])

  return (
    <div className="flex flex-col overflow-auto">
      {/* Header row */}
      <div className="flex sticky top-0 bg-background z-10 border-b">
        <div className="w-14 shrink-0 border-r" />
        {days.map((date, i) => {
          const t = isSameDay(date, today)
          return (
            <div
              key={i}
              className={cn(
                'flex-1 text-center py-2 text-xs font-medium border-r last:border-r-0',
                t ? 'text-primary' : 'text-muted-foreground',
              )}
            >
              <div>{DAY_LABELS[i]}</div>
              <div
                className={cn(
                  'inline-flex items-center justify-center h-6 w-6 rounded-full text-xs',
                  t && 'bg-primary text-primary-foreground',
                )}
              >
                {date.getDate()}
              </div>
            </div>
          )
        })}
      </div>

      {/* Time grid */}
      <div className="flex flex-col">
        {hours.map((hour) => (
          <div key={hour} className="flex border-b last:border-b-0 min-h-[40px]">
            {/* Time label */}
            <div className="w-14 shrink-0 border-r pr-1 pt-0.5 text-right text-2xs text-muted-foreground">
              {String(hour).padStart(2, '0')}:00
            </div>
            {/* Day columns */}
            {days.map((date, di) => {
              const key = formatDateKey(date)
              const dayEvents = (eventsByDate.get(key) ?? []).filter((ev) => {
                const start = new Date(ev.start)
                return start.getHours() === hour
              })

              return (
                <div key={di} className="flex-1 border-r last:border-r-0 px-0.5 py-0.5">
                  {dayEvents.map((ev) => {
                    const start = new Date(ev.start)
                    const end = new Date(ev.end)
                    const durationMin = Math.max(15, (end.getTime() - start.getTime()) / 60000)
                    const heightPx = Math.max(20, (durationMin / 60) * 40)

                    return (
                      <div key={ev.uid} style={{ height: heightPx }}>
                        <EventChip event={ev} onClick={() => onEventClick?.(ev.uid)} />
                      </div>
                    )
                  })}
                </div>
              )
            })}
          </div>
        ))}
      </div>
    </div>
  )
}

// ─── Agenda View ────────────────────────────────────────────────────────

function AgendaView({
  events,
  onEventClick,
}: {
  events: CalendarEvent[]
  onEventClick?: (uid: string) => void
}) {
  const today = new Date()

  // Group by date, then sort
  const grouped = useMemo(() => {
    const map = new Map<string, CalendarEvent[]>()
    const sorted = [...events].sort(
      (a, b) => new Date(a.start).getTime() - new Date(b.start).getTime(),
    )
    for (const ev of sorted) {
      const key = formatDateKey(new Date(ev.start))
      const arr = map.get(key) ?? []
      arr.push(ev)
      map.set(key, arr)
    }
    return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b))
  }, [events])

  if (events.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-muted-foreground">
        <CalendarIcon className="h-10 w-10 mb-2 opacity-40" />
        <p className="text-sm">등록된 일정이 없습니다</p>
      </div>
    )
  }

  return (
    <div className="divide-y">
      {grouped.map(([dateKey, dayEvents]) => {
        const date = new Date(`${dateKey}T00:00:00`)
        const today_ = isSameDay(date, today)
        const label = date.toLocaleDateString('ko-KR', {
          month: 'long',
          day: 'numeric',
          weekday: 'short',
        })

        return (
          <div key={dateKey} className="py-3 px-2">
            <div className="flex items-center gap-2 mb-2">
              <span
                className={cn('text-sm font-semibold', today_ ? 'text-primary' : 'text-foreground')}
              >
                {label}
              </span>
              {today_ && (
                <Badge variant="secondary" className="text-2xs px-1.5 py-0">
                  오늘
                </Badge>
              )}
            </div>
            <div className="space-y-1 pl-2">
              {dayEvents.map((ev) => {
                const start = new Date(ev.start)
                const end = new Date(ev.end)
                const timeLabel = `${start.toLocaleTimeString('ko-KR', {
                  hour: '2-digit',
                  minute: '2-digit',
                })} – ${end.toLocaleTimeString('ko-KR', {
                  hour: '2-digit',
                  minute: '2-digit',
                })}`

                return (
                  <button
                    key={ev.uid}
                    type="button"
                    onClick={() => onEventClick?.(ev.uid)}
                    className="w-full text-left flex items-start gap-3 rounded-md px-2 py-1.5 hover:bg-muted/50 transition-colors cursor-pointer"
                  >
                    <div className="mt-1">
                      <span
                        className={cn(
                          'inline-block w-2 h-2 rounded-full',
                          ev.source === 'agent' && 'bg-blue-500',
                          ev.source === 'user' && 'bg-purple-500',
                          ev.source === 'cron' && 'bg-gray-400',
                        )}
                      />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">{ev.title}</div>
                      <div className="text-xs text-muted-foreground">{timeLabel}</div>
                    </div>
                  </button>
                )
              })}
            </div>
          </div>
        )
      })}
    </div>
  )
}

// ─── Main CalendarView ──────────────────────────────────────────────────

export function CalendarView({ events, onEventClick, onDateClick }: CalendarViewProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('month')
  const [currentDate, setCurrentDate] = useState(() => new Date())

  const year = currentDate.getFullYear()
  const month = currentDate.getMonth()

  const eventsByDate = useMemo(() => groupByDate(events), [events])

  // Navigation
  const goPrev = () => {
    setCurrentDate((d) => {
      if (viewMode === 'week') return addDays(d, -7)
      return new Date(d.getFullYear(), d.getMonth() - 1, 1)
    })
  }

  const goNext = () => {
    setCurrentDate((d) => {
      if (viewMode === 'week') return addDays(d, 7)
      return new Date(d.getFullYear(), d.getMonth() + 1, 1)
    })
  }

  const goToday = () => setCurrentDate(new Date())

  // Header title
  const headerTitle = useMemo(() => {
    if (viewMode === 'week') {
      const days = buildWeekGrid(currentDate)
      const from = days[0]!
      const to = days[6]!
      if (from.getMonth() === to.getMonth()) {
        return from.toLocaleDateString('ko-KR', {
          year: 'numeric',
          month: 'long',
          day: 'numeric',
        })
      }
      return `${from.toLocaleDateString('ko-KR', { month: 'short', day: 'numeric' })} – ${to.toLocaleDateString('ko-KR', { month: 'short', day: 'numeric', year: 'numeric' })}`
    }
    return formatMonthYear(currentDate)
  }, [currentDate, viewMode])

  const today = new Date()
  const isViewingCurrentMonth =
    viewMode === 'month'
      ? year === today.getFullYear() && month === today.getMonth()
      : viewMode === 'week'
        ? (() => {
            const days = buildWeekGrid(currentDate)
            return days.some((d) => isSameDay(d, today))
          })()
        : false

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="flex items-center justify-between px-2 py-2 border-b">
        {/* Nav */}
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" onClick={goPrev}>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" onClick={goNext}>
            <ChevronRight className="h-4 w-4" />
          </Button>
          {!isViewingCurrentMonth && (
            <Button variant="outline" size="sm" onClick={goToday} className="ml-1 text-xs">
              오늘
            </Button>
          )}
          <h2 className="text-sm font-semibold ml-2">{headerTitle}</h2>
        </div>

        {/* View mode toggle */}
        <div className="flex items-center gap-1">
          <Button
            variant={viewMode === 'month' ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => setViewMode('month')}
            className="text-xs"
          >
            <CalendarIcon className="h-3.5 w-3.5 mr-1" />월
          </Button>
          <Button
            variant={viewMode === 'week' ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => setViewMode('week')}
            className="text-xs"
          >
            <CalendarIcon className="h-3.5 w-3.5 mr-1" />주
          </Button>
          <Button
            variant={viewMode === 'agenda' ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => setViewMode('agenda')}
            className="text-xs"
          >
            <List className="h-3.5 w-3.5 mr-1" />
            목록
          </Button>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-auto">
        {viewMode === 'month' && (
          <MonthView
            year={year}
            month={month}
            eventsByDate={eventsByDate}
            onEventClick={onEventClick}
            onDateClick={onDateClick}
          />
        )}
        {viewMode === 'week' && (
          <WeekView
            anchorDate={currentDate}
            eventsByDate={eventsByDate}
            onEventClick={onEventClick}
          />
        )}
        {viewMode === 'agenda' && <AgendaView events={events} onEventClick={onEventClick} />}
      </div>
    </div>
  )
}
