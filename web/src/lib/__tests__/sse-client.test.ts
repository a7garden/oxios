import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { SseClient } from '@/lib/sse-client'
import { useAuthStore } from '@/stores/auth'

// Mirrors the chat-WS long-tail recovery test (stores.test.ts). The SSE
// client used to give up permanently after MAX_RECONNECT_ATTEMPTS=10,
// stranding the dashboard live-feed at isConnected === false after a long
// daemon/network outage. It now mirrors the WS: fast exponential backoff
// then a steady long-tail retry that never stops, with the counter reset on
// a successful connect.
describe('SseClient reconnect (long-tail recovery)', () => {
  beforeEach(() => {
    useAuthStore.getState().setToken('test-token')
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
    useAuthStore.getState().setToken(null)
  })

  it('keeps retrying past the old hard cap instead of giving up forever', async () => {
    const fetchMock = vi.fn(async () => {
      throw new Error('network down')
    })
    vi.stubGlobal('fetch', fetchMock)

    const client = new SseClient()
    await client.connect('/api/events', vi.fn())

    // Fast backoff 1+2+4+8+16 = 31s (5 attempts), then long-tail 10s each.
    // 120s runs the full backoff plus many long-tail retries.
    await vi.advanceTimersByTimeAsync(120_000)

    // Old behaviour stopped fetching at the cap (≤ 11 calls). Long-tail
    // recovery must keep fetching well past it.
    expect(fetchMock.mock.calls.length).toBeGreaterThanOrEqual(13)
  })

  it('resumes from the fast-backoff window after a successful connection', async () => {
    let succeed = false
    const fetchMock = vi.fn(async () => {
      if (!succeed) throw new Error('network down')
      // Successful response whose body immediately EOFs — onOpen fires
      // (resetting the backoff counter) and the read loop breaks.
      return {
        ok: true,
        status: 200,
        body: {
          getReader: () => ({ read: async () => ({ done: true, value: undefined }) }),
        },
      }
    })
    vi.stubGlobal('fetch', fetchMock)

    const client = new SseClient()
    await client.connect('/api/events', vi.fn())

    // Exhaust the fast backoff (5 failures → counter pinned at the cap).
    await vi.advanceTimersByTimeAsync(31_000)
    expect((globalThis.fetch as Mock).mock.calls.length).toBeGreaterThanOrEqual(6)

    // The next retry is long-tail (10s). Let it succeed, which resets the
    // counter via the onOpen site.
    succeed = true
    await vi.advanceTimersByTimeAsync(11_000)
    const callsAtSuccess = (globalThis.fetch as Mock).mock.calls.length

    // Fail again. If the counter reset worked, the next retry is the
    // fast-backoff 1s (fires within 2s); if it stayed pinned at the cap it
    // would be the 10s long-tail (would not fire within 2s).
    succeed = false
    await vi.advanceTimersByTimeAsync(2_000)
    expect((globalThis.fetch as Mock).mock.calls.length).toBeGreaterThan(callsAtSuccess)
  })
})
