import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link } from '@tanstack/react-router'
import { Bot, Loader2, RefreshCw, Send, User } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Textarea } from '@/components/ui/textarea'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { useChatStore } from '@/stores/chat'
import type { Space, Session } from '@/types'

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
    activeSpaceId,
    sendMessage,
    loadSession,
    newSession,
    setActiveSpace,
  } = useChatStore()

  const [input, setInput] = useState('')
  const [showHistory, setShowHistory] = useState(false)
  const bottomRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, isStreaming])

  const handleSend = () => {
    if (!input.trim() || isStreaming) return
    sendMessage(input.trim())
    setInput('')
  }

  return (
    <div className="flex h-[calc(100vh-8rem)]">
      {/* ── Left: Space + Session sidebar ─────────────────────────── */}
      <SpaceSessionSidebar
        activeSpaceId={activeSpaceId}
        activeSessionId={activeSessionId}
        onSelectSpace={setActiveSpace}
        onSelectSession={loadSession}
        onNewSession={newSession}
        onToggleHistory={() => setShowHistory((v) => !v)}
        showHistory={showHistory}
      />

      {/* ── Right: Chat area ──────────────────────────────────────── */}
      <div className="flex flex-1 flex-col min-w-0">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b">
          <div>
            <h2 className="text-sm font-semibold">
              {activeSessionId ? t('chat.activeConversation') : t('chat.newConversation')}
            </h2>
            {!connected && (
              <span className="text-xs text-muted-foreground">{t('chat.connecting')}</span>
            )}
          </div>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => { if (activeSessionId) loadSession(activeSessionId) }}
            >
              <RefreshCw className="h-3 w-3 mr-1" /> {t('chat.refreshing')}
            </Button>
            <Button variant="outline" size="sm" onClick={newSession}>
              {t('chat.newConversationButton')}
            </Button>
          </div>
        </div>

        {/* Messages */}
        <Card className="flex-1 flex flex-col min-h-0 mx-4 my-3 border-t-0">
          <ScrollArea
            className="flex-1 p-4"
            role="log"
            aria-label={t('common.chatMessages')}
          >
            {messages.length === 0 ? (
              <div className="flex items-center justify-center h-full text-muted-foreground">
                <p>
                  {!connected
                    ? t('chat.serverConnecting')
                    : t('chat.sendHint')}
                </p>
              </div>
            ) : (
              <div className="space-y-4">
                {messages.map((msg, i) => (
                  <div
                    // biome-ignore lint/suspicious/noArrayIndexKey: messages lack unique IDs
                    key={`${msg.role}-${i}`}
                    className={`flex gap-3 ${msg.role === 'user' ? 'justify-end' : ''}`}
                  >
                    {msg.role === 'assistant' && (
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
                        <Bot className="h-4 w-4" />
                      </div>
                    )}
                    <div
                      className={`rounded-lg px-4 py-2 max-w-[80%] ${msg.role === 'user' ? 'bg-primary text-primary-foreground' : 'bg-muted'}`}
                    >
                      {msg.role === 'user' ? (
                        <p className="text-sm whitespace-pre-wrap">{msg.content}</p>
                      ) : (
                        <div className="text-sm prose prose-sm dark:prose-invert max-w-none [&>p:first-child]:mt-0 [&>p:last-child]:mb-0">
                          <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown>
                        </div>
                      )}
                    </div>
                    {msg.role === 'user' && (
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted">
                        <User className="h-4 w-4" />
                      </div>
                    )}
                  </div>
                ))}
                {isStreaming && (
                  <div className="flex gap-3">
                    <div className="flex h-8 w-8 items-center justify-center rounded-full bg-primary text-primary-foreground">
                      <Bot className="h-4 w-4" />
                    </div>
                    <div className="flex items-center gap-2 rounded-lg bg-muted px-4 py-2">
                      <Loader2 className="h-4 w-4 animate-spin" />
                      <span className="text-sm text-muted-foreground">{t('chat.thinking')}</span>
                    </div>
                  </div>
                )}
                <div ref={bottomRef} />
              </div>
            )}
          </ScrollArea>

          {/* Input */}
          <div className="border-t p-4">
            <div className="flex gap-2">
              <Textarea
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault()
                    handleSend()
                  }
                }}
                placeholder={
                  connected ? t('chat.inputPlaceholder') : t('chat.waitingForConnection')
                }
                disabled={!connected || isStreaming}
                className="min-h-[44px] max-h-[120px] resize-none"
                rows={1}
              />
              <Button
                onClick={handleSend}
                disabled={!input.trim() || isStreaming || !connected}
                size="icon"
                aria-label={t('common.sendMessage')}
              >
                <Send className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </Card>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Space + Session sidebar
