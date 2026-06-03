import { Link } from '@tanstack/react-router'
import { Pause, Play, Radio, Trash2 } from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useEvents } from '@/hooks/use-events'
import { formatEvent, isInterestingEvent } from '@/lib/event-formatter'
import { formatRelativeTime } from '@/lib/utils'
import type { OxiosEvent } from '@/types'

/** Cap the rendered list — beyond this we just count and show "more". */
const MAX_VISIBLE = 200

/**
 * Live activity feed.
 *
 * Subscribes to the singleton SSE event store and shows a filtered,
 * capped list of "interesting" events. Supports a Pause toggle so the
 * user can stop the list from scrolling while investigating.
 */
export function LiveActivityFeed() {
  const { t } = useTranslation()
  const { events, isConnected, error: connectionError } = useEvents()
  const [paused, setPaused] = useState(false)
  const [frozenEvents, setFrozenEvents] = useState<OxiosEvent[] | null>(null)
  const [filter, setFilter] = useState<string>('all')
  const scrollRef = useRef<HTMLDivElement>(null)

  // When the user pauses, snapshot the current event list and stop
  // updating the visible list. Resume picks up from the live stream.
  const sourceEvents = paused ? (frozenEvents ?? events) : events

  // If we just transitioned to paused, freeze the current events.
  if (paused && frozenEvents === null) {
    setFrozenEvents(events)
  }

  const filtered = useMemo(() => {
    const filtered = sourceEvents.filter(isInterestingEvent)
    if (filter === 'all') return filtered.slice(0, MAX_VISIBLE)
    return filtered.filter((e) => e.type.startsWith(filter)).slice(0, MAX_VISIBLE)
  }, [sourceEvents, filter])

  const total = sourceEvents.filter(isInterestingEvent).length
  const overflow = Math.max(0, total - filtered.length)

  return (
    <Card className="flex h-full flex-col">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Radio className="h-4 w-4" />
          {t('dashboard.liveActivity')}
          <Badge variant="secondary" className="ml-1" aria-live="polite">
            {total}
          </Badge>
          {isConnected && !connectionError && (
            <span
              className="ml-1 h-2 w-2 rounded-full bg-emerald-500 animate-pulse"
              aria-label={t('dashboard.connected')}
            />
          )}
          {connectionError && (
            <Badge variant="destructive" className="ml-1">
              {t('dashboard.disconnected')}
            </Badge>
          )}
        </CardTitle>
        <div className="flex items-center gap-1">
          <select
            className="h-7 rounded-md border bg-background px-2 text-xs"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            aria-label={t('dashboard.filterEvents')}
          >
            <option value="all">{t('dashboard.filterAll')}</option>
            <option value="agent_">{t('dashboard.filterAgents')}</option>
            <option value="tool_">{t('dashboard.filterTools')}</option>
            <option value="memory_">{t('dashboard.filterMemory')}</option>
            <option value="approval_">{t('dashboard.filterApprovals')}</option>
            <option value="phase_,evaluation_,seed_">{t('dashboard.filterSeeds')}</option>
          </select>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={() => {
              if (paused) {
                setFrozenEvents(null)
                setPaused(false)
              } else {
                setPaused(true)
              }
            }}
            aria-pressed={paused}
            aria-label={paused ? t('dashboard.resume') : t('dashboard.pause')}
            title={paused ? t('dashboard.resume') : t('dashboard.pause')}
          >
            {paused ? <Play className="h-3.5 w-3.5" /> : <Pause className="h-3.5 w-3.5" />}
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex-1 pt-0">
        <div
          ref={scrollRef}
          className="h-[360px] overflow-y-auto pr-1"
          role="log"
          aria-label={t('dashboard.liveActivity')}
        >
          {filtered.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-center text-muted-foreground">
              {paused ? (
                <>
                  <Pause className="h-8 w-8" />
                  <p className="text-sm">{t('dashboard.pausedHint')}</p>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => {
                      setFrozenEvents(null)
                      setPaused(false)
                    }}
                  >
                    <Trash2 className="h-3.5 w-3.5 mr-1" /> {t('dashboard.clearAndResume')}
                  </Button>
                </>
              ) : (
                <>
                  <Radio className="h-8 w-8" />
                  <p className="text-sm">{t('dashboard.noActivityYet')}</p>
                </>
              )}
            </div>
          ) : (
            <ul className="space-y-1">
              {filtered.map((event, i) => {
                const fmt = formatEvent(event)
                const Icon = fmt.icon
                const key = (event.id as string | undefined) ?? `evt-${i}-${event.timestamp ?? ''}`
                const time = event.timestamp
                  ? new Date(event.timestamp as string).toLocaleTimeString()
                  : ''
                const relative = event.timestamp
                  ? formatRelativeTime(event.timestamp as string)
                  : ''
                const inner = (
                  <div className="flex items-start gap-2 rounded-md border border-transparent px-2 py-1.5 hover:border-border hover:bg-accent/30 transition-colors">
                    <Icon
                      className={`mt-0.5 h-3.5 w-3.5 shrink-0 ${fmt.color}`}
                      aria-hidden="true"
                    />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-1.5 text-sm">
                        <Badge variant="outline" className="px-1 py-0 text-[10px] uppercase">
                          {fmt.label}
                        </Badge>
                        <span className="truncate text-foreground">{fmt.summary}</span>
                      </div>
                    </div>
                    <span
                      className="shrink-0 text-[10px] text-muted-foreground tabular-nums"
                      title={time}
                    >
                      {relative}
                    </span>
                  </div>
                )
                return (
                  <li key={key}>
                    {fmt.href ? (
                      <Link to={fmt.href} className="block">
                        {inner}
                      </Link>
                    ) : (
                      inner
                    )}
                  </li>
                )
              })}
              {overflow > 0 && (
                <li className="text-center text-xs text-muted-foreground py-2">
                  {t('dashboard.moreEvents', { count: overflow })}
                </li>
              )}
            </ul>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
