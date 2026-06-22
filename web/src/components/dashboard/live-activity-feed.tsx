import { Link } from '@tanstack/react-router'
import { Pause, Play, Radio, Trash2 } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import { useEvents } from '@/hooks/use-events'
import { formatEvent, isInterestingEvent } from '@/lib/event-formatter'
import { formatRelativeTime } from '@/lib/utils'
import type { OxiosEvent } from '@/types'

/** Cap the rendered list — beyond this we just count and show "more". */
const MAX_VISIBLE = 200

/** Pixel distance from the bottom that still counts as "auto-scroll on". */
const AUTO_SCROLL_THRESHOLD = 24

/**
 * Map the dashboard filter value to a list of event-type prefixes.
 * The single-prefix `startsWith` check can't model "matches several
 * prefixes" (the old "Seeds" option's value was `phase_,evaluation_,seed_`
 * which matched nothing), so each filter maps to one or more prefixes.
 */
const FILTER_PREFIXES: Record<string, string[]> = {
  all: [],
  agent_: ['agent_'],
  tool_: ['tool_'],
  memory_: ['memory_'],
  approval_: ['approval_'],
}

function matchesFilter(type: string, filter: string): boolean {
  const prefixes = Array.isArray(FILTER_PREFIXES[filter]) ? FILTER_PREFIXES[filter] : []
  if (prefixes.length === 0) return true
  return prefixes.some((p) => type.startsWith(p))
}

/**
 * Live activity feed.
 *
 * Subscribes to the singleton SSE event store and shows a filtered,
 * capped list of "interesting" events. Supports a Pause toggle so the
 * user can stop the list from scrolling while investigating. Auto-
 * scroll is disabled when the user scrolls up (Step 4 of the RFC).
 *
 * Two rendering variants:
 * - `"card"` (default): wraps content in its own <Card>. Used when the
 *   feed is rendered standalone (e.g. the /events page).
 * - `"bare"`: renders only the body (header + list). The parent
 *   component provides the Card wrapper. Used inside
 *   `AgentsActivityCard` to avoid Card-in-Card nesting.
 */
export function LiveActivityFeed({ variant = 'card' }: { variant?: 'card' | 'bare' } = {}) {
  const { t } = useTranslation()
  const { events, isConnected, error: connectionError } = useEvents()
  const [paused, setPaused] = useState(false)
  const [frozenEvents, setFrozenEvents] = useState<OxiosEvent[] | null>(null)
  const [filter, setFilter] = useState<string>('all')
  const scrollRef = useRef<HTMLDivElement>(null)
  const [autoScroll, setAutoScroll] = useState(true)

  // Freeze the event list on the transition into `paused`. This runs
  // in an effect (not in render) to avoid the setState-during-render
  // anti-pattern: the previous version called setFrozenEvents directly
  // in the function body, which works only because it's guarded by an
  // idempotency check. The effect form is the correct idiom.
  useEffect(() => {
    if (paused) {
      setFrozenEvents((current) => current ?? events)
    }
  }, [paused, events])

  // When the user pauses, snapshot the current event list and stop
  // updating the visible list. Resume picks up from the live stream.
  const sourceEvents = paused ? (frozenEvents ?? events) : events

  const filtered = useMemo(() => {
    const interesting = sourceEvents.filter(isInterestingEvent)
    const total = interesting.length
    const matched = interesting.filter((e) => matchesFilter(e.type, filter))
    return { visible: matched.slice(0, MAX_VISIBLE), total }
  }, [sourceEvents, filter])

  const overflow = Math.max(0, filtered.total - filtered.visible.length)

  // Auto-scroll: when the list grows, snap to the bottom IF the user
  // hasn't scrolled up. We detect "scrolled up" by checking if the
  // scroll position is within `AUTO_SCROLL_THRESHOLD` of the bottom on
  // every scroll event. This implements the RFC §5 step 4.
  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const onScroll = () => {
      const distance = el.scrollHeight - el.scrollTop - el.clientHeight
      setAutoScroll(distance < AUTO_SCROLL_THRESHOLD)
    }
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [])

  useEffect(() => {
    if (!autoScroll) return
    const el = scrollRef.current
    if (!el) return
    el.scrollTop = el.scrollHeight
  }, [filtered.visible, autoScroll])

  const list = (
    <div
      ref={scrollRef}
      className="h-full overflow-y-auto pr-1"
      role="log"
      aria-label={t('dashboard.liveActivity')}
    >
      {filtered.visible.length === 0 ? (
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
          {filtered.visible.map((event, i) => {
            const fmt = formatEvent(event)
            const Icon = fmt.icon
            const key = (event.id as string | undefined) ?? `evt-${i}-${event.timestamp ?? ''}`
            const time = event.timestamp
              ? new Date(event.timestamp as string).toLocaleTimeString()
              : ''
            const relative = event.timestamp ? formatRelativeTime(event.timestamp as string, t) : ''
            const inner = (
              <div className="flex items-start gap-2 rounded-md border border-transparent px-2 py-1.5 hover:border-border hover:bg-accent/50 transition-all">
                <Icon className={`mt-0.5 h-3.5 w-3.5 shrink-0 ${fmt.color}`} aria-hidden="true" />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-1.5 text-sm">
                    <Badge variant="outline" className="px-1 py-0 text-2xs uppercase">
                      {fmt.label}
                    </Badge>
                    <span className="truncate text-foreground">{fmt.summary}</span>
                  </div>
                </div>
                <span className="shrink-0 text-2xs text-muted-foreground tabular-nums" title={time}>
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
  )

  const controls = (
    <div className="flex items-center gap-1">
      <select
        className="h-7 rounded-md border bg-background px-2 text-xs transition-colors hover:bg-muted/50 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
        aria-label={t('dashboard.filterEvents')}
      >
        <option value="all">{t('dashboard.filterAll')}</option>
        <option value="agent_">{t('dashboard.filterAgents')}</option>
        <option value="tool_">{t('dashboard.filterTools')}</option>
        <option value="memory_">{t('dashboard.filterMemory')}</option>
        <option value="approval_">{t('dashboard.filterApprovals')}</option>
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
  )

  const header = (
    <div className="flex flex-row items-center justify-between space-y-0 pb-2">
      <div className="flex items-center gap-2 text-base font-semibold">
        <Radio className="h-4 w-4" />
        {t('dashboard.liveActivity')}
        <Badge variant="secondary" className="ml-1" aria-live="polite">
          {filtered.total}
        </Badge>
        {isConnected && !connectionError && (
          <span
            className="ml-1 h-2 w-2 rounded-full bg-success animate-pulse"
            aria-label={t('dashboard.connected')}
          />
        )}
        {connectionError && (
          <Badge variant="destructive" className="ml-1">
            {t('dashboard.disconnected')}
          </Badge>
        )}
      </div>
      {controls}
    </div>
  )

  if (variant === 'bare') {
    // Bare variant: parent Card already provides the title. Show only
    // the filter/pause controls to avoid duplicating the title and the
    // "실시간 활동" label inside the card.
    return (
      <div className="flex h-full flex-col">
        <div className="flex items-center justify-end pb-2">{controls}</div>
        {list}
      </div>
    )
  }

  return (
    <Card className="flex h-full flex-col">
      <CardHeader>{header}</CardHeader>
      <CardContent className="flex-1 pt-0">{list}</CardContent>
    </Card>
  )
}