// ---------------------------------------------------------------------------

function SpaceSessionSidebar({
  activeSpaceId,
  activeSessionId,
  onSelectSpace,
  onSelectSession,
  onNewSession,
  onToggleHistory,
  showHistory,
}: {
  activeSpaceId: string | null
  activeSessionId: string | null
  onSelectSpace: (id: string | null) => void
  onSelectSession: (id: string) => void
  onNewSession: () => void
  onToggleHistory: () => void
  showHistory: boolean
}) {
  const { t } = useTranslation()
  const { data: spacesData } = useQuery({
    queryKey: ['spaces'],
    queryFn: () =>
      fetch('/api/spaces').then((r) => r.json()) as Promise<{
        items: Space[]
        total: number
      }>,
    refetchInterval: 30000,
  })

  const { data: sessionsData, refetch: refetchSessions } = useQuery({
    queryKey: ['sessions', activeSpaceId],
    queryFn: () =>
      fetch('/api/sessions').then((r) => r.json()) as Promise<{
        items: Session[]
        total: number
      }>,
    refetchInterval: 10000,
  })

  const spaces: Space[] = spacesData?.items ?? []
  const sessions: Session[] = sessionsData?.items ?? []

  // Group sessions by date for display
  const grouped = groupSessionsByDate(sessions)

  return (
    <div className="w-56 shrink-0 border-r flex flex-col overflow-hidden">
      {/* Spaces */}
      <div className="p-2 border-b">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
            {t('chat.spacesLabel')}
          </span>
        </div>
        <div className="space-y-0.5">
          {spaces.map((space) => (
            <button
              key={space.id}
              onClick={() => onSelectSpace(space.id)}
              className={`w-full flex items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left transition-colors ${
                activeSpaceId === space.id
                  ? 'bg-accent text-accent-foreground font-medium'
                  : 'hover:bg-accent/50 text-muted-foreground'
              }`}
            >
              <span
                className={`h-2 w-2 rounded-full shrink-0 ${
                  space.active !== false ? 'bg-emerald-500' : 'bg-muted'
                }`}
              />
              <span className="truncate">{space.name}</span>
            </button>
          ))}
          {spaces.length === 0 && (
            <p className="text-xs text-muted-foreground px-2 py-1">
              {t('chat.loadingSpacesShort')}
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
                        : s.id.slice(0, 8) + '...'}
                    </span>
                    <span className="block text-[10px] text-muted-foreground/60">
                      {new Date(s.created_at).toLocaleString('ko-KR', {
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
              onClick={onToggleHistory}
              className="text-xs text-muted-foreground hover:text-foreground mb-1 px-2"
            >
              {t('chat.showLess')}
            </button>
            {sessions.map((s) => (
              <button
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
                <div className="text-[10px] text-muted-foreground/70">
                  {new Date(s.created_at).toLocaleString('ko-KR', {
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
          to="/spaces"
          className="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs text-muted-foreground hover:bg-accent/50 hover:text-foreground"
        >
          {t('chat.manageSpaces')}
        </Link>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function groupSessionsByDate(
  sessions: Session[],
): Record<string, Session[]> {
  const now = new Date()
  const groups: Record<string, Session[]> = {}

  for (const s of sessions) {
    const d = new Date(s.created_at)
    let label: string
    const diffDays = Math.floor(
      (now.getTime() - d.getTime()) / (1000 * 60 * 60 * 24),
    )
    if (diffDays === 0) label = 'today'
    else if (diffDays === 1) label = 'yesterday'
    else if (diffDays < 7) label = 'thisWeek'
    else label = 'previous'

    if (!groups[label]) groups[label] = []
    groups[label]!.push(s)
  }

  return groups
}