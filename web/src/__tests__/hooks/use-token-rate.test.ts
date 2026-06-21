import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { computePerMinute, useTokenRate } from '@/hooks/use-token-rate'
import { useEventStore } from '@/stores/events'
import type { OxiosEvent } from '@/types'

/**
 * Build a `token_usage_update` event with cumulative token counts.
 * `cumulative` is the session-cumulative value at `ts`; `delta` is the
 * number of new tokens consumed in the window ending at `ts`.
 */
function tokenEvent(opts: {
  sessionId: string
  ts: number // epoch ms
  inputTokens: number
  outputTokens: number
}): OxiosEvent {
  return {
    type: 'token_usage_update',
    session_id: opts.sessionId,
    timestamp: new Date(opts.ts).toISOString(),
    input_tokens: opts.inputTokens,
    output_tokens: opts.outputTokens,
  }
}

describe('useTokenRate — per-session rate', () => {
  beforeEach(() => {
    // Reset the singleton store between tests so events don't leak.
    useEventStore.setState({ events: [], isConnected: false, error: null })
  })

  it('returns 0 when there are no token events', () => {
    const { result } = renderHook(() => useTokenRate())
    expect(result.current.tokensPerMin).toBe(0)
    expect(result.current.history).toEqual([])
  })

  it('computes a non-NaN rate when events from two different sessions are interleaved', async () => {
    // Session A: cumulative went from 1000 → 2000 over 60s ⇒ 1000 tokens/min
    // Session B: cumulative went from 500 → 500 over 30s  ⇒ 0 tokens/min
    // The old code would have computed a meaningless cross-session delta
    // (last cumulative from A minus previous from B) and produced NaN or
    // a garbage number. The new code groups by session, computes each
    // session's rate, and sums them. Expected system rate: ~1000
    // tokens/min.
    const now = Date.now()
    const events: OxiosEvent[] = [
      // Most recent first (per store convention).
      tokenEvent({ sessionId: 'B', ts: now, inputTokens: 300, outputTokens: 200 }),
      tokenEvent({ sessionId: 'A', ts: now - 1000, inputTokens: 1200, outputTokens: 800 }),
      tokenEvent({ sessionId: 'B', ts: now - 30_000, inputTokens: 250, outputTokens: 250 }),
      tokenEvent({ sessionId: 'A', ts: now - 60_000, inputTokens: 600, outputTokens: 400 }),
    ]

    act(() => {
      useEventStore.setState({ events, isConnected: true, error: null })
    })

    const { result } = renderHook(() => useTokenRate())

    await waitFor(() => {
      expect(result.current.tokensPerMin).not.toBeNaN()
    })

    // Per-session A: 2000 - 1000 = 1000 tokens, 59s window → ~1017 tokens/min
    // Per-session B: 500 - 500 = 0 tokens, 30s window → 0 tokens/min
    // Total: ~1017 tokens/min
    expect(Number.isFinite(result.current.tokensPerMin)).toBe(true)
    expect(result.current.tokensPerMin).toBeGreaterThan(800)
    expect(result.current.tokensPerMin).toBeLessThan(1300)
    expect(result.current.history.length).toBeGreaterThan(0)
  })

  it('returns 0 when there is only a single token event for a session', async () => {
    const now = Date.now()
    const events: OxiosEvent[] = [
      tokenEvent({ sessionId: 'A', ts: now, inputTokens: 100, outputTokens: 50 }),
    ]
    act(() => {
      useEventStore.setState({ events, isConnected: true, error: null })
    })
    const { result } = renderHook(() => useTokenRate())
    // First event: no delta yet, rate stays 0.
    expect(result.current.tokensPerMin).toBe(0)
  })

  it('does not fire setHistory on non-token SSE events (memoization)', async () => {
    const now = Date.now()
    const token = tokenEvent({
      sessionId: 'A',
      ts: now,
      inputTokens: 1000,
      outputTokens: 500,
    })
    act(() => {
      useEventStore.setState({ events: [token], isConnected: true, error: null })
    })
    const { result } = renderHook(() => useTokenRate())
    await waitFor(() => expect(result.current.history.length).toBe(1))

    // Push a non-token event. The fingerprint should not change, so
    // history should NOT grow.
    const nonToken: OxiosEvent = {
      type: 'agent_started',
      agent_id: 'agent-1',
      timestamp: new Date(now + 1000).toISOString(),
    }
    act(() => {
      useEventStore.setState({
        events: [nonToken, token],
        isConnected: true,
        error: null,
      })
    })

    // Give the effect a tick to (not) run.
    await new Promise((r) => setTimeout(r, 10))
    expect(result.current.history.length).toBe(1)
  })
})

describe('computePerMinute — pure function', () => {
  it('returns 0 for an empty list', () => {
    expect(computePerMinute([])).toBe(0)
  })

  it('returns 0 when only one sample is provided', () => {
    expect(computePerMinute([{ sessionId: 'A', ts: 1000, total: 100 }])).toBe(0)
  })

  it('computes a per-session rate ignoring mixed-session events', () => {
    // Two samples from session A: 100 → 200 over 60s ⇒ 100 tokens/min
    // Plus a sample from session B in between.
    const samples = [
      { sessionId: 'B', ts: 0, total: 999 },
      { sessionId: 'A', ts: 0, total: 100 },
      { sessionId: 'A', ts: 60_000, total: 200 },
    ]
    const rate = computePerMinute(samples)
    expect(rate).toBeCloseTo(100, 5)
  })

  it('returns 0 when only one session has a single sample and a different session has a single sample', () => {
    // Two sessions, each with exactly one sample — no per-session
    // deltas to compute. Function should return 0.
    const samples = [
      { sessionId: 'B', ts: 0, total: 100 },
      { sessionId: 'A', ts: 60_000, total: 500 },
    ]
    expect(computePerMinute(samples)).toBe(0)
  })

  it('sums per-session rates when multiple sessions have valid deltas', () => {
    // Session A: 0 → 100 over 60s ⇒ 100 tokens/min
    // Session B: 0 → 60 over 60s  ⇒ 60 tokens/min
    // Total: 160 tokens/min
    const samples = [
      { sessionId: 'A', ts: 0, total: 0 },
      { sessionId: 'B', ts: 0, total: 0 },
      { sessionId: 'A', ts: 60_000, total: 100 },
      { sessionId: 'B', ts: 60_000, total: 60 },
    ]
    expect(computePerMinute(samples)).toBeCloseTo(160, 5)
  })

  it('returns 0 when the cumulative counter has reset (negative delta)', () => {
    const samples = [
      { sessionId: 'A', ts: 0, total: 1000 },
      { sessionId: 'A', ts: 60_000, total: 100 },
    ]
    expect(computePerMinute(samples)).toBe(0)
  })
})
