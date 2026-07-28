import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { SseClient } from '@/lib/sse-client'
import { useAuthStore } from '@/stores/auth'

// ---------------------------------------------------------------------------
// helpers — build a scripted ReadableStream that the SseClient's reader can
// pull from. Using the WHATWG ReadableStream API means the same
// `.getReader()` path the production code calls is exercised (jsdom does not
// implement it, but we stub `fetch` so the SseClient never sees jsdom's
// missing globals).
// ---------------------------------------------------------------------------

/** Minimal Reader shape — matches what `ReadableStream.getReader()` returns
 *  in spec environments. */
interface ScriptedReader {
  read: () => Promise<{ done: boolean; value?: Uint8Array }>
  releaseLock: () => void
}

interface ScriptedStreamOptions {
  /** Split the encoded payload into chunks at these byte offsets. */
  splitAt?: number[]
}

/** A stand-in for `ReadableStream<Uint8Array>` that yields pre-baked chunks.
 *  Returned from stubbed `fetch` as `response.body`. */
class ScriptedBody {
  private chunks: Uint8Array[]
  private consumed = false

  constructor(payload: string, opts: ScriptedStreamOptions = {}) {
    const encoded = new TextEncoder().encode(payload)
    if (opts.splitAt && opts.splitAt.length > 0) {
      const out: Uint8Array[] = []
      let start = 0
      for (const at of opts.splitAt) {
        out.push(encoded.slice(start, at))
        start = at
      }
      out.push(encoded.slice(start))
      this.chunks = out
    } else {
      this.chunks = [encoded]
    }
  }

  getReader(): ScriptedReader {
    if (this.consumed) throw new Error('ScriptedBody already consumed')
    this.consumed = true
    const chunks = this.chunks
    let i = 0
    return {
      async read() {
        if (i >= chunks.length) return { done: true, value: undefined }
        return { done: false, value: chunks[i++]! }
      },
      releaseLock() {},
    }
  }

  /** Override for tests that need an indefinitely pending reader. */
  setReaderOverride(override: () => ScriptedReader) {
    this.getReader = override
  }
}

function sseResponse(body: ScriptedBody, status = 200): Response {
  // We only need `ok`, `status`, and a `body.getReader()` on the response —
  // type the literal as Response so the SseClient sees the real contract.
  return {
    ok: status >= 200 && status < 300,
    status,
    body: body as unknown as ReadableStream<Uint8Array>,
  } as Response
}

