import { useRouter } from '@tanstack/react-router'
import { Check, Copy, MessageSquarePlus, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { MessageBubble } from '@/components/chat/message-bubble'
import { ToolApprovalCard } from '@/components/chat/tool-approval-card'
import { ChatInput } from '@/components/chat/chat-input'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useEngineConfig } from '@/hooks/use-engine'
import { api } from '@/lib/api-client'
import { useChatStore } from '@/stores/chat'
import { useQuickAskStore } from '@/stores/quick-ask'

/**
 * QuickAskDialog — global one-shot question overlay.
 *
 * Renders in AppLayout so it overlays every route. Sends `ephemeral: true`
 * over its own short-lived WS; nothing is persisted. Uses the same ChatInput
 * as the regular chat page so the UX (model picker, queue, stop, streaming)
 * is identical — the only difference is that no session is saved.
 */
export function QuickAskDialog() {
  const { t } = useTranslation()
  const router = useRouter()
  const open = useQuickAskStore((s) => s.open)
  const closeQuickAsk = useQuickAskStore((s) => s.closeQuickAsk)
  const messages = useQuickAskStore((s) => s.messages)
  const isStreaming = useQuickAskStore((s) => s.isStreaming)
  const send = useQuickAskStore((s) => s.send)
  const cancel = useQuickAskStore((s) => s.cancel)
  const quickAskModel = useQuickAskStore((s) => s.quickAskModel)
  const setQuickAskModel = useQuickAskStore((s) => s.setQuickAskModel)
  const queuedCount = useQuickAskStore((s) => s._pendingQueue.length)
  const lastExchange = useQuickAskStore((s) => s.lastExchange)
  const activeToolApproval = useQuickAskStore((s) => s.activeToolApproval)
  const resolveToolApproval = useQuickAskStore((s) => s.resolveToolApproval)
  const reset = useQuickAskStore((s) => s.reset)

  // Sync the engine-config one-shot model into the store (single source: Settings).
  const { data: engineConfig } = useEngineConfig()
  useEffect(() => {
    const configured = engineConfig?.quick_ask_model
    const fallback = engineConfig?.default_model || null
    setQuickAskModel(configured || fallback)
  }, [engineConfig?.quick_ask_model, engineConfig?.default_model, setQuickAskModel])

  const [input, setInput] = useState('')
  const [copied, setCopied] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)

  // Auto-scroll on new content.
  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' })
  }, [messages, isStreaming])

  // Reset input when the dialog closes.
  useEffect(() => {
    if (!open) {
      setInput('')
      setCopied(false)
    }
  }, [open])

  const handleCopy = async () => {
    const reply = lastExchange?.reply
    if (!reply) return
    try {
      await navigator.clipboard.writeText(reply)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      toast.error(t('quickAsk.copyFailed'))
    }
  }

  const handlePromote = async () => {
    if (!lastExchange) return
    try {
      const res = await api.post<{ session_id: string }>('/api/chat/seed', {
        user_message: lastExchange.prompt,
        agent_response: lastExchange.reply,
        trajectory_steps: lastExchange.activities
          .filter((a) => a.type === 'tool_call')
          .map((a) => ({
            tool: a.toolName,
            input: a.toolArgs,
            output: a.outputSummary,
            duration_ms: a.durationMs,
          })),
        reasoning_text: lastExchange.activities
          .filter((a) => a.type === 'reasoning')
          .map((a) => a.content)
          .join('\n'),
        project_id: undefined,
      })
      closeQuickAsk()
      reset()
      router.history.push('/chat')
      // Seed the chat store so /chat shows the promoted exchange immediately.
      await useChatStore.getState().loadSession(res.session_id)
      toast.success(t('quickAsk.promoted'))
    } catch {
      toast.error(t('quickAsk.promoteFailed'))
    }
  }

  const hasResult = !isStreaming && lastExchange !== null
  const empty = messages.length === 0

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) closeQuickAsk()
      }}
    >
      <DialogContent
        showCloseButton={false}
        onOpenAutoFocus={(e) => e.preventDefault()}
        className="flex h-[80vh] max-w-2xl flex-col gap-0 p-0 sm:rounded-xl"
      >
        <DialogHeader className="flex-row items-center justify-between border-b px-5 py-3">
          <div className="flex items-center gap-2">
            <DialogTitle className="text-sm font-medium">{t('quickAsk.title')}</DialogTitle>
          </div>
          <DialogDescription className="sr-only">{t('quickAsk.placeholder')}</DialogDescription>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={closeQuickAsk}
            aria-label={t('common.close')}
          >
            <X className="h-4 w-4" />
          </Button>
        </DialogHeader>

        <ScrollArea className="flex-1" ref={scrollRef}>
          <div className="space-y-4 px-5 py-4">
            {empty && !isStreaming && (
              <p className="py-12 text-center text-sm text-muted-foreground">
                {t('quickAsk.placeholder')}
              </p>
            )}
            {messages.map((m) => (
              <MessageBubble key={m.id} message={m} />
            ))}
            {activeToolApproval && (
              <ToolApprovalCard
                toolName={activeToolApproval.toolName}
                reason={activeToolApproval.reason}
                onApprove={() => resolveToolApproval(activeToolApproval.id, true)}
                onDeny={() => resolveToolApproval(activeToolApproval.id, false)}
              />
            )}
          </div>
        </ScrollArea>

        {/* Result actions (copy / promote) — shown only when a turn completed. */}
        {hasResult && (
          <div className="flex items-center justify-between border-t px-5 py-1.5">
            <div className="flex items-center gap-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={handleCopy}
                className="h-7 gap-1.5 px-2 text-xs"
              >
                {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                {t('quickAsk.copy')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={handlePromote}
                className="h-7 gap-1.5 px-2 text-xs"
              >
                <MessageSquarePlus className="h-3 w-3" />
                {t('quickAsk.promote')}
              </Button>
            </div>
            <span className="text-[10px] text-muted-foreground">{t('quickAsk.notSaved')}</span>
          </div>
        )}

        {/* Input — same ChatInput as the chat page for UX parity. */}
        <div className="border-t bg-background/95 backdrop-blur-sm shrink-0">
          <ChatInput
            value={input}
            onChange={setInput}
            onSend={(content) => send(content)}
            onCancel={cancel}
            isStreaming={isStreaming}
            connected={true}
            queuedCount={queuedCount}
            activeModelId={quickAskModel}
            setActiveModelId={setQuickAskModel}
            placeholder={t('quickAsk.placeholder')}
            showNewChatHint={false}
          />
        </div>
      </DialogContent>
    </Dialog>
  )
}
