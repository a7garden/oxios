// useAssistantActions — action-bar handlers for assistant messages.
//
// Extracted from the legacy message-bubble.tsx so both AssistantMessage and
// future variants (supervisor, agent-council) can share the same action set.

import { useCallback, useState } from 'react'
import { toast } from 'sonner'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage } from '@/types'
import type { ReactNode } from 'react'
import { Copy, RefreshCw, Trash2 } from 'lucide-react'

interface UseAssistantActionsArgs {
  message: ChatMessage
  onRetry?: () => void
}

export interface AssistantActionsResult {
  actions: ReactNode
  copied: boolean
}

export function useAssistantActions({
  message,
  onRetry,
}: UseAssistantActionsArgs): AssistantActionsResult {
  const { removeMessage, sendMessage, messages } = useChatStore()
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(message.content)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }, [message.content])

  const handleDelete = useCallback(() => {
    removeMessage?.(message.id)
  }, [message.id, removeMessage])

  const handleRegenerate = useCallback(() => {
    // Find preceding user message and re-send it
    const idx = messages.findIndex((m) => m.id === message.id)
    if (idx <= 0) return
    const precedingUser = messages[idx - 1]
    if (!precedingUser || precedingUser.role !== 'user') return
    removeMessage?.(message.id)
    removeMessage?.(precedingUser.id)
    sendMessage(precedingUser.content)
  }, [message.id, messages, removeMessage, sendMessage])

  const isError = !!message.metadata?.isError

  return {
    copied,
    actions: (
      <div className="flex items-center gap-0.5">
        <ActionButton onClick={handleCopy} title={copied ? 'Copied!' : 'Copy'}>
          {copied ? <span className="text-2xs">Copied</span> : <Copy className="w-3 h-3" />}
        </ActionButton>
        {!isError && (
          <ActionButton onClick={handleRegenerate} title="Regenerate">
            <RefreshCw className="w-3 h-3" />
          </ActionButton>
        )}
        {isError && onRetry && (
          <ActionButton onClick={onRetry} title="Retry" hoverDanger>
            <RefreshCw className="w-3 h-3" />
          </ActionButton>
        )}
        <ActionButton onClick={handleDelete} title="Delete" hoverDanger>
          <Trash2 className="w-3 h-3" />
        </ActionButton>
      </div>
    ),
  }
}

function ActionButton({
  onClick,
  title,
  hoverDanger,
  children,
}: {
  onClick: () => void
  title: string
  hoverDanger?: boolean
  children: ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      className={`inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:bg-muted transition-colors ${
        hoverDanger ? 'hover:text-destructive' : 'hover:text-foreground'
      }`}
    >
      {children}
    </button>
  )
}

/** Silence unused-import warnings when toast ends up unused after refactors. */
void toast
