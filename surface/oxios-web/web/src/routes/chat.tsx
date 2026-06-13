import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ChatInput, type ContextAttachment } from '@/components/chat/chat-input'
import { EmptyChatState } from '@/components/chat/empty-chat-state'
import { InterviewWizard } from '@/components/chat/interview-wizard'
import { MessageBubble } from '@/components/chat/message-bubble'
import { ToolApprovalCard } from '@/components/chat/tool-approval-card'
import { TypingIndicator } from '@/components/chat/typing-indicator'
import { AiDetectionBadge } from '@/components/project/ai-detection-badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useChatStore } from '@/stores/chat'

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
    specMode,
    toggleSpecMode,
    sendMessage,
    setActiveProject,
    dismissDetection,
    submitInterviewResponse,
    activeToolApproval,
    resolveToolApproval,
    disconnect,
    connect,
    newSession,
  } = useChatStore()

  const [input, setInput] = useState('')
  const [userScrolledUp, setUserScrolledUp] = useState(false)
  const bottomRef = useRef<HTMLDivElement>(null)
  const scrollAreaRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to bottom on new messages, but only if user hasn't scrolled up
  useEffect(() => {
    if (userScrolledUp) return
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
      if (mod && e.shiftKey && e.key.toLowerCase() === 'm') {
        e.preventDefault()
        toggleSpecMode()
      }
      if (mod && e.shiftKey && e.key.toLowerCase() === 'n') {
        e.preventDefault()
        newSession()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [toggleSpecMode, newSession])

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

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col min-w-0">
        {/* Reconnect warning banner */}
        {!connected && (
          <div className="flex items-center gap-2 px-4 py-2 bg-warning/10 text-warning text-xs border-b">
            <span className="h-2 w-2 rounded-full bg-warning animate-pulse shrink-0" />
            <span>{t('chat.reconnecting', 'Reconnecting...')}</span>
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
                const assistantIndex = msg.role === 'assistant'
                  ? messages.slice(0, _idx).filter((m) => m.role === 'assistant').length
                  : undefined
                return (
                  <MessageBubble
                    key={msg.id}
                    message={msg}
                    sessionId={activeSessionId ?? undefined}
                    assistantIndex={assistantIndex}
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

              {/* Typing indicator */}
              {isStreaming && !activeInterview && !activeToolApproval && <TypingIndicator />}

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
              onCancel={handleCancel}
              disabled={isStreaming}
              isStreaming={isStreaming}
              connected={connected}
              specMode={specMode}
            />
          </div>
        )}
      </div>
    </div>
  )
}
