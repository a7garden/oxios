import { useState } from 'react'
import { ChevronDown, ChevronRight, ListTree } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { ChatActivity } from '@/types'
import { ActivityCard } from './activity-card'

interface ActivityTimelineProps {
  activities: ChatActivity[]
  className?: string
  /** Auto-collapse after this many cards to keep the chat view calm. */
  collapseAfter?: number
  /** Render a header strip summarising tool calls / token usage. */
  showHeader?: boolean
}

/**
 * RFC-015: renders the chat transparency timeline for one assistant turn.
 *
 * Each activity is rendered as a collapsed card by default; the whole
 * timeline is itself collapsible when it grows large. The header strip
 * shows a quick summary (number of tool calls, total tokens) so the
 * user has a sense of activity even when collapsed.
 */
export function ActivityTimeline({
  activities,
  className,
  collapseAfter = 8,
  showHeader = true,
}: ActivityTimelineProps) {
  const { t } = useTranslation()
  const [timelineCollapsed, setTimelineCollapsed] = useState(activities.length > collapseAfter)
  const [cardsCollapsed, setCardsCollapsed] = useState(false)

  if (!activities.length) return null

  const toolCount = activities.filter((a) => a.type === 'tool_call').length
  const totalInput = activities.reduce((s, a) => s + (a.inputTokens ?? 0), 0)
  const totalOutput = activities.reduce((s, a) => s + (a.outputTokens ?? 0), 0)

  return (
    <div className={cn('my-2 ml-1 space-y-1', className)}>
      {showHeader && (
        <button
          type="button"
          onClick={() => setTimelineCollapsed((v) => !v)}
          className="flex w-full items-center gap-2 text-[11px] text-muted-foreground hover:text-foreground transition-colors"
          aria-expanded={!timelineCollapsed}
        >
          {timelineCollapsed ? (
            <ChevronRight className="h-3 w-3" />
          ) : (
            <ChevronDown className="h-3 w-3" />
          )}
          <ListTree className="h-3 w-3" />
          <span className="font-medium">
            {t('chat.transparency.timelineHeader', { count: activities.length })}
          </span>
          {toolCount > 0 && (
            <span className="text-muted-foreground/70">
              · {t('chat.transparency.toolCallCount', { count: toolCount })}
            </span>
          )}
          {totalInput + totalOutput > 0 && (
            <span className="text-muted-foreground/70">
              · {t('chat.transparency.tokenCount', {
                count: totalInput + totalOutput,
              })}
            </span>
          )}
        </button>
      )}

      {!timelineCollapsed && (
        <div className="space-y-1">
          {activities.length > collapseAfter && (
            <button
              type="button"
              onClick={() => setCardsCollapsed((v) => !v)}
              className="text-[10px] text-muted-foreground hover:text-foreground"
            >
              {cardsCollapsed
                ? t('chat.transparency.expandAll')
                : t('chat.transparency.collapseAll')}
            </button>
          )}
          <div
            className={cn(
              'space-y-1 transition-opacity',
              cardsCollapsed && 'opacity-70',
            )}
          >
            {activities.map((a) => (
              <ActivityCard key={a.id} activity={a} />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
