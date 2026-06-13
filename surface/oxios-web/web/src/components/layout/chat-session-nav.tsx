import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { Plus, RefreshCw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import { useChatStore } from '@/stores/chat'
import { useSidebarStore } from '@/stores/sidebar'
import type { Project, Session } from '@/types'
import {
  itemActive,
  itemBase,
  itemCollapsedBase,
  itemDense,
  itemInactive,
  sectionGap,
  sectionHeader,
  sectionSeparator,
} from './sidebar'

// ---------------------------------------------------------------------------
// ChatSessionNav — renders inside the main sidebar when chat mode is active.
// Replaces the old inner ProjectSessionSidebar.
// ---------------------------------------------------------------------------

export function ChatSessionNav() {
  const { collapsed } = useSidebarStore()

  if (collapsed) {
    return <CollapsedChatNav />
  }

  return <ExpandedChatNav />
}

// ---------------------------------------------------------------------------
// Expanded
// ---------------------------------------------------------------------------

function ExpandedChatNav() {
  const { t } = useTranslation()
  const activeProjectId = useChatStore((s) => s.activeProjectId)
  const activeSessionId = useChatStore((s) => s.activeSessionId)
  const setActiveProject = useChatStore((s) => s.setActiveProject)
  const loadSession = useChatStore((s) => s.loadSession)
  const newSession = useChatStore((s) => s.newSession)

  const { data: projectsData } = useQuery({
    queryKey: ['projects'],
    queryFn: () => api.get<{ items: Project[]; total: number }>('/api/projects'),
    refetchInterval: 30_000,
  })

  const { data: sessionsData, refetch: refetchSessions } = useQuery({
    queryKey: ['sessions', activeProjectId],
    queryFn: () => api.get<{ items: Session[]; total: number }>('/api/sessions'),
    refetchInterval: 10_000,
  })

  const projects: Project[] = Array.isArray(projectsData?.items) ? projectsData.items : []
  const sessions: Session[] = Array.isArray(sessionsData?.items) ? sessionsData.items : []
  const grouped = groupSessionsByDate(sessions)

  return (
    <>
      {/* New session */}
      <div className={sectionGap}>
        <Button
          variant={activeSessionId ? 'outline' : 'default'}
          size="sm"
          className="w-full"
          onClick={newSession}
        >
          <Plus className="h-3 w-3 mr-1" />
          {t('chat.newConversationButton')}
        </Button>
      </div>

      {/* Projects */}
      {projects.length > 0 && (
        <div className={sectionGap}>
          <p className={sectionHeader}>{t('chat.projectsLabel', 'Projects')}</p>
          <div className="space-y-0.5">
            {projects.map((p) => (
              <button
                type="button"
                key={p.id}
                onClick={() => setActiveProject(p.id)}
                className={cn(itemBase, activeProjectId === p.id ? itemActive : itemInactive)}
              >
                <span className="h-2 w-2 rounded-full shrink-0 bg-success" />
                <span className="truncate">{p.name}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      <div className={sectionSeparator} />

      {/* Sessions */}
      <div className="flex-1 overflow-y-auto">
        <div className="flex items-center justify-between px-2 mb-1">
          <span className={sectionHeader.replace('mb-1 ', '')}>
            {t('chat.sessionsLabel', 'Sessions')}
          </span>
          <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => refetchSessions()}>
            <RefreshCw className="h-3 w-3" />
          </Button>
        </div>

        {Object.entries(grouped).map(([label, group]) => (
          <div key={label} className={sectionGap}>
            <p className="px-2.5 text-xs text-muted-foreground mb-0.5">{t(`chat.${label}`)}</p>
            {group.map((s) => (
              <button
                type="button"
                key={s.id}
                onClick={() => loadSession(s.id)}
                className={cn(itemDense, activeSessionId === s.id ? itemActive : itemInactive)}
              >
                <span className="block truncate">
                  {s.title ?? `${s.id.slice(0, 8)}...`}
                </span>
                <span className="block text-2xs text-muted-foreground/60">
                  {new Date(s.created_at).toLocaleString(undefined, {
                    month: 'short',
                    day: 'numeric',
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </span>
              </button>
            ))}
          </div>
        ))}

        {sessions.length === 0 && (
          <p className="text-xs text-sidebar-foreground/50 px-4 py-2">
            {t('chat.noSessions', 'No sessions yet')}
          </p>
        )}
      </div>

      {/* Footer links */}
      <div className={sectionSeparator.replace('my-2', 'mt-2 mb-0')} />
      <div className="space-y-0.5">
        <Link to="/sessions" className={cn(itemBase, itemInactive)}>
          {t('chat.manageSessions')}
        </Link>
        <Link to="/projects" className={cn(itemBase, itemInactive)}>
          {t('chat.manageProjects', 'Manage Projects')}
        </Link>
      </div>
    </>
  )
}

// ---------------------------------------------------------------------------
// Collapsed
// ---------------------------------------------------------------------------

function CollapsedChatNav() {
  const { t } = useTranslation()
  const newSession = useChatStore((s) => s.newSession)

  return (
    <div className="flex flex-col items-center gap-1 py-1">
      <Tooltip>
        <TooltipTrigger asChild>
          <button type="button" onClick={newSession} className={cn(itemCollapsedBase, itemInactive)}>
            <Plus className="h-4 w-4" />
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">{t('chat.newConversationButton')}</TooltipContent>
      </Tooltip>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function groupSessionsByDate(sessions: Session[]): Record<string, Session[]> {
  const now = new Date()
  const groups: Record<string, Session[]> = {}

  for (const s of sessions) {
    const d = new Date(s.created_at)
    let label: string
    const diffDays = Math.floor((now.getTime() - d.getTime()) / (1000 * 60 * 60 * 24))
    if (diffDays === 0) label = 'today'
    else if (diffDays === 1) label = 'yesterday'
    else if (diffDays < 7) label = 'thisWeek'
    else label = 'previous'

    if (!groups[label]) groups[label] = []
    groups[label]!.push(s)
  }

  return groups
}
