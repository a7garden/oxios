import {
  Brain,
  ChevronDown,
  ChevronRight,
  Clock,
  Cpu,
  Loader2,
  Sparkles,
  Wrench,
} from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { ChatActivity } from '@/types'
import { BrowseContextBadge } from './browse-context-badge'
import { BrowseContextDetail } from './browse-context-detail'

interface ActivityCardProps {
  activity: ChatActivity
  className?: string
}

/**
 * RFC-015: a single transparency activity entry. Collapsed by default
 * (header only), expanded on click. Mirrors the visual rhythm of the
 * Claude Code / Gemini tool-call cards.
 */
export function ActivityCard({ activity, className }: ActivityCardProps) {
  const { t } = useTranslation()
  const [expanded, setExpanded] = useState(false)
  const { icon, label, badge } = getActivityMeta(activity, t)
  const durationStr = formatDuration(activity.durationMs)

  return (
    <div className={cn('rounded-lg border bg-muted/30 text-xs', className)}>
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-1.5 text-left transition-colors hover:bg-muted/50"
        aria-expanded={expanded}
      >
        {expanded ? (
          <ChevronDown className="h-3 w-3 shrink-0" />
        ) : (
          <ChevronRight className="h-3 w-3 shrink-0" />
        )}
        <span className="shrink-0 text-muted-foreground">{icon}</span>
        <span className="font-medium truncate">{label}</span>
        {activity.type === 'tool_call' && activity.isRunning && (
          <Loader2 className="h-3 w-3 animate-spin text-muted-foreground shrink-0" />
        )}
        {activity.type === 'tool_call' && activity.tabId && (
          <span
            className="text-2xs text-muted-foreground/70 font-mono shrink-0"
            title={`Browser tab ${activity.tabId}`}
          >
            {activity.tabId.slice(0, 8)}
          </span>
        )}
        {activity.type === 'tool_call' && activity.progress && !activity.context && (
          <span className="text-2xs text-muted-foreground truncate max-w-[40ch]">
            {activity.progress}
          </span>
        )}
        {activity.type === 'tool_call' && activity.context && (
          <BrowseContextBadge context={activity.context} />
        )}
        {badge}
        {durationStr && (
          <span className="ml-auto flex items-center gap-1 text-muted-foreground">
            <Clock className="h-3 w-3" /> {durationStr}
          </span>
        )}
      </button>
      {expanded && (
        <div className="border-t px-3 py-2 space-y-2">
          <ActivityDetail activity={activity} t={t} />
        </div>
      )}
    </div>
  )
}

type Translator = (key: string, opts?: Record<string, unknown>) => string

function ActivityDetail({ activity, t }: { activity: ChatActivity; t: Translator }) {
  switch (activity.type) {
    case 'tool_call':
      return (
        <>
          {activity.context && <BrowseContextDetail context={activity.context} />}
          {activity.toolArgs !== undefined && (
            <div>
              <p className="text-2xs font-medium text-muted-foreground mb-1 uppercase tracking-wider">
                {t('chat.transparency.input')}
              </p>
              <pre className="text-xs bg-background rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-40 overflow-y-auto">
                {typeof activity.toolArgs === 'string'
                  ? activity.toolArgs
                  : JSON.stringify(activity.toolArgs, null, 2)}
              </pre>
            </div>
          )}
          {activity.outputSummary !== undefined && (
            <div>
              <p className="text-2xs font-medium text-muted-foreground mb-1 uppercase tracking-wider">
                {t('chat.transparency.output')}
              </p>
              <pre className="text-xs bg-background rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-40 overflow-y-auto">
                {activity.outputSummary}
              </pre>
            </div>
          )}
        </>
      )
    case 'memory': {
      const y = activity.count === 1 ? 'y' : 'ies'
      const q = activity.query
      const src = activity.memorySource
      const base = t('chat.transparency.memoryRecall', { count: activity.count ?? 0, y })
      const recallFor = q
        ? t('chat.transparency.memoryRecallFor', {
            count: activity.count ?? 0,
            y,
            query: truncate(q, 60),
          })
        : null
      const srcSuffix = src ? t('chat.transparency.memorySource', { source: src }) : ''
      return (
        <p className="text-xs text-muted-foreground">
          {recallFor ?? base}
          {srcSuffix}
        </p>
      )
    }
    case 'reasoning':
      return (
        <p className="text-xs italic text-muted-foreground whitespace-pre-wrap">
          {activity.content}
        </p>
      )
    case 'usage':
      return (
        <p className="text-xs text-muted-foreground">
          {t('chat.transparency.inputOutputTokens', {
            in: activity.inputTokens ?? 0,
            out: activity.outputTokens ?? 0,
          })}
        </p>
      )
    case 'phase':
      return null
    default:
      return null
  }
}

function getActivityMeta(
  activity: ChatActivity,
  t: Translator,
): {
  icon: React.ReactNode
  label: string
  badge: React.ReactNode
} {
  switch (activity.type) {
    case 'tool_call':
      return {
        icon: <Wrench className="h-3 w-3" />,
        label: activity.toolName ?? 'tool',
        badge: activity.isError ? (
          <span className="text-2xs px-1.5 py-0.5 rounded bg-error/10 text-error font-medium">
            {t('chat.transparency.error')}
          </span>
        ) : null,
      }
    case 'memory':
      return {
        icon: <Brain className="h-3 w-3" />,
        label: t('chat.transparency.memoryRecall', {
          count: activity.count ?? 0,
          y: activity.count === 1 ? 'y' : 'ies',
        }),
        badge: (
          <span className="text-2xs px-1.5 py-0.5 rounded bg-violet-500/10 text-violet-600 font-medium">
            {activity.memoryAction ?? 'recall'}
          </span>
        ),
      }
    case 'reasoning':
      return {
        icon: <Sparkles className="h-3 w-3" />,
        label: activity.reasoningSource ?? t('chat.transparency.reasoning'),
        badge: null,
      }
    case 'usage':
      return {
        icon: <Cpu className="h-3 w-3" />,
        label: t('chat.transparency.tokenCount', {
          count: (activity.inputTokens ?? 0) + (activity.outputTokens ?? 0),
        }),
        badge: null,
      }
    case 'phase':
      return {
        icon: <Sparkles className="h-3 w-3" />,
        label: t('chat.transparency.phaseLabel', {
          phase: activity.phase ?? 'unknown',
        }),
        badge: (
          <span className="text-2xs px-1.5 py-0.5 rounded bg-info/10 text-info font-medium">
            {activity.status ?? 'started'}
          </span>
        ),
      }
  }
}

function formatDuration(ms?: number): string | null {
  if (!ms) return null
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}

function truncate(s: string, n: number): string {
  return s.length > n ? `${s.slice(0, n)}…` : s
}
