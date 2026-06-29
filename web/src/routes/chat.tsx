import { createFileRoute } from '@tanstack/react-router'
import { RefreshCw } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ChatInput, type ContextAttachment } from '@/components/chat/chat-input'
import { EmptyChatState } from '@/components/chat/empty-chat-state'
import { InterviewWizard } from '@/components/chat/interview-wizard'
import { LiveActivityBar } from '@/components/chat/live-activity-bar'
import { MessageBubble } from '@/components/chat/message-bubble'
import { ToolApprovalCard } from '@/components/chat/tool-approval-card'
import { MountDetectionBadge } from '@/components/mount/mount-detection-badge'
import { AiDetectionBadge } from '@/components/project/ai-detection-badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useRoles } from '@/hooks/use-engine'
import { useChatStore } from '@/stores/chat'
import { useMounts } from '@/hooks/use-mounts'

export const Route = createFileRoute('/chat')({ component: ChatPage })

// ---------------------------------------------------------------------------
// Chat UI — Claude-inspired centered layout
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
    activeRole,
    activeModelId,
    activeMountIds,
    setActiveMountIds,
    sendMessage,
    setActiveProject,
    setActiveRole,
    setActiveModelId,
    dismissDetection,
    submitInterviewResponse,
    activeToolApproval,
    resolveToolApproval,
    disconnect,
    connect,
    newSession,
  } = useChatStore()
  const { data: rolesData } = useRoles()
  const roles = Object.entries(rolesData?.roles ?? {}).map(([name, model]) => ({ name, model }))
  const { data: mountsData } = useMounts()
  const activeMountIdsArr = activeMountIds ? activeMountIds.split(',').filter(Boolean) : []
  const activeMounts = activeMountIdsArr
    .map((id) => {
      const m = mountsData?.items?.find((x) => x.id === id)
      return m ? { id: m.id, label: m.name } : null
    })
    .filter((x): x is { id: string; label: string } => x !== null)

  const handleAttachMount = (id: string) => {
    const cur = activeMountIds ? activeMountIds.split(',').filter(Boolean) : []
    if (cur.includes(id)) return
    setActiveMountIds([...cur, id])
  }
  const handleRemoveMount = (id: string) => {
    const cur = activeMountIds ? activeMountIds.split(',').filter(Boolean) : []
    setActiveMountIds(cur.filter((x) => x !== id))
  }

  const [input, setInput] = useState('')
  const [userScrolledUp, setUserScrolledUp] = useState(false)
  const bottomRef = useRef<HTMLDivElement>(null)
  const scrollAreaRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to bottom on new messages, but only if user hasn't scrolled up
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, isStreaming, userScrolledUp])

  // Auto-connect WebSocket on mount
  useEffect(() => {
    connect()
  }, [connect])

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey
      if (mod && e.shiftKey && e.key.toLowerCase() === 'n') {
        e.preventDefault()
        newSession()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [newSession])

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80
    setUserScrolledUp(!atBottom)
  }

  const handleSend = (content: string, contextItems: ContextAttachment[]) => {
    if (!content.trim() || isStreaming) return

    // Build message with context references
    let enrichedContent = content

    // If there are context attachments, append them as structured references
    if (contextItems.length > 0) {
      const contextRefs = contextItems
        .map((ctx) => {
          if (ctx.type === 'knowledge') {
            return `[context:knowledge:${ctx.id}]`
          }
          return `[context:memory:${ctx.id}]`
        })
        .join(' ')
      enrichedContent = `${content}\n${contextRefs}`
    }

    sendMessage(enrichedContent)
    setInput('')
    setUserScrolledUp(false)
  }

  const handleCancel = () => {
    disconnect()
    setTimeout(() => connect(), 100)
  }

  // RFC-032: retry the message that produced an error card. Pop the error
  // bubble AND the user message that preceded it (the store will append a
  // fresh user message when we resend, so leaving the original in place
  // would duplicate it on screen). After removal, scroll the user back to
  // the bottom and re-fire the same send pipeline as their original tap.
  const handleRetry = (errorMessageId: string) => {
    const errIdx = messages.findIndex((m) => m.id === errorMessageId)
    if (errIdx < 0) return
    const precedingUser = [...messages.slice(0, errIdx)].reverse().find((m) => m.role === 'user')
    if (!precedingUser) return
    const { removeMessage } = useChatStore.getState()
    removeMessage?.(errorMessageId)
    removeMessage?.(precedingUser.id)
    handleSend(precedingUser.content, [])
    setUserScrolledUp(false)
  }

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col min-w-0">
        {/* Reconnect warning banner */}
        {!connected && (
          <div className="flex items-center gap-2 px-4 py-2 bg-warning/10 text-warning text-xs border-b">
            <span className="h-2 w-2 rounded-full bg-warning animate-pulse shrink-0" />
            <span className="flex-1">{t('chat.reconnecting', 'Reconnecting...')}</span>
            <Button
              size="sm"
              variant="ghost"
              className="h-6 px-2 text-warning hover:text-warning"
              onClick={() => {
                disconnect()
                connect()
              }}
            >
              <RefreshCw className="h-3 w-3 mr-1" />
              {t('chat.retry', 'Retry')}
            </Button>
          </div>
        )}

        {/* AI Detection Badge */}
        {detectedProject && !activeProjectId && (
          <AiDetectionBadge
            project={detectedProject}
            onApply={() => setActiveProject(detectedProject.id)}
            onDismiss={() => dismissDetection(detectedProject.id)}
          />
        )}

        {/* RFC-025: Mount Detection Badge */}
        <MountDetectionBadge />

        {/* ── Messages area ── */}
        <ScrollArea
          ref={scrollAreaRef as any}
          className="flex-1 min-h-0"
          onScroll={handleScroll}
          role="log"
          aria-label={t('common.chatMessages')}
        >
          <div className="max-w-3xl mx-auto px-4 py-6">
            {messages.length === 0 && <EmptyChatState />}
            <div className="space-y-5">
              {messages.map((msg, _idx) => {
                // Compute assistant-only index for knowledge save tracking
                const assistantIndex =
                  msg.role === 'assistant'
                    ? messages.slice(0, _idx).filter((m) => m.role === 'assistant').length
                    : undefined
                return (
                  <MessageBubble
                    key={msg.id}
                    message={msg}
                    sessionId={activeSessionId ?? undefined}
                    assistantIndex={assistantIndex}
                    onRetry={msg.metadata?.isError ? () => handleRetry(msg.id) : undefined}
                  />
                )
              })}

              {/* Interview wizard */}
              {activeInterview && activeInterview.length > 0 && (
                <InterviewWizard
                  questions={activeInterview}
                  round={interviewRound}
                  ambiguity={interviewAmbiguity}
                  onSubmit={submitInterviewResponse}
                  disabled={isStreaming}
                />
              )}

              {/* Tool approval */}
              {activeToolApproval && (
                <ToolApprovalCard
                  toolName={activeToolApproval.toolName}
                  reason={activeToolApproval.reason}
                  onApprove={() => resolveToolApproval(activeToolApproval.id, true)}
                  onDeny={() => resolveToolApproval(activeToolApproval.id, false)}
                  disabled={isStreaming}
                />
              )}

              {/* Live activity header (replaces legacy 3-dot typing indicator) */}
              {isStreaming && !activeInterview && !activeToolApproval && <LiveActivityBar />}

              <div ref={bottomRef} />
            </div>
          </div>
        </ScrollArea>

        {/* ── Input (fixed at bottom) ── */}
        {!activeInterview && (
          <div className="border-t bg-background/95 backdrop-blur-sm shrink-0">
            <ChatInput
              value={input}
              onChange={setInput}
              onSend={handleSend}
              roles={roles}
              activeRole={activeRole}
              setActiveRole={setActiveRole}
              activeModelId={activeModelId}
              setActiveModelId={setActiveModelId}
              onCancel={handleCancel}
              disabled={isStreaming}
              isStreaming={isStreaming}
              connected={connected}
              activeMounts={activeMounts}
              onAttachMount={handleAttachMount}
              onRemoveMount={handleRemoveMount}
            />
          </div>
        )}
      </div>
    </div>
  )
}
