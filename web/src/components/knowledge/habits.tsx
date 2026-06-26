import { BarChart3, ChevronLeft, ChevronRight, Smile } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { useKnowledgeHabits } from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'

// ─── Types ────────────────────────────────────────────────────

/** Backend: habit name → { dayOfYear → status } */
type HabitMap = Record<string, Record<string, number>>

// ─── Helpers ──────────────────────────────────────────────────

/** Get number of days in a year */
function daysInYear(year: number): number {
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 366 : 365
}

/** Get day-of-year for a date */
function dayOfYear(date: Date): number {
  const start = new Date(date.getFullYear(), 0, 0)
  const diff = date.getTime() - start.getTime()
  return Math.floor(diff / (1000 * 60 * 60 * 24))
}

/** Get the ISO week number for a day-of-year */
function weekOfYear(year: number, doy: number): number {
  const jan1 = new Date(year, 0, 1)
  const dayNum = jan1.getDay() // 0=Sun
  // Adjust: Monday = start of week
  const offset = dayNum === 0 ? 6 : dayNum - 1
  return Math.floor((doy - 1 + offset) / 7)
}

/** Total weeks in the grid (53 covers all years) */
const TOTAL_WEEKS = 53

/** Colored dot representing a habit/mood status. */
function StatusDot({ status, isMood }: { status: number; isMood: boolean }) {
  let color = 'bg-muted/40'
  if (status !== -1 && status !== undefined) {
    if (isMood) {
      const moodColors = [
        'bg-muted',
        'bg-error',
        'bg-warning',
        'bg-warning/80',
        'bg-success/60',
        'bg-success',
      ]
      const level = Math.min(Math.max(status, 0), 5)
      color = moodColors[level] ?? color
    } else if (status > 0) {
      color = 'bg-success/80'
    }
  }
  return <span className={cn('inline-block h-2 w-2 rounded-full align-middle', color)} />
}

// ─── Year Grid (GitHub contribution graph style) ──────────────

function HabitYearGrid({
  habitName,
  yearData,
  year,
  isMood,
}: {
  habitName: string
  yearData: Record<string, number>
  year: number
  isMood: boolean
}) {
  const { t } = useTranslation()
  const [hoveredDay, setHoveredDay] = useState<number | null>(null)

  // Build a 53×7 grid (week × weekday)
  // Each cell represents one day. Position by (week, weekday)
  const totalDays = daysInYear(year)

  // Build grid: map each day to (week, weekday)
  const grid = useMemo(() => {
    const cells: Array<{
      doy: number
      week: number
      weekday: number
      status: number
      date: Date
    }> = []

    for (let doy = 1; doy <= totalDays; doy++) {
      const date = new Date(year, 0, doy)
      const weekday = date.getDay() // 0=Sun
      const week = weekOfYear(year, doy)
      const status = yearData[String(doy)] ?? -1 // -1 = no data
      cells.push({ doy, week, weekday, status, date })
    }
    return cells
  }, [yearData, year, totalDays])

  // Calculate stats
  const completedDays = useMemo(
    () => Object.values(yearData).filter((v) => v > 0).length,
    [yearData],
  )
  const totalTracked = Object.keys(yearData).length

  // Color mapping
  const getColor = (status: number, isMood: boolean): string => {
    if (status === -1) return 'bg-transparent'
    if (status === 0) return 'bg-muted/40'

    if (isMood) {
      // Mood: 1-5 scale
      const level = Math.min(Math.max(status, 0), 5)
      const colors = [
        'bg-muted/40',
        'bg-error/60',
        'bg-warning/60',
        'bg-warning/50',
        'bg-success/70',
        'bg-success',
      ]
      return colors[level] ?? 'bg-muted/40'
    }

    // Regular habit: completed
    // Check if weekend (simplified: we use the status from backend)
    return 'bg-success/80'
  }

  // Format tooltip
  const formatTooltip = (doy: number, status: number): string => {
    const date = new Date(year, 0, doy)
    const dateStr = date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
    if (status === -1) return `${dateStr}`
    if (status === 0) return `${dateStr}: ${t('knowledge.markIncomplete')}`
    if (isMood) {
      return `${dateStr}: ●`
    }
    return `${dateStr}: ${t('knowledge.markComplete')}`
  }

  // Build grid as CSS grid: 53 columns (weeks) × 7 rows (Mon-Sun)
  // Re-map weekday: Mon=0, Tue=1, ..., Sun=6
  const remapWeekday = (dow: number) => (dow === 0 ? 6 : dow - 1)

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium flex items-center gap-1.5">
          {isMood && <Smile className="h-4 w-4" />}
          {habitName}
        </h3>
        <span className="text-xs text-muted-foreground">
          {completedDays}/{totalTracked}
        </span>
      </div>

      {/* Year grid */}
      <div
        className="relative"
        style={{
          display: 'grid',
          gridTemplateColumns: `repeat(${TOTAL_WEEKS}, 1fr)`,
          gridTemplateRows: 'repeat(7, 1fr)',
          gap: '2px',
          width: '100%',
          aspectRatio: `${TOTAL_WEEKS * 1.2} / 7`, // slightly wider cells
        }}
      >
        {grid.map((cell) => {
          const col = cell.week + 1
          const row = remapWeekday(cell.weekday) + 1
          return (
            <div
              key={cell.doy}
              className={cn(
                'rounded-sm transition-colors cursor-default',
                getColor(cell.status, isMood),
                hoveredDay === cell.doy && 'ring-1 ring-foreground/30',
              )}
              style={{ gridColumn: col, gridRow: row }}
              onMouseEnter={() => setHoveredDay(cell.doy)}
              onMouseLeave={() => setHoveredDay(null)}
              title={formatTooltip(cell.doy, cell.status)}
            />
          )
        })}
      </div>

      {/* Tooltip */}
      {hoveredDay !== null &&
        (() => {
          const date = new Date(year, 0, hoveredDay)
          const status = yearData[String(hoveredDay)]
          const dateStr = date.toLocaleDateString(undefined, {
            month: 'long',
            day: 'numeric',
            weekday: 'short',
          })
          return (
            <div className="text-xs text-muted-foreground">
              {dateStr}:{' '}
              {status === undefined ? '—' : <StatusDot status={status} isMood={isMood ?? false} />}
            </div>
          )
        })()}

      {/* Month labels */}
      <div
        className="text-2xs text-muted-foreground/60 mt-1"
        style={{
          display: 'grid',
          gridTemplateColumns: `repeat(${TOTAL_WEEKS}, 1fr)`,
          gap: '2px',
        }}
      >
        {Array.from({ length: 12 }, (_, m) => {
          const firstDay = new Date(year, m, 1)
          const doy = dayOfYear(firstDay)
          const week = weekOfYear(year, doy)
          return (
            <span key={m} style={{ gridColumn: `${week + 1} / span 3` }}>
              {firstDay.toLocaleDateString(undefined, { month: 'short' })}
            </span>
          )
        })}
      </div>
    </div>
  )
}

