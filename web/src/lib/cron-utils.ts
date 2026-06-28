/**
 * Human-friendly cron helpers for the Web UI.
 *
 * Standard 5-field cron: `minute hour day-of-month month day-of-week`.
 * These helpers translate between raw cron expressions and a structured
 * {@link SimpleSchedule} that the {@link CronScheduleEditor} GUI edits.
 *
 * Any expression that does not fit one of the supported simple patterns
 * falls back to "advanced" (raw) editing so power users are never blocked.
 */

/** Supported frequency presets the GUI can express. */
export type FrequencyMode = 'minutes' | 'hourly' | 'daily' | 'weekly' | 'monthly'

/** Structured representation of a schedule the GUI understands. */
export interface SimpleSchedule {
  /** Which preset family the schedule belongs to. */
  mode: FrequencyMode
  /** `minutes`: run every N minutes. */
  minuteInterval: number
  /** `hourly`: run every N hours. */
  hourInterval: number
  /** `hourly`: minute within the hour to fire. */
  atMinute: number
  /** `daily`/`weekly`/`monthly`: hour (0-23). */
  hour: number
  /** `daily`/`weekly`/`monthly`: minute (0-59). */
  minute: number
  /** `weekly`: selected weekdays (0 = Sunday … 6 = Saturday). */
  weekdays: number[]
  /** `monthly`: day of month (1-31). */
  dayOfMonth: number
}

/** A sensible default: daily at 09:00. */
export const DEFAULT_SCHEDULE: SimpleSchedule = {
  mode: 'daily',
  minuteInterval: 5,
  hourInterval: 2,
  atMinute: 0,
  hour: 9,
  minute: 0,
  weekdays: [1],
  dayOfMonth: 1,
}

/** Preset intervals offered for the "every N minutes" mode. */
export const MINUTE_INTERVAL_OPTIONS = [1, 2, 5, 10, 15, 20, 30]

/** Preset intervals offered for the "every N hours" mode. */
export const HOUR_INTERVAL_OPTIONS = [1, 2, 3, 4, 6, 8, 12]

/**
 * Serialize a {@link SimpleSchedule} into a canonical 5-field cron string.
 * Weekdays are emitted sorted and de-duplicated.
 */
export function simpleToCron(s: SimpleSchedule): string {
  switch (s.mode) {
    case 'minutes':
      return `*/${s.minuteInterval} * * * *`
    case 'hourly':
      return `${s.atMinute} */${s.hourInterval} * * *`
    case 'daily':
      return `${s.minute} ${s.hour} * * *`
    case 'weekly':
      return `${s.minute} ${s.hour} * * ${
        s.weekdays.length ? [...new Set(s.weekdays)].sort((a, b) => a - b).join(',') : '*'
      }`
    case 'monthly':
      return `${s.minute} ${s.hour} ${s.dayOfMonth} * *`
  }
}

/** Canonical cron string for {@link DEFAULT_SCHEDULE} (daily at 09:00). */
export const DEFAULT_CRON = simpleToCron(DEFAULT_SCHEDULE)

const NUM = /^\d+$/

function clampCheck(value: number, min: number, max: number): boolean {
  return value >= min && value <= max
}

/**
 * Best-effort parse of a 5-field cron expression into a {@link SimpleSchedule}.
 *
 * Returns `null` when the expression does not match any supported simple
 * pattern (lists in minute/hour, named weekdays, etc.), signaling the caller
 * to fall back to raw editing.
 */
export function cronToSimple(cron: string): SimpleSchedule | null {
  const parts = cron.trim().split(/\s+/)
  if (parts.length !== 5) return null
  const [m, h, dom, mon, dow] = parts as [string, string, string, string, string]
  // We only simplify the common case where month is unrestricted.
  if (mon !== '*') return null

  // minutes: */N * * * *
  const mi = m.match(/^\*\/(\d+)$/)
  if (mi && h === '*' && dom === '*' && dow === '*') {
    const v = parseInt(mi[1] ?? '', 10)
    if (!clampCheck(v, 1, 59)) return null
    return { ...DEFAULT_SCHEDULE, mode: 'minutes', minuteInterval: v }
  }

  // hourly: M */N * * *   (also M * * * * meaning every hour)
  const hi = h.match(/^\*\/(\d+)$/)
  if ((hi || h === '*') && NUM.test(m) && dom === '*' && dow === '*') {
    const mv = parseInt(m, 10)
    const hv = hi ? parseInt(hi[1] ?? '', 10) : 1
    if (!clampCheck(mv, 0, 59) || !clampCheck(hv, 1, 23)) return null
    return { ...DEFAULT_SCHEDULE, mode: 'hourly', hourInterval: hv, atMinute: mv }
  }

  // weekly: M H * * DOW
  if (NUM.test(m) && NUM.test(h) && dom === '*' && dow !== '*' && /^\d+(,\d+)*$/.test(dow)) {
    const mv = parseInt(m, 10)
    const hv = parseInt(h, 10)
    if (!clampCheck(mv, 0, 59) || !clampCheck(hv, 0, 23)) return null
    const wd = dow.split(',').map(Number)
    if (wd.some((d) => !clampCheck(d, 0, 6))) return null
    return { ...DEFAULT_SCHEDULE, mode: 'weekly', hour: hv, minute: mv, weekdays: wd }
  }

  // monthly: M H DOM * *
  if (NUM.test(m) && NUM.test(h) && NUM.test(dom) && dow === '*') {
    const mv = parseInt(m, 10)
    const hv = parseInt(h, 10)
    const dv = parseInt(dom, 10)
    if (!clampCheck(mv, 0, 59) || !clampCheck(hv, 0, 23) || !clampCheck(dv, 1, 31)) return null
    return { ...DEFAULT_SCHEDULE, mode: 'monthly', hour: hv, minute: mv, dayOfMonth: dv }
  }

  // daily: M H * * *
  if (NUM.test(m) && NUM.test(h) && dom === '*' && dow === '*') {
    const mv = parseInt(m, 10)
    const hv = parseInt(h, 10)
    if (!clampCheck(mv, 0, 59) || !clampCheck(hv, 0, 23)) return null
    return { ...DEFAULT_SCHEDULE, mode: 'daily', hour: hv, minute: mv }
  }

  return null
}

/** Validate that a string is a well-formed 5-field cron expression. */
export function isValidCron(cron: string): boolean {
  const parts = cron.trim().split(/\s+/)
  if (parts.length !== 5) return false
  // Each field may contain digits, *, /, -, and commas.
  return parts.every((p) => /^[\d*/,\s-]+$/.test(p) && p.length > 0)
}

/** Format an hour:minute pair as "HH:MM" (24-hour, zero-padded). */
export function formatTime(hour: number, minute: number): string {
  return `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`
}

/** Parse a "HH:MM" string (from `<input type="time">`) into [hour, minute]. */
export function parseTime(value: string): [number, number] {
  const [h = '0', m = '0'] = value.split(':')
  return [parseInt(h, 10) || 0, parseInt(m, 10) || 0]
}
