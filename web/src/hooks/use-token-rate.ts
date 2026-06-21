import { useEffect, useRef, useState } from 'react'
import { useEvents } from '@/hooks/use-events'

/**
 * Compute an approximate "tokens per minute" metric from the SSE stream.
 *
 * Strategy: each `token_usage_update` event reports CUMULATIVE
 * `input_tokens + output_tokens` for a given `session_id`. Because the
 * counter resets per session, a delta between events from different
 * sessions is meaningless (it can be a large negative number that
 * would yield a NaN/garbage rate).
 *
 * To compute a per-session rate, we walk the recent events in
 * chronological order and group them by `session_id`. The most recent
 * two samples per session are kept; the rate is the (last - prev)
 * delta divided by the elapsed time. This handles concurrent sessions
 * (e.g. one agent burning tokens while another idles) correctly and
 * returns 0 when only one sample (or none) is available.
 *
 * The hook is also gated so that `setRate` / `setHistory` only fire
 * when a NEW token event has been seen — the SSE store replaces the
 * `events` array on every tick, so a naive `useEffect([events])` would
 * re-run dozens of times per second and trigger redundant state
 * updates. The fingerprint check (timestamp + session + totals) is
 * stable as long as the underlying event hasn't changed.
 *
 * Smoothing: `0.6 / 0.4` is an exponential moving average chosen so
 * the displayed value reflects roughly the last ~5 samples, which
 * matches the 30-sample ring buffer's natural averaging window.
 *
 * Returns:
 * - `tokensPerMin`: smoothed rate over the last 60s (rolling avg)
 * - `history`: small time-series suitable for a sparkline
 */
const HISTORY_CAP = 30
const SMOOTH_OLD = 0.6
const SMOOTH_NEW = 0.4

interface TokenSample {
  sessionId: string
  ts: number
  total: number
}

type EventList = ReturnType<typeof useEvents>['events']

/**
 * Build a stable identifier for the most recent `token_usage_update`
 * event. Returns `null` if no valid event is present. Used to skip
 * redundant recomputation when non-token events arrive on the SSE
 * stream.
 */
function fingerprint(events: EventList): string | null {
  for (const e of events) {
    if (e.type !== 'token_usage_update') continue
    const rawTs = e.timestamp
    if (typeof rawTs !== 'string') continue
    const ts = new Date(rawTs).getTime()
    if (!Number.isFinite(ts) || ts <= 0) continue
    const inT = Number(e.input_tokens ?? 0)
    const outT = Number(e.output_tokens ?? 0)
    if (!Number.isFinite(inT) || !Number.isFinite(outT)) continue
    const session = (e.session_id as string | undefined) ?? ''
    return `${ts}|${session}|${inT}|${outT}`
  }
  return null
}

/**
 * Walk the most-recent-first events list and, per session, keep the
 * most recent and the second-most recent `token_usage_update` samples.
 * Returns them as a chronologically-sorted array.
 *
 * Exported for unit testing.
 */
export function buildSamples(events: EventList): TokenSample[] {
  const lastPerSession = new Map<string, TokenSample>()
  const prevPerSession = new Map<string, TokenSample>()

  for (const e of events) {
    if (e.type !== 'token_usage_update') continue
    const rawTs = e.timestamp
    if (typeof rawTs !== 'string') continue
    const ts = new Date(rawTs).getTime()
    if (!Number.isFinite(ts) || ts <= 0) continue
    const inT = Number(e.input_tokens ?? 0)
    const outT = Number(e.output_tokens ?? 0)
    if (!Number.isFinite(inT) || !Number.isFinite(outT)) continue
    const sessionId = (e.session_id as string | undefined) ?? '__default__'
    const total = inT + outT
    const existing = lastPerSession.get(sessionId)
    if (existing) {
      prevPerSession.set(sessionId, existing)
    }
    lastPerSession.set(sessionId, { sessionId, ts, total })
  }

  const samples: TokenSample[] = []
  for (const [sessionId, last] of lastPerSession) {
    samples.push(last)
    const prev = prevPerSession.get(sessionId)
    if (prev) samples.push(prev)
  }
  samples.sort((a, b) => a.ts - b.ts)
  return samples
}

/**
 * Compute a system-wide tokens-per-minute rate by summing the
 * per-session deltas from a list of samples produced by `buildSamples`.
 *
 * The cumulative counter is per session, so a delta between two
 * samples from different sessions is meaningless. We group by session
 * and sum each session's (newer - older) / dt * 60 rate. A session
 * with only one sample contributes 0. A session whose counter has
 * reset (negative delta) or whose samples have a non-positive time
 * delta contributes 0. A session with no prior sample contributes 0.
 *
 * Exported for unit testing.
 */
export function computePerMinute(samples: TokenSample[]): number {
  if (samples.length < 2) return 0
  // Group by session; iterate chronologically so we always end up with
  // the LAST (newest) sample in `lastPerSession[sessionId]` and the
  // second-to-last in `prevPerSession[sessionId]`.
  const lastPerSession = new Map<string, TokenSample>()
  const prevPerSession = new Map<string, TokenSample>()
  for (const s of samples) {
    const existing = lastPerSession.get(s.sessionId)
    if (existing) {
      prevPerSession.set(s.sessionId, existing)
    }
    lastPerSession.set(s.sessionId, s)
  }

  let totalRate = 0
  for (const [sessionId, last] of lastPerSession) {
    const prev = prevPerSession.get(sessionId)
    if (!prev) continue
    const dtSec = (last.ts - prev.ts) / 1000
    if (dtSec <= 0) continue
    const tokenDelta = last.total - prev.total
    if (tokenDelta < 0) continue // session restart or counter reset
    totalRate += (tokenDelta / dtSec) * 60
  }
  return totalRate
}

export function useTokenRate() {
  const { events } = useEvents()
  const [rate, setRate] = useState(0)
  const [history, setHistory] = useState<number[]>([])
  const lastFingerprint = useRef<string | null>(null)

  useEffect(() => {
    // Gate: only re-run when a NEW token event has been observed.
    // The events array reference changes on every SSE tick regardless
    // of whether a token event is in it, so we fingerprint the latest
    // token event and short-circuit if it hasn't changed.
    const fp = fingerprint(events)
    if (fp === null) return
    if (fp === lastFingerprint.current) return
    lastFingerprint.current = fp

    const samples = buildSamples(events)
    const perMin = computePerMinute(samples)
    const safe = Number.isFinite(perMin) ? Math.max(0, perMin) : 0

    setRate((prev) => (prev === 0 ? safe : prev * SMOOTH_OLD + safe * SMOOTH_NEW))

    setHistory((h) => {
      const next = [...h, safe]
      return next.length > HISTORY_CAP ? next.slice(-HISTORY_CAP) : next
    })
  }, [events])

  return { tokensPerMin: rate, history }
}
