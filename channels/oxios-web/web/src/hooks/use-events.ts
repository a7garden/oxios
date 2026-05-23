import { useCallback, useEffect, useRef, useState } from 'react'
import { SseClient } from '@/lib/sse-client'
import type { OxiosEvent } from '@/types'

export function useEvents() {
  const [events, setEvents] = useState<OxiosEvent[]>([])
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<Error | null>(null)
  const clientRef = useRef<SseClient | null>(null)

  const connect = useCallback(() => {
    // Disconnect previous
    clientRef.current?.disconnect()

    const client = new SseClient()
    clientRef.current = client
    setIsConnected(true)
    setError(null)

    client.connect(
      '/api/events',
      (_event, data) => {
        const oxiosEvent = data as OxiosEvent
        setEvents((prev) => [oxiosEvent, ...prev].slice(0, 100))
      },
      (err) => {
        setError(err)
        setIsConnected(false)
      },
    )
  }, [])

  useEffect(() => {
    connect()
    return () => {
      clientRef.current?.disconnect()
      setIsConnected(false)
    }
  }, [connect])

  const reconnect = useCallback(() => {
    setEvents([])
    connect()
  }, [connect])

  return { events, isConnected, error, reconnect }
}
