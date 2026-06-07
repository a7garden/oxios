import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { Menu, RefreshCw } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ChatInput } from '@/components/chat/chat-input'
import { EmptyChatState } from '@/components/chat/empty-chat-state'
import { InterviewResponse } from '@/components/chat/interview-response'
import { MessageBubble } from '@/components/chat/message-bubble'
import { AiDetectionBadge } from '@/components/project/ai-detection-badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { api } from '@/lib/api-client'
import { useChatStore } from '@/stores/chat'
import type { Project, Session } from '@/types'

export const Route = createFileRoute('/chat')({ component: ChatPage })

// ---------------------------------------------------------------------------
// Chat UI
// ---------------------------------------------------------------------------

function ChatPage() {
  const { t } = useTranslation()
  const {
    messages,
    isStreaming,
    connected,
    activeSessionId,
    activeProjectId,
    detectedProject,
    activeInterview,
    interviewRound,
    interviewAmbiguity,
    sendMessage,
    loadSession,
    newSession,
    setActiveProject,
    dismissDetection,
    submitInterviewResponse,
    disconnect,
    connect,
  } = useChatStore()

  const [input, setInput] = useState('')
  const [showHistory, setShowHistory] = useState(false)
  const [userScrolledUp, setUserScrolledUp] = useState(false)
  const [sidebarOpen, setSidebarOpen] = useState(true)
  const bottomRef = useRef<HTMLDivElement>(null)
  const scrollAreaRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to bottom on new messages, but only if user hasn't scrolled up
  useEffect(() => {
    if (userScrolledUp) return
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, isStreaming, userScrolledUp])

  // Auto-connect WebSocket on mount (moved from store rehydrate to avoid
  // connecting on every page load — only when chat route is active)
  useEffect(() => {
    connect()
  }, [connect])

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80
    setUserScrolledUp(!atBottom)
  }

  const handleSend = () => {
    if (!input.trim() || isStreaming) return
    sendMessage(input.trim())
    setInput('')
    setUserScrolledUp(false)
  }

  const handleCancel = () => {
    disconnect()
    // Small delay before reconnecting to ensure clean state
    setTimeout(() => connect(), 100)
  }

  return (
    <div className="flex h-[calc(100vh-8rem)]">
      {/* ── Left: Project + Session sidebar (collapsible) ──────── */}
      {sidebarOpen && (
        <ProjectSessionSidebar
          activeProjectId={activeProjectId}
          activeSessionId={activeSessionId}
          onSelectProject={setActiveProject}
          onSelectSession={loadSession}
          onNewSession={newSession}
          onToggleHistory={() => setShowHistory((v) => !v)}
          showHistory={showHistory}
        />
      )}

      {/* ── Right: Chat area ──────────────────────────────────────── */}
      <div className="flex flex-1 flex-col min-w-0">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b">
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => setSidebarOpen((v) => !v)}
              aria-label={t('chat.toggleSidebar')}
            >
              <Menu className="h-4 w-4" />
            </Button>
            <h2 className="text-sm font-semibold">
              {activeSessionId ? t('chat.activeConversation') : t('chat.newConversation')}
            </h2>
          </div>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                if (activeSessionId) loadSession(activeSessionId)
              }}
            >
              <RefreshCw className="h-3 w-3 mr-1" /> {t('chat.refreshing')}
            </Button>
            <Button variant="outline" size="sm" onClick={newSession}>
              {t('chat.newConversationButton')}
            </Button>
          </div>
        </div>

        {/* Reconnect warning banner — only shown when disconnected */}
        {!connected && (
          <div className="flex items-center gap-2 px-4 py-2 bg-warning/10 text-warning text-xs border-b">
            <span className="h-2 w-2 rounded-full bg-warning animate-pulse shrink-0" />
            <span>{t('chat.reconnecting', 'Reconnecting...')}</span>
          </div>
        )}

        {/* AI Detection Badge (Phase 2 stub — always hidden) */}
        {detectedProject && !activeProjectId && (
          <AiDetectionBadge
            project={detectedProject}
            onApply={() => setActiveProject(detectedProject.id)}
            onDismiss={() => dismissDetection(detectedProject.id)}
          />
        )}

        {/* Messages */}
        <Card className="flex-1 flex flex-col min-h-0 mx-4 my-3 border-t-0">
          <ScrollArea
            ref={scrollAreaRef as any}
            className="flex-1 p-4"
            onScroll={handleScroll}
            role="log"
            aria-label={t('common.chatMessages')}
          >
            {messages.length === 0 ? (
              <EmptyChatState onSuggestionClick={(text) => setInput(text)} />
            ) : (
              <div className="space-y-4">
                {messages.map((msg) => (
                  <MessageBubble key={msg.id} message={msg} />
                ))}

                {/* Interactive interview UI */}
                {activeInterview && activeInterview.length > 0 && (
                  <InterviewResponse
                    questions={activeInterview}
                    round={interviewRound}
                    ambiguity={interviewAmbiguity}
                    onSubmit={submitInterviewResponse}
                    disabled={isStreaming}
                  />
                )}

                <div ref={bottomRef} />
              </div>
            )}
          </ScrollArea>

          {/* Input — hidden during active interview */}
          {!activeInterview && (
            <ChatInput
              value={input}
              onChange={setInput}
              onSend={handleSend}
              onCancel={handleCancel}
              disabled={isStreaming}
              isStreaming={isStreaming}
              connected={connected}
            />
          )}
        </Card>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Project + Session sidebar
