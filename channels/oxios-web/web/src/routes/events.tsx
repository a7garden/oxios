import { createFileRoute } from '@tanstack/react-router'
import { Bell, RefreshCw } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { EmptyState } from '@/components/shared/empty-state'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useEvents } from '@/hooks/use-events'
import type { OxiosEvent } from '@/types'

export const Route = createFileRoute('/events')({ component: EventsPage })

function EventsPage() {
  const { events: liveEvents, isConnected, error: connectionError, reconnect } = useEvents()
  const scrollRef = useRef<HTMLDivElement>(null)
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const [_r, setRefreshKey] = useState(0)

  // biome-ignore lint/correctness/useExhaustiveDependencies: liveEvents.length is sufficient dependency for scroll-to-bottom
  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight })
  }, [liveEvents.length])

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Events</h1>
          <p className="text-muted-foreground">Live event stream</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => { reconnect?.(); setRefreshKey((k) => k + 1) }}>
          <RefreshCw className="h-4 w-4 mr-1" /> Refresh
        </Button>
      </div>

      <Card className="flex flex-col">
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <Bell className="h-4 w-4" />
            Event Stream
            <Badge variant="secondary" className="ml-2" aria-live="polite">
              {liveEvents.length}
            </Badge>
            {isConnected && !connectionError && (
              <div className="ml-auto h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
            )}
            {connectionError && (
              <div className="ml-auto flex items-center gap-1.5 text-destructive text-xs">
                <div className="h-2 w-2 rounded-full bg-destructive" />
                Connection lost
              </div>
            )}
          </CardTitle>
        </CardHeader>
        <CardContent className="flex-1">
          {connectionError && (
            <div className="mb-3 rounded border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              Failed to connect to event stream: {connectionError.message}
            </div>
          )}
          <div
            ref={scrollRef}
            className="h-[500px] overflow-y-auto space-y-1"
            role="log"
            aria-label="Event stream"
          >
            {liveEvents.length === 0 ? (
              <EmptyState
                icon={<Bell className="h-8 w-8" />}
                title="No events"
                description="Events will stream in real-time."
                className="py-8"
              />
            ) : (
              liveEvents.map((event: OxiosEvent, i: number) => (
                <div
                  key={event.id ?? `evt-${i}`}
                  className="flex items-start gap-3 rounded border p-2 text-sm"
                >
                  <Badge variant="outline" className="shrink-0 text-xs">
                    {event.type}
                  </Badge>
                  <div className="flex-1 min-w-0">
                    <p className="font-mono text-xs truncate">
                      {event.data
                        ? JSON.stringify(event.data).slice(0, 120)
                        : event.id?.slice(0, 16)}
                    </p>
                  </div>
                  <span className="text-xs text-muted-foreground shrink-0">
                    {event.timestamp ? new Date(event.timestamp).toLocaleTimeString() : '—'}
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
