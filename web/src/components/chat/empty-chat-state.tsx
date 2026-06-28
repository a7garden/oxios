import { useQuery } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { BookOpen, Brain, FolderKanban, MessageSquare, Wrench } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { api } from '@/lib/api-client'
import { useChatStore } from '@/stores/chat'
import type { Session } from '@/types'

/**
 * Empty state shown when the chat has no messages yet.
 *
 * Surfaces Oxios's 4 capabilities (knowledge / memory / tools / projects)
 * as entry points so new users know what the system can do, then lists
 * recent sessions for quick continuation.
 */
export function EmptyChatState() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const loadSession = useChatStore((s) => s.loadSession)

  const { data: sessionsData } = useQuery({
    queryKey: ['sessions-recent'],
    queryFn: () => api.get<{ items: Session[]; total: number }>('/api/sessions'),
    refetchInterval: 30_000,
  })

  const sessions: Session[] = Array.isArray(sessionsData?.items)
    ? sessionsData.items.slice(0, 8)
    : []

  const capabilities = [
    {
      key: 'knowledge',
      icon: BookOpen,
      title: t('chat.cap.knowledge', '지식'),
      desc: t('chat.cap.knowledgeDesc', '문서 검색'),
      href: '/knowledge',
      tone: 'text-info',
    },
    {
      key: 'memory',
      icon: Brain,
      title: t('chat.cap.memory', '메모리'),
      desc: t('chat.cap.memoryDesc', '시맨틱 검색'),
      href: '/memory',
      tone: 'text-primary',
    },
    {
      key: 'tools',
      icon: Wrench,
      title: t('chat.cap.tools', '도구'),
      desc: t('chat.cap.toolsDesc', 'shell·HTTP·MCP'),
      href: '/mcp',
      tone: 'text-warning',
    },
    {
      key: 'projects',
      icon: FolderKanban,
      title: t('chat.cap.projects', '프로젝트'),
      desc: t('chat.cap.projectsDesc', '연결된 컨텍스트'),
      href: '/projects',
      tone: 'text-success',
    },
  ] as const

  return (
    <div className="flex flex-col items-center gap-8 py-12 px-4">
      <div className="text-center space-y-3">
        <p className="text-lg font-semibold text-foreground">
          {t('chat.greeting', 'What can I help you with?')}
        </p>
        <p className="text-xs text-muted-foreground">
          {t('chat.subgreeting', '시작할 위치를 선택하거나 자유롭게 질문하세요')}
        </p>
      </div>

      {/* Capability grid — 4 cells, one per Oxios surface */}
      <div className="grid w-full max-w-2xl grid-cols-2 gap-2 sm:grid-cols-4">
        {capabilities.map((cap) => {
          const Icon = cap.icon
          return (
            <button
              key={cap.key}
              type="button"
              onClick={() => navigate({ to: cap.href })}
              className="group flex flex-col items-start gap-1.5 rounded-lg border bg-card p-3 text-left transition-all hover:border-primary/30 hover:shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            >
              <span
                className={`flex h-7 w-7 items-center justify-center rounded-md bg-muted transition-colors group-hover:bg-accent`}
              >
                <Icon className={`h-4 w-4 ${cap.tone}`} />
              </span>
              <span className="text-sm font-medium leading-tight">{cap.title}</span>
              <span className="text-2xs text-muted-foreground leading-tight">{cap.desc}</span>
            </button>
          )
        })}
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
