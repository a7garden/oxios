import { createFileRoute } from '@tanstack/react-router'
import { ArrowDown, RefreshCw } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { VList, type VListHandle } from 'virtua'
import type { AttachedFile, ContextAttachment } from '@/components/chat/chat-input'
import { ChatInput } from '@/components/chat/chat-input'
import { ChatMiniMap } from '@/components/chat/chat-minimap'
import { CompressedGroup } from '@/components/chat/compressed-group'
import { EmptyChatState } from '@/components/chat/empty-chat-state'
import { InterviewWizard } from '@/components/chat/interview-wizard'
import { MessageBubble } from '@/components/chat/message-bubble'
import { PathAccessCard } from '@/components/chat/path-access-card'
import { TextSelectionBar } from '@/components/chat/text-selection-bar'
import { ToolApprovalCard } from '@/components/chat/tool-approval-card'
import { MountDetectionBadge } from '@/components/mount/mount-detection-badge'
import { PortalPanel } from '@/components/portal/portal-panel'
import { AiDetectionBadge } from '@/components/project/ai-detection-badge'
import { Button } from '@/components/ui/button'
import { useDraftPersistence } from '@/hooks/use-draft-persistence'
import { useRoles } from '@/hooks/use-engine'
import { useMounts } from '@/hooks/use-mounts'
import { buildChatRows } from '@/lib/chat-rows'
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
    activePathAccess,
    resolvePathAccess,
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
  const vListRef = useRef<VListHandle>(null)
  const messagesContainerRef = useRef<HTMLDivElement>(null)
  const atBottomRef = useRef(true)
  const [expanded, setExpanded] = useState(false)

  // Compressed groups: collapse older messages when a conversation is long.
  const COLLAPSE_THRESHOLD = 40
  const VISIBLE_TAIL = 20

  // Virtualized row model (LobeHub borrow): the VList renders this flat array.
  const rows = useMemo(
    () =>
      buildChatRows({
        messages,
        expanded,
        collapseThreshold: COLLAPSE_THRESHOLD,
        visibleTail: VISIBLE_TAIL,
        hasInterview: !!activeInterview && activeInterview.length > 0,
        hasToolApproval: !!activeToolApproval,
        hasPathAccess: !!activePathAccess,
      }),
    [messages, expanded, activeInterview, activeToolApproval, activePathAccess],
  )

  // Auto-scroll to the last row while the user is at (or near) the bottom.
  // Re-anchors as the streaming message grows (dep on trailing content length).
  const lastContentLen = messages.at(-1)?.content?.length ?? 0
  useEffect(() => {
    if (atBottomRef.current) {
      vListRef.current?.scrollToIndex(rows.length - 1, { align: 'end' })
    }
  }, [rows.length, isStreaming, lastContentLen])

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

  const handleVListScroll = (offset: number) => {
    const vl = vListRef.current
    if (!vl) return
    const atBottom = vl.scrollSize - offset - vl.viewportSize < 80
    atBottomRef.current = atBottom
    setUserScrolledUp(!atBottom)
  }

  const handleMiniMapJump = (index: number) => {
    const rowIndex = rows.findIndex((r) => r.kind === 'message' && r.index === index)
    if (rowIndex >= 0) {
      vListRef.current?.scrollToIndex(rowIndex, { align: 'center', smooth: true })
    }
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
        <div ref={messagesContainerRef} className="relative flex-1 min-h-0">
          <VList
            ref={vListRef}
            onScroll={handleVListScroll}
            className="h-full"
            role="log"
            aria-label={t('common.chatMessages')}
          >
            {rows.map((row) => {
              if (row.kind === 'empty') {
                return (
                  <div key="empty" className="mx-auto max-w-3xl px-4 py-6">
                    <EmptyChatState />
                  </div>
                )
              }
              if (row.kind === 'collapse-bar') {
                return (
                  <div key="collapse-bar" className="mx-auto max-w-3xl px-4 pt-6">
                    <CompressedGroup
                      count={row.count}
                      expanded={expanded}
                      onToggle={() => setExpanded((v) => !v)}
                    />
                  </div>
                )
              }
              if (row.kind === 'message') {
                const m = row.message
                const assistantIndex =
                  m.role === 'assistant'
                    ? messages.slice(0, row.index).filter((x) => x.role === 'assistant').length
                    : undefined
                return (
                  <div
                    key={m.id}
                    data-msg-index={row.index}
                    className="mx-auto max-w-3xl px-4 py-0.5"
                  >
                    <MessageBubble
                      message={m}
                      sessionId={activeSessionId ?? undefined}
                      assistantIndex={assistantIndex}
                      onRetry={m.metadata?.isError ? () => handleRetry(m.id) : undefined}
                    />
                  </div>
                )
              }
              if (row.kind === 'interview') {
                return (
                  <div key="interview" className="mx-auto max-w-3xl px-4 py-2">
                    <InterviewWizard
                      questions={activeInterview!}
                      round={interviewRound}
                      ambiguity={interviewAmbiguity}
                      onSubmit={submitInterviewResponse}
                      disabled={isStreaming}
                    />
                  </div>
                )
              }
              if (row.kind === 'tool-approval') {
                return (
                  <div key="tool-approval" className="mx-auto max-w-3xl px-4 py-2">
                    <ToolApprovalCard
                      toolName={activeToolApproval!.toolName}
                      reason={activeToolApproval!.reason}
                      onApprove={(remember) =>
                        resolveToolApproval(activeToolApproval!.id, true, remember)
                      }
                      onDeny={() => resolveToolApproval(activeToolApproval!.id, false)}
                      disabled={isStreaming}
                    />
                  </div>
                )
              }
              // path-access
              return (
                <div key="path-access" className="mx-auto max-w-3xl px-4 py-2">
                  <PathAccessCard
                    path={activePathAccess!.path}
                    mode={activePathAccess!.mode}
                    toolName={activePathAccess!.toolName}
                    reason={activePathAccess!.reason}
                    onMount={() => resolvePathAccess(activePathAccess!.id, 'mount')}
                    onTempAllow={() => resolvePathAccess(activePathAccess!.id, 'temp')}
                    onDeny={() => resolvePathAccess(activePathAccess!.id, 'deny')}
                    disabled={isStreaming}
                  />
                </div>
              )
            })}
          </VList>
          {userScrolledUp && (
            <button
              type="button"
              onClick={() => {
                vListRef.current?.scrollToIndex(rows.length - 1, { align: 'end', smooth: true })
                setUserScrolledUp(false)
              }}
              className="absolute bottom-4 left-1/2 z-10 flex h-9 w-9 -translate-x-1/2 items-center justify-center rounded-full border bg-background shadow-lg transition-all hover:bg-accent"
              aria-label={t('chat.scrollToBottom')}
            >
              <ArrowDown className="h-4 w-4" />
            </button>
          )}
          <ChatMiniMap messages={messages} onJump={handleMiniMapJump} />
          <TextSelectionBar containerRef={messagesContainerRef} />
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