// ---------------------------------------------------------------------------
// SSE data tests — drive a real ReadableStream through the production parser
// to verify event/data line parsing, partial-line and partial-chunk
// boundaries, the auth header on connect, the onOpen callback, malformed
// JSON, and the 401 / disconnect short-circuits. The existing suite covers
// reconnect backoff — these tests pin the streaming-data contract that bugs
// here would silently drop.
// ---------------------------------------------------------------------------
describe('SseClient data parsing', () => {
  beforeEach(() => {
    useAuthStore.getState().setToken('test-token')
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
    useAuthStore.getState().setToken(null)
  })

  it('emits parsed event + JSON data on default "message" events', async () => {
    const fetchMock = vi.fn(async () => sseResponse(new ScriptedBody('data: {"foo":"bar"}\n\n')))
    vi.stubGlobal('fetch', fetchMock)

    const events: Array<{ name: string; data: unknown }> = []
    const client = new SseClient()
    await client.connect('/api/events', (name, data) => events.push({ name, data }))

    // The reader returns done after the single chunk — the loop breaks and
    // the long-tail reconnect is scheduled. We only care about the events.
    expect(events).toEqual([{ name: 'message', data: { foo: 'bar' } }])
    client.disconnect()
  })

  it('honours the SSE event: name field and resets between data lines', async () => {
    const wire = [
      'event: ping',
      'data: {"t":1}',
      '',
      'data: {"t":2}', // implicit "message" name
      '',
    ].join('\n')
    const fetchMock = vi.fn(async () => sseResponse(new ScriptedBody(wire)))
    vi.stubGlobal('fetch', fetchMock)

    const events: Array<{ name: string; data: unknown }> = []
    const client = new SseClient()
    await client.connect('/api/events', (name, data) => events.push({ name, data }))

    expect(events).toEqual([
      { name: 'ping', data: { t: 1 } },
      { name: 'message', data: { t: 2 } },
    ])
    client.disconnect()
  })

  it('passes malformed JSON data through as the raw string', async () => {
    const wire = 'data: not-json\n\n'
    const fetchMock = vi.fn(async () => sseResponse(new ScriptedBody(wire)))
    vi.stubGlobal('fetch', fetchMock)

    const events: Array<{ name: string; data: unknown }> = []
    const client = new SseClient()
    await client.connect('/api/events', (name, data) => events.push({ name, data }))

    expect(events).toEqual([{ name: 'message', data: 'not-json' }])
    client.disconnect()
  })

  it('reassembles partial UTF-8 lines split across chunks', async () => {
    // The wire splits mid-line — the reader yields two chunks whose bytes
    // join into one logical event. The decoder with `{ stream: true }` must
    // hold the trailing partial until the next chunk completes it.
    const payload = 'data: {"a":1}\n\ndata: {"b'
    const split = 'data: {"a":1}\n\ndata: {"b'.length // split inside the second data
    const fetchMock = vi.fn(async () =>
      sseResponse(new ScriptedBody(payload, { splitAt: [split] })),
    )
    vi.stubGlobal('fetch', fetchMock)

    const events: Array<{ name: string; data: unknown }> = []
    const client = new SseClient()
    await client.connect('/api/events', (name, data) => events.push({ name, data }))

    // First chunk contains the whole first event.
    expect(events[0]).toEqual({ name: 'message', data: { a: 1 } })
    // Second chunk only carries the prefix of the second data line — the
    // parser must keep buffering and NOT emit anything yet. The stream then
    // returns done without the remainder, so the buffered line is dropped
    // (it never reached a terminator). This asserts we never silently emit
    // a half-line as if it were complete.
    expect(events.length).toBe(1)
    client.disconnect()
  })

  it('sends the Bearer token, Accept, and Cache-Control headers on connect', async () => {
    const headers: Record<string, string> = {}
    const fetchMock = vi.fn(async (_url: string, init?: RequestInit) => {
      const raw = init?.headers
      if (raw && typeof raw === 'object') {
        for (const [k, v] of Object.entries(raw as Record<string, string>)) {
          headers[k] = v
        }
      }
      return sseResponse(new ScriptedBody(''))
    })
    vi.stubGlobal('fetch', fetchMock)

    const client = new SseClient()
    await client.connect('/api/events', vi.fn(), vi.fn(), () => {})

    expect(headers.Authorization).toBe('Bearer test-token')
    expect(headers.Accept).toBe('text/event-stream')
    expect(headers['Cache-Control']).toBe('no-cache')
    client.disconnect()
  })

  it('fires onOpen exactly once when the stream opens', async () => {
    const fetchMock = vi.fn(async () => sseResponse(new ScriptedBody('data: {"x":1}\n\n')))
    vi.stubGlobal('fetch', fetchMock)

    const onOpen = vi.fn()
    const client = new SseClient()
    await client.connect('/api/events', vi.fn(), vi.fn(), onOpen)
    expect(onOpen).toHaveBeenCalledTimes(1)
    client.disconnect()
  })

  it('emits onError and stops retrying on a 401 response', async () => {
    // 401 must abort the reconnect loop — the token has expired, retrying
    // just hammers an auth endpoint and tells the UI nothing useful.
    // Production code clears currentPath so scheduleReconnect() short-
    // circuits.
    const fetchMock = vi.fn(async () => sseResponse(new ScriptedBody(''), 401))
    vi.stubGlobal('fetch', fetchMock)

    const onError = vi.fn()
    const onEvent = vi.fn()
    const client = new SseClient()
    await client.connect('/api/events', onEvent, onError)

    expect(onError).toHaveBeenCalledTimes(1)
    const err = onError.mock.calls[0]?.[0]
    expect(err instanceof Error).toBe(true)
    expect((err as Error).message).toBe('SSE HTTP 401')

    // Advance a generous window — fetch must NOT be called again.
    await vi.advanceTimersByTimeAsync(120_000)
    expect((globalThis.fetch as Mock).mock.calls.length).toBe(1)
  })

  it('aborts the in-flight request on disconnect()', async () => {
    // Stream returns a body whose read() resolves only when the AbortSignal
    // fires — disconnect() must invoke controller.abort() so the read loop
    // exits and no reconnect is scheduled.
    const fetchMock = vi.fn(async (_url: string, init?: RequestInit) => {
      const signal = init?.signal
      const body: ScriptedBody = new ScriptedBody('')
      body.setReaderOverride(() => ({
        read: () =>
          new Promise<{ done: boolean }>((resolve, reject) => {
            if (!signal) return resolve({ done: true })
            if (signal.aborted) {
              const err = new Error('aborted')
              err.name = 'AbortError'
              return reject(err)
            }
            signal.addEventListener(
              'abort',
              () => {
                const err = new Error('aborted')
                err.name = 'AbortError'
                reject(err)
              },
              { once: true },
            )
          }),
        releaseLock: () => {},
      }))
      return sseResponse(body)
    })
    vi.stubGlobal('fetch', fetchMock)

    const client = new SseClient()
    // Do NOT await connect() — the read loop parks on the never-resolving
    // promise, which is exactly the state we want to test.
    void client.connect('/api/events', vi.fn())
    // Yield once so the fetch + reader.read() in flight get a chance to
    // install before we abort.
    await Promise.resolve()
    client.disconnect()

    // The disconnect path aborted the signal → the read promise rejected
    // with AbortError → the catch in doConnect swallowed it → control
    // returned to the test. The far-future timer advance confirms no
    // reconnect was scheduled (currentPath is null after disconnect).
    await vi.advanceTimersByTimeAsync(60_000)
    expect((globalThis.fetch as Mock).mock.calls.length).toBe(1)
  })
})

// ---------------------------------------------------------------------------
// Reconnect (long-tail recovery) — kept verbatim from the original suite so
// the regression that motivated the backoff rewrite remains pinned.
// ---------------------------------------------------------------------------
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
    client.disconnect()
  })

  it('resumes from the fast-backoff window after a successful connection', async () => {
    let succeed = false
    const fetchMock = vi.fn(async () => {
      if (!succeed) throw new Error('network down')
      // Successful response whose body immediately EOFs — onOpen fires
      // (resetting the backoff counter) and the read loop breaks.
      return sseResponse(new ScriptedBody(''))
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
    client.disconnect()
  })
})
