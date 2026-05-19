import { useEffect, useRef, useState } from 'react'
import { SseClient } from '@/lib/sse-client'
import type { OxiosEvent } from '@/types'

export function useEvents() {
  const [events, setEvents] = useState<OxiosEvent[]>([])
  const clientRef = useRef<SseClient | null>(null)

  useEffect(() => {
    const client = new SseClient()
    clientRef.current = client

    client.connect(
      '/api/events',
      (_event, data) => {
        const oxiosEvent = data as OxiosEvent
        setEvents((prev) => [oxiosEvent, ...prev].slice(0, 100))
      },
      (err) => {
        console.error('SSE error:', err)
      },
    )

    return () => {
      client.disconnect()
    }
  }, [])

  return { events }
}