import { create } from 'zustand'

/**
 * Global event store — single source of truth for SSE events.
 *
 * Problem: both `useGlobalEvents` (in AppLayout) and `/events` page call
 * `useEvents()` which each creates a separate SSE connection. With this
 * central store, only ONE connection exists, and all consumers read from it.
 */

import { SseClient } from '@/lib/sse-client'
import type { OxiosEvent } from '@/types'

const MAX_EVENTS = 100

interface EventState {
  events: OxiosEvent[]
  isConnected: boolean
  error: Error | null
  /** Connect the singleton SSE stream. Idempotent — safe to call multiple times. */
  connect: () => void
  /** Reconnect (clear events + reconnect). */
  reconnect: () => void
}

let client: SseClient | null = null

export const useEventStore = create<EventState>((set, get) => ({
  events: [],
  isConnected: false,
  error: null,

  connect() {
    // Already connected
    if (client) return

    const sse = new SseClient()
    client = sse
    set({ error: null })

    sse.connect(
      '/api/events',
      (event, data) => {
        // RFC-024 SP2 (C2 resync path): the server emits a `resync`
        // event when the broadcast bus lagged and dropped events. The
        // client cannot reconstruct the missed stream, so it pulls
        // fresh state via the regular HTTP API and clears the in-memory
        // event log so the UI does not display a mix of stale and
        // current entries. We re-trigger any consumer that has a
        // refresh hook attached (e.g. useApprovalWatcher via a query
        // invalidation in AppLayout).
        if (event === 'resync') {
          set({ events: [] })
          window.dispatchEvent(new CustomEvent('oxios:resync'))
          return
        }
        const evt = data as OxiosEvent
        set((s) => ({ events: [evt, ...s.events].slice(0, MAX_EVENTS) }))
      },
      (err) => {
        set({ error: err, isConnected: false })
        client = null
      },
      () => {
        // onOpen callback — connected successfully
        set({ isConnected: true })
      },
    )
  },

  reconnect() {
    client?.disconnect()
    client = null
    set({ events: [], isConnected: false, error: null })
    get().connect()
  },
}))

/**
 * Convenience hook: returns the event store state.
 * Automatically connects on first mount via zustand (connect called in AppLayout).
 */
export function useEvents() {
  const events = useEventStore((s) => s.events)
  const isConnected = useEventStore((s) => s.isConnected)
  const error = useEventStore((s) => s.error)
  const reconnect = useEventStore((s) => s.reconnect)
  return { events, isConnected, error, reconnect }
}