// ─── Main Component ───────────────────────────────────────────

export function Habits() {
  const { t } = useTranslation()
  const currentYear = new Date().getFullYear()
  const [year, setYear] = useState(currentYear)
  const { data: habits, isLoading } = useKnowledgeHabits(year)

  if (isLoading) {
    return <div className="py-6 text-muted-foreground">{t('knowledge.loadingHabits')}</div>
  }

  const habitsData = habits as HabitMap | undefined
  const habitEntries = habitsData ? Object.entries(habitsData) : []

  // Sort: Mood last
  const sortedEntries = habitEntries.sort(([a], [b]) => {
    if (a === 'Mood') return 1
    if (b === 'Mood') return -1
    return a.localeCompare(b)
  })

  return (
    <div className="space-y-4">
      {/* Year navigation */}
      <div className="flex items-center justify-end gap-2">
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={() => setYear((y) => y - 1)}
        >
          <ChevronLeft className="h-4 w-4" />
        </Button>
        <span className="text-sm font-medium w-12 text-center">{year}</span>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={() => setYear((y) => Math.min(y + 1, currentYear))}
          disabled={year >= currentYear}
        >
          <ChevronRight className="h-4 w-4" />
        </Button>
      </div>

      {/* Legend */}
      <div className="flex items-center gap-4 text-xs text-muted-foreground">
        <span className="flex items-center gap-1">
          <span className="w-3 h-3 rounded-sm bg-muted/40 inline-block" />
          {t('knowledge.habitSkipped')}
        </span>
        <span className="flex items-center gap-1">
          <span className="w-3 h-3 rounded-sm bg-success/80 inline-block" />
          {t('knowledge.habitCompleted')}
        </span>
        <span className="flex items-center gap-1">
          <span className="w-3 h-3 rounded-sm bg-success inline-block" />
          {t('knowledge.habitWeekend')}
        </span>
      </div>

      {/* Habit grids */}
      {sortedEntries.length > 0 ? (
        <div className="space-y-8">
          {sortedEntries.map(([habitName, yearData]) => (
            <HabitYearGrid
              key={habitName}
              habitName={habitName}
              yearData={yearData}
              year={year}
              isMood={habitName === 'Mood'}
            />
          ))}
        </div>
      ) : (
        <div className="text-center py-12">
          <BarChart3 className="h-8 w-8 text-muted-foreground mx-auto mb-3" />
          <p className="text-muted-foreground">{t('knowledge.noHabitData', { year })}</p>
          <p className="text-xs text-muted-foreground mt-1">{t('knowledge.trackHabitsHint')}</p>
        </div>
      )}
    </div>
  )
}
