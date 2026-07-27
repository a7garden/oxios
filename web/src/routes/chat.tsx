import { createFileRoute } from '@tanstack/react-router'
import { ArrowDown, RefreshCw } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { AttachedFile, ContextAttachment } from '@/components/chat/chat-input'
import { ChatInput } from '@/components/chat/chat-input'
import { ChatMiniMap } from '@/components/chat/chat-minimap'
import { CompressedGroup } from '@/components/chat/compressed-group'
import { EmptyChatState } from '@/components/chat/empty-chat-state'
import { InterviewWizard } from '@/components/chat/interview-wizard'
import { MessageBubble } from '@/components/chat/message-bubble'
import { TextSelectionBar } from '@/components/chat/text-selection-bar'
import { ToolApprovalCard } from '@/components/chat/tool-approval-card'
import { MountDetectionBadge } from '@/components/mount/mount-detection-badge'
import { PortalPanel } from '@/components/portal/portal-panel'
import { AiDetectionBadge } from '@/components/project/ai-detection-badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useDraftPersistence } from '@/hooks/use-draft-persistence'
import { useRoles } from '@/hooks/use-engine'
import { useMounts } from '@/hooks/use-mounts'
import { addInputHistory } from '@/lib/input-history-storage'
import { useChatStore } from '@/stores/chat'
import { usePortalStore } from '@/stores/portal'

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
  const queuedCount = useChatStore((s) => s._pendingQueue.length)
  const stackOpen = usePortalStore((s) => s.stack.length > 0)
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
  useDraftPersistence(activeSessionId, input, setInput)
  const [userScrolledUp, setUserScrolledUp] = useState(false)
  const bottomRef = useRef<HTMLDivElement>(null)
  const scrollAreaRef = useRef<HTMLDivElement>(null)

  // Compressed groups: collapse older messages when a conversation is long.
  const COLLAPSE_THRESHOLD = 40
  const VISIBLE_TAIL = 20
  const collapseCount = messages.length > COLLAPSE_THRESHOLD ? messages.length - VISIBLE_TAIL : 0

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

  const handleMiniMapJump = (index: number) => {
    scrollAreaRef.current
      ?.querySelector(`[data-msg-index="${index}"]`)
      ?.scrollIntoView({ behavior: 'smooth', block: 'center' })
  }

  const handleSend = (
    content: string,
    contextItems: ContextAttachment[],
    files: AttachedFile[],
  ) => {
    if (!content.trim()) return

    let enrichedContent = content

    // Append context references
    if (contextItems.length > 0) {
      const contextRefs = contextItems
        .map((ctx) => {
          if (ctx.type === 'knowledge') return `[context:knowledge:${ctx.id}]`
          if (ctx.type === 'file') return `[context:file:${ctx.id}]`
          return `[context:memory:${ctx.id}]`
        })
        .join(' ')
      enrichedContent = `${content}\n${contextRefs}`
    }

    // Append file contents
    if (files.length > 0) {
      const fileContents = files
        .map((f) => {
          if (f.content) {
            return `[file:${f.name}]\n${f.content}\n[/file]`
          }
          if (f.dataUrl) {
            return `[image:${f.name}](${f.dataUrl})`
          }
          return `[file:${f.name}]`
        })
        .join('\n')
      enrichedContent = `${enrichedContent}\n${fileContents}`
    }

    addInputHistory(content)
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
    handleSend(precedingUser.content, [], [])
    setUserScrolledUp(false)
  }

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col min-w-0">
        {/* Reconnect warning banner */}
        {!connected && (
          <div className="flex items-center gap-2 px-4 py-2 bg-warning/10 text-warning text-xs border-b">
            <span className="h-2 w-2 rounded-full bg-warning animate-pulse shrink-0" />
            <span className="flex-1">{t('chat.reconnecting')}</span>
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
              {t('chat.retry')}
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
        <div className="relative flex-1 min-h-0">
          <ScrollArea
            ref={scrollAreaRef as any}
            className="h-full"
            onScroll={handleScroll}
            role="log"
            aria-label={t('common.chatMessages')}
          >
            <div className="max-w-3xl mx-auto px-4 py-6">
              {messages.length === 0 && <EmptyChatState />}
              <div className="space-y-1">
                {/* Collapsed older messages */}
                {collapseCount > 0 && (
                  <CompressedGroup count={collapseCount}>
                    {messages.slice(0, collapseCount).map((msg, _idx) => {
                      const assistantIndex =
                        msg.role === 'assistant'
                          ? messages.slice(0, _idx).filter((m) => m.role === 'assistant').length
                          : undefined
                      return (
                        <div key={msg.id} data-msg-index={_idx}>
                          <MessageBubble
                            message={msg}
                            sessionId={activeSessionId ?? undefined}
                            assistantIndex={assistantIndex}
                            onRetry={msg.metadata?.isError ? () => handleRetry(msg.id) : undefined}
                          />
                        </div>
                      )
                    })}
                  </CompressedGroup>
                )}
                {/* Visible recent messages */}
                {messages.slice(collapseCount).map((msg, i) => {
                  const _idx = collapseCount + i
                  const assistantIndex =
                    msg.role === 'assistant'
                      ? messages.slice(0, _idx).filter((m) => m.role === 'assistant').length
                      : undefined
                  return (
                    <div key={msg.id} data-msg-index={_idx}>
                      <MessageBubble
                        message={msg}
                        sessionId={activeSessionId ?? undefined}
                        assistantIndex={assistantIndex}
                        onRetry={msg.metadata?.isError ? () => handleRetry(msg.id) : undefined}
                      />
                    </div>
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
                    onApprove={(remember) =>
                      resolveToolApproval(activeToolApproval.id, true, remember)
                    }
                    onDeny={() => resolveToolApproval(activeToolApproval.id, false)}
                    disabled={isStreaming}
                  />
                )}

                <div ref={bottomRef} />
              </div>
            </div>
          </ScrollArea>
          {userScrolledUp && (
            <button
              type="button"
              onClick={() => {
                bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
                setUserScrolledUp(false)
              }}
              className="absolute bottom-4 left-1/2 z-10 flex h-9 w-9 -translate-x-1/2 items-center justify-center rounded-full border bg-background shadow-lg transition-all hover:bg-accent"
              aria-label={t('chat.scrollToBottom')}
            >
              <ArrowDown className="h-4 w-4" />
            </button>
          )}
          <ChatMiniMap messages={messages} onJump={handleMiniMapJump} />
          <TextSelectionBar containerRef={scrollAreaRef} />
        </div>
        {!activeInterview && (
          <div className="bg-background/95 backdrop-blur-sm shrink-0">
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
              isStreaming={isStreaming}
              connected={connected}
              activeMounts={activeMounts}
              queuedCount={queuedCount}
              onAttachMount={handleAttachMount}
              onRemoveMount={handleRemoveMount}
            />
          </div>
        )}
      </div>
      {stackOpen && <PortalPanel className="shrink-0" />}
    </div>
  )
}
