import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Bell, RefreshCw } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { ErrorState } from '@/components/shared/error-state'
import { EmptyState } from '@/components/shared/empty-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import { useEvents } from '@/hooks/use-events'
import type { OxiosEvent } from '@/types'

export const Route = createFileRoute('/events')({ component: EventsPage })

function EventsPage() {
  const { events: liveEvents, isConnected, error: connectionError } = useEvents()
  const scrollRef = useRef<HTMLDivElement>(null)

  const { isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['events'],
    queryFn: () => api.get<{ items: OxiosEvent[] }>('/api/events?limit=50'),
  })

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight })
  }, [liveEvents.length])

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

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
              liveEvents.map((event, i) => (
                <div
                  key={event.id ?? i}
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
