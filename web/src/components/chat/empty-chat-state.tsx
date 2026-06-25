import { useQuery } from '@tanstack/react-query'
import { MessageSquare } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { api } from '@/lib/api-client'
import { useChatStore } from '@/stores/chat'
import type { Session } from '@/types'

/**
 * Empty state shown when the chat has no messages yet.
 * Shows recent sessions.
 */
export function EmptyChatState() {
  const { t } = useTranslation()
  const loadSession = useChatStore((s) => s.loadSession)

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
      </div>

      {sessions.length > 0 ? (
        <div className="w-full max-w-md space-y-1">
          <p className="text-xs font-medium text-muted-foreground mb-2">
            {t('chat.recentSessions', 'Recent conversations')}
          </p>
          <div className="space-y-1">
            {sessions.map((s) => {
              const timeStr = formatRelativeTime(s.created_at)

              return (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => loadSession(s.id)}
                  className="flex items-center gap-3 w-full rounded-lg border bg-card px-3 py-2.5 text-left text-sm transition-all hover:bg-accent hover:border-primary/20 hover:shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                >
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground">
                    <MessageSquare className="h-4 w-4" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-foreground">{s.title ?? `${s.id.slice(0, 8)}…`}</p>
                    <p className="text-2xs text-muted-foreground">{timeStr}</p>
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
