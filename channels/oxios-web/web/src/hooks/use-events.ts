import { useEffect, useRef, useState } from 'react'
import { SseClient } from '@/lib/sse-client'
import type { OxiosEvent } from '@/types'

export function useEvents() {
  const [events, setEvents] = useState<OxiosEvent[]>([])
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<Error | null>(null)
  const clientRef = useRef<SseClient | null>(null)

  useEffect(() => {
    const client = new SseClient()
    clientRef.current = client
    setIsConnected(true)

    client.connect(
      '/api/events/stream',
      (_event, data) => {
        const oxiosEvent = data as OxiosEvent
        setEvents((prev) => [oxiosEvent, ...prev].slice(0, 100))
      },
      (err) => {
        setError(err)
        setIsConnected(false)
      },
    )

    return () => {
      client.disconnect()
      setIsConnected(false)
    }
  }, [])

  return { events, isConnected, error }
}
