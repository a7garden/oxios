import { useEffect, useState } from 'react'
import { useEvents } from '@/hooks/use-events'

/**
 * Compute an approximate "tokens per minute" metric from the SSE stream.
 *
 * Strategy: each `TokenUsageUpdate` event reports cumulative
 * `input_tokens + output_tokens` for a session. We track the latest
 * cumulative value per session, and on each update compute the
 * delta-since-last-update. Then we normalise to tokens/min by
 * dividing by the time elapsed since the previous update.
 *
 * Returns:
 * - `tokensPerMin`: smoothed rate over the last 60s (rolling avg)
 * - `history`: small time-series suitable for a sparkline
 */
export function useTokenRate() {
  const { events } = useEvents()
  const [rate, setRate] = useState(0)
  const [history, setHistory] = useState<number[]>([])

  useEffect(() => {
    // Find the most recent TokenUsageUpdate event.
    // The events store keeps most-recent first.
    let totalTokens = 0
    let lastTs: number | null = null
    let prevTokens = 0
    let prevTs: number | null = null

    for (const event of events) {
      if (event.type !== 'token_usage_update') continue
      const inT = Number(event.input_tokens ?? 0)
      const outT = Number(event.output_tokens ?? 0)
      const ts = event.timestamp ? new Date(event.timestamp as string).getTime() : null
      if (ts === null) continue
      if (lastTs === null) {
        // first (most recent) sample — anchor point
        lastTs = ts
        totalTokens = inT + outT
      } else {
        // walk backwards
        if (prevTs === null) {
          prevTs = ts
          prevTokens = inT + outT
        }
      }
    }

    if (lastTs === null || prevTs === null) {
      // Not enough data to compute a rate — leave the rate as-is.
      return
    }

    const tokenDelta = totalTokens - prevTokens
    const dtSec = (lastTs - prevTs) / 1000
    if (dtSec <= 0) return

    const tokensPerMin = (tokenDelta / dtSec) * 60
    const safe = Number.isFinite(tokensPerMin) ? Math.max(0, tokensPerMin) : 0

    setRate((prev) => {
      // Light smoothing so the number doesn't jump wildly
      return prev === 0 ? safe : prev * 0.6 + safe * 0.4
    })

    setHistory((h) => {
      const next = [...h, safe]
      return next.length > 30 ? next.slice(-30) : next
    })
  }, [events])

  return { tokensPerMin: rate, history }
}
