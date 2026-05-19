import { createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { LoadingCards } from '@/components/shared/loading'
import { EmptyState } from '@/components/shared/empty-state'
import { Bell, RefreshCw } from 'lucide-react'
import type { OxiosEvent } from '@/types'
import { useState, useEffect, useRef } from 'react'
import { SseClient } from '@/lib/sse-client'

export const Route = createFileRoute('/events')({ component: EventsPage })

function EventsPage() {
  const [liveEvents, setLiveEvents] = useState<OxiosEvent[]>([])
  const sseRef = useRef<SseClient | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  const { data: initial, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['events'],
    queryFn: () => api.get<{ items: OxiosEvent[] }>('/api/events?limit=50'),
  })

  useEffect(() => {
    const events = initial?.items ?? []
    if (events.length > 0 && liveEvents.length === 0) {
      setLiveEvents(events.reverse())
    }
  }, [initial])

  useEffect(() => {
    const token = localStorage.getItem('oxios-api-key') || ''
    const client = new SseClient('/api/events/stream', token, (event) => {
      setLiveEvents((prev) => [...prev.slice(-99), event as OxiosEvent])
      scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight })
    })
    sseRef.current = client
    client.connect()
    return () => client.close()
  }, [])

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight })
  }, [liveEvents])

  if (isLoading) return <LoadingCards count={4} />

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Events</h1>
          <p className="text-muted-foreground">Live event stream</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
          <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
        </Button>
      </div>

      <Card className="flex flex-col">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <Bell className="h-4 w-4" />
            Event Stream
            <Badge variant="secondary" className="ml-2">{liveEvents.length}</Badge>
            <div className="ml-auto h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1">
          <div ref={scrollRef} className="h-[500px] overflow-y-auto space-y-1">
            {liveEvents.length === 0 ? (
              <EmptyState
                icon={<Bell className="h-8 w-8" />}
                title="No events"
                description="Events will stream in real-time."
                className="py-8"
              />
            ) : (
              liveEvents.map((event, i) => (
                <div key={event.id ?? i} className="flex items-start gap-3 rounded border p-2 text-sm">
                  <Badge variant="outline" className="shrink-0 text-xs">{event.type}</Badge>
                  <div className="flex-1 min-w-0">
                    <p className="font-mono text-xs truncate">
                      {event.data ? JSON.stringify(event.data).slice(0, 120) : event.id?.slice(0, 16)}
                    </p>
                  </div>
                  <span className="text-xs text-muted-foreground shrink-0">
                    {new Date(event.timestamp).toLocaleTimeString()}
                  </span>
                </div>
              ))
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
