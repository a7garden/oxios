/**
 * Timeline (Gantt-style) visualization helpers for cron jobs.
 *
 * Fire-time computation strategy:
 * - `nextRun` from the backend is authoritative (computed by the Rust `cron`
 *   crate with exact 5–7 field support) and is always shown as the primary marker.
 * - `cron-parser` (npm) projects *additional* future fire times to visualize
 *   the schedule pattern. It handles 5–6 field expressions; 7-field and exotic
 *   syntax will throw, which we catch and gracefully degrade (show only `nextRun`).
 * - Both compute in UTC to match the backend.
 */

import { CronExpressionParser } from 'cron-parser'

export type TimeRange = '24h' | '48h' | '7d'

export interface TimeRangeOption {
  value: TimeRange
  hours: number
}

export const TIME_RANGES: TimeRangeOption[] = [
  { value: '24h', hours: 24 },
  { value: '48h', hours: 48 },
  { value: '7d', hours: 168 },
]

/** Fixed pixel density — consistent visual spacing across all ranges. */
export const PIXELS_PER_HOUR = 42

/** Label column width (must match the `w-52` on the sticky label cell). */
export const LABEL_WIDTH = 208

/** Above this fire-count, render a continuous bar instead of individual dots. */
export const DENSE_THRESHOLD = 25

/** Max fire times to compute per job (safety cap for very frequent schedules). */
const MAX_FIRE_TIMES = 300

/** Dedup window — skip projected times within 30s of an existing marker. */
const DEDUP_MS = 30_000

export interface FireTime {
  /** JS Date of the scheduled fire. */
  date: Date
  /** True = authoritative `next_run` from the backend. */
  isPrimary: boolean
}

export interface JobFireData {
  jobId: string
  fireTimes: FireTime[]
  isDense: boolean
  parseFailed: boolean
}

/**
 * Compute future fire times for a cron expression within a time window.
 *
 * Returns `{ times, parseFailed }`. When `cron-parser` can't parse the
 * expression (6–7 field, exotic syntax), `parseFailed` is `true` and `times`
 * contains only `nextRun` (if available).
 */
export function computeFireTimes(
  cronExpr: string,
  nextRun: string | undefined,
  windowHours: number,
): { times: FireTime[]; parseFailed: boolean } {
  const now = Date.now()
  const windowEnd = now + windowHours * 3_600_000
  const times: FireTime[] = []
  let parseFailed = false

  // 1. Always add next_run from backend (authoritative)
  if (nextRun) {
    const ts = new Date(nextRun).getTime()
    if (ts <= windowEnd) {
      times.push({ date: new Date(ts), isPrimary: true })
    }
  }

  // 2. Project additional fire times with cron-parser
  try {
    const interval = CronExpressionParser.parse(cronExpr.trim(), {
      currentDate: new Date(now),
      tz: 'UTC',
    })

    let count = 0
    while (count < MAX_FIRE_TIMES) {
      let next: Date
      try {
        next = interval.next().toDate()
      } catch {
        break
      }
      if (next.getTime() > windowEnd) break
      const exists = times.some((t) => Math.abs(t.date.getTime() - next.getTime()) < DEDUP_MS)
      if (!exists) {
        times.push({ date: next, isPrimary: false })
      }
      count++
    }
  } catch {
    parseFailed = true
  }

  times.sort((a, b) => a.date.getTime() - b.date.getTime())
  return { times, parseFailed }
}

/**
 * Compute fire data for a list of jobs. Groups computation so the timeline
 * component can memoize on a single value.
 */
export function computeJobFireData(
  jobs: { id: string; schedule: string; next_run?: string }[],
  windowHours: number,
): Map<string, JobFireData> {
  const map = new Map<string, JobFireData>()
  for (const job of jobs) {
    const { times, parseFailed } = computeFireTimes(job.schedule, job.next_run, windowHours)
    map.set(job.id, {
      jobId: job.id,
      fireTimes: times,
      isDense: times.length > DENSE_THRESHOLD,
      parseFailed,
    })
  }
  return map
}

/** Total ms from `now` to a fire time, clamped to ≥ 0. */
export function fireOffsetMs(date: Date, nowMs: number): number {
  return Math.max(0, date.getTime() - nowMs)
}

/** Convert ms offset to pixel position on the timeline. */
export function msToPx(ms: number): number {
  return (ms / 3_600_000) * PIXELS_PER_HOUR
}

/**
 * Compute aligned tick positions for the time axis.
 * Ticks align to natural local-time boundaries (00:00, 03:00, 06:00, …).
 */
export function computeTicks(windowStartMs: number, hours: number, range: TimeRange): Date[] {
  const intervalHours = range === '24h' ? 3 : range === '48h' ? 6 : 24
  const intervalMs = intervalHours * 3_600_000
  const endMs = windowStartMs + hours * 3_600_000

  // Align to local midnight, then step by interval
  const start = new Date(windowStartMs)
  start.setSeconds(0, 0)
  start.setMinutes(0)
  // For 24h: align to 3h; for 48h: align to 6h; for 7d: align to midnight
  if (range === '7d') {
    start.setHours(0)
  } else {
    const h = start.getHours()
    start.setHours(Math.ceil(h / intervalHours) * intervalHours)
  }
  if (start.getTime() <= windowStartMs) {
    start.setTime(start.getTime() + intervalMs)
  }

  const ticks: Date[] = []
  for (let t = start.getTime(); t <= endMs; t += intervalMs) {
    ticks.push(new Date(t))
  }
  return ticks
}

/** Format a tick label for the time axis. */
export function formatTickLabel(date: Date, range: TimeRange): string {
  if (range === '7d') {
    return date.toLocaleDateString(undefined, {
      weekday: 'short',
      month: 'short',
      day: 'numeric',
    })
  }
  const time = date.toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
  if (range === '48h') {
    return `${date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })} ${time}`
  }
  return time
}

/** Chart color CSS variable for a job at a given index (cycles through 5). */
export function chartColor(index: number): string {
  return `var(--chart-${(index % 5) + 1})`
}
