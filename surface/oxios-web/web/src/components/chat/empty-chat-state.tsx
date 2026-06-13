import { MessageSquare, Zap } from 'lucide-react'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import { useChatStore } from '@/stores/chat'
import type { Session } from '@/types'

/**
 * Empty state shown when the chat has no messages yet.
 * Shows mode toggle and recent sessions.
 */
export function EmptyChatState() {
  const { t } = useTranslation()
  const loadSession = useChatStore((s) => s.loadSession)
  const specMode = useChatStore((s) => s.specMode)
  const toggleSpecMode = useChatStore((s) => s.toggleSpecMode)

  const { data: sessionsData } = useQuery({
    queryKey: ['sessions-recent'],
    queryFn: () => api.get<{ items: Session[]; total: number }>('/api/sessions'),
    refetchInterval: 30_000,
  })

  const sessions: Session[] = Array.isArray(sessionsData?.items)
    ? sessionsData.items.slice(0, 8)
    : []

  return (
    <div className="flex flex-col items-center gap-8 py-12 px-4">
      <div className="text-center space-y-3">
        <p className="text-lg font-semibold text-foreground">
          {t('chat.greeting', 'What can I help you with?')}
        </p>
        {/* Mode toggle — only visible before first message */}
        <button
          type="button"
          onClick={toggleSpecMode}
          className={cn(
            'mx-auto inline-flex items-center gap-1.5 text-2xs font-medium px-3 py-1.5 rounded-full border transition-all',
            specMode
              ? 'text-primary border-primary/30 bg-primary/5'
              : 'text-muted-foreground border-border bg-muted/40 hover:bg-muted',
          )}
        >
          {specMode ? <Zap className="h-3 w-3" /> : <MessageSquare className="h-3 w-3" />}
          {specMode ? 'Ouroboros' : 'Chat'}
        </button>
      </div>

      {sessions.length > 0 ? (
        <div className="w-full max-w-md space-y-1">
          <p className="text-xs font-medium text-muted-foreground mb-2">
            {t('chat.recentSessions', 'Recent conversations')}
          </p>
          <div className="space-y-1">
            {sessions.map((s) => {
              const isSpec = s.metadata?.mode === 'spec' || s.metadata?.mode === 'ouroboros'
              const timeStr = formatRelativeTime(s.created_at)

              return (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => loadSession(s.id)}
                  className="flex items-center gap-3 w-full rounded-lg border bg-card px-3 py-2.5 text-left text-sm transition-all hover:bg-accent hover:border-primary/20 hover:shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                >
                  <div
                    className={cn(
                      'flex h-8 w-8 shrink-0 items-center justify-center rounded-full',
                      isSpec
                        ? 'bg-primary/10 text-primary'
                        : 'bg-muted text-muted-foreground',
                    )}
                  >
                    {isSpec ? <Zap className="h-4 w-4" /> : <MessageSquare className="h-4 w-4" />}
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-foreground">
                      {s.title ?? `${s.id.slice(0, 8)}…`}
                    </p>
                    <p className="text-2xs text-muted-foreground">
                      {isSpec ? 'Ouroboros · ' : ''}
                      {timeStr}
                    </p>
                  </div>
                </button>
              )
            })}
          </div>
        </div>
      ) : null}
    </div>
  )
}

function formatRelativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime()
  const minutes = Math.floor(diff / 60_000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 7) return `${days}d ago`
  return new Date(dateStr).toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
  })
}