// ---------------------------------------------------------------------------

function ProjectSessionSidebar({
  activeProjectId,
  activeSessionId,
  onSelectProject,
  onSelectSession,
  onNewSession,
  onToggleHistory,
  showHistory,
}: {
  activeProjectId: string | null
  activeSessionId: string | null
  onSelectProject: (id: string | null) => void
  onSelectSession: (id: string) => void
  onNewSession: () => void
  onToggleHistory: () => void
  showHistory: boolean
}) {
  const { t } = useTranslation()
  const { data: projectsData } = useQuery({
    queryKey: ['projects'],
    queryFn: () => api.get<{ items: Project[]; total: number }>('/api/projects'),
    refetchInterval: 30000,
  })

  const { data: sessionsData, refetch: refetchSessions } = useQuery({
    queryKey: ['sessions', activeProjectId],
    queryFn: () => api.get<{ items: Session[]; total: number }>('/api/sessions'),
    refetchInterval: 10000,
  })

  const projects: Project[] = projectsData?.items ?? []
  const sessions: Session[] = sessionsData?.items ?? []

  // Group sessions by date for display
  const grouped = groupSessionsByDate(sessions)

  return (
    <div className="w-56 shrink-0 border-r flex flex-col overflow-hidden">
      {/* Projects */}
      <div className="p-2 border-b">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
            {t('chat.projectsLabel', 'Projects')}
          </span>
        </div>
        <div className="space-y-0.5">
          {projects.map((project) => (
            <button
              type="button"
              key={project.id}
              onClick={() => onSelectProject(project.id)}
              className={`w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left transition-colors ${
                activeProjectId === project.id
                  ? 'bg-accent text-accent-foreground font-medium'
                  : 'hover:bg-accent/50 text-muted-foreground'
              }`}
            >
              <span className="h-2 w-2 rounded-full shrink-0 bg-success" />
              <span className="truncate">{project.name}</span>
            </button>
          ))}
          {projects.length === 0 && (
            <p className="text-xs text-muted-foreground px-2 py-1">
              {t('chat.loadingProjectsShort', 'No projects')}
            </p>
          )}
        </div>
      </div>

      {/* Sessions */}
      <div className="flex-1 overflow-y-auto">
        <div className="p-2 border-b flex items-center justify-between">
          <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
            {t('chat.sessionsLabel')}
          </span>
          <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => refetchSessions()}>
            <RefreshCw className="h-3 w-3" />
          </Button>
        </div>

        {!showHistory ? (
          <div className="p-2">
            <Button
              variant={activeSessionId ? 'outline' : 'default'}
              size="sm"
              className="w-full mb-2"
              onClick={onNewSession}
            >
              {t('chat.newConversationButton')}
            </Button>
            {Object.entries(grouped).map(([label, group]) => (
              <div key={label} className="mb-2">
                <p className="text-xs text-muted-foreground px-2 mb-1">{t(`chat.${label}`)}</p>
                {group.map((s) => (
                  <button
                    type="button"
                    key={s.id}
                    onClick={() => onSelectSession(s.id)}
                    className={`w-full text-left px-2 py-1.5 rounded text-xs transition-colors ${
                      activeSessionId === s.id
                        ? 'bg-accent text-accent-foreground font-medium'
                        : 'hover:bg-accent/50 text-muted-foreground'
                    }`}
                  >
                    <span className="block truncate">
                      {s.message_count != null && s.message_count > 0
                        ? t('chat.messageCount', { count: s.message_count })
                        : `${s.id.slice(0, 8)}...`}
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
            {sessions.length > 0 && (
              <button
                type="button"
                onClick={onToggleHistory}
                className="w-full text-xs text-muted-foreground hover:text-foreground mt-2 px-2"
              >
                {t('chat.viewAllSessions', { count: sessions.length })}
              </button>
            )}
          </div>
        ) : (
          <div className="p-2 space-y-0.5">
            <button
              type="button"
              onClick={onToggleHistory}
              className="text-xs text-muted-foreground hover:text-foreground mb-1 px-2"
            >
              {t('chat.showLess')}
            </button>
            {sessions.map((s) => (
              <button
                type="button"
                key={s.id}
                onClick={() => {
                  onSelectSession(s.id)
                  onToggleHistory()
                }}
                className={`w-full text-left px-2 py-1.5 rounded text-xs transition-colors ${
                  activeSessionId === s.id
                    ? 'bg-accent text-accent-foreground font-medium'
                    : 'hover:bg-accent/50 text-muted-foreground'
                }`}
              >
                <div className="truncate font-mono">{s.id.slice(0, 12)}...</div>
                <div className="text-2xs text-muted-foreground/70">
                  {new Date(s.created_at).toLocaleString(undefined, {
                    month: 'short',
                    day: 'numeric',
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </div>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Footer links */}
      <div className="p-2 border-t space-y-0.5">
        <Link
          to="/sessions"
          className="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        >
          {t('chat.manageSessions')}
        </Link>
        <Link
          to="/projects"
          className="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        >
          {t('chat.manageProjects', 'Manage Projects')}
        </Link>
      </div>
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
