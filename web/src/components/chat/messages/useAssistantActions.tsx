// useAssistantActions — action-bar handlers for assistant messages.
//
// Extracted from the legacy message-bubble.tsx so both AssistantMessage and
// future variants (supervisor, agent-council) can share the same action set.
//
// Phase 4 (2026-07-21): returns a MessageAction[] consumed by MessageActionBar,
// replacing the bespoke inline JSX.

import { useCallback, useState } from 'react'
import { Copy, RefreshCw, Trash2 } from 'lucide-react'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage } from '@/types'
import type { MessageAction } from './components/MessageActionBar'

interface UseAssistantActionsArgs {
  message: ChatMessage
  onRetry?: () => void
}

export interface AssistantActionsResult {
  actions: MessageAction[]
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
    const idx = messages.findIndex((m) => m.id === message.id)
    if (idx <= 0) return
    const precedingUser = messages[idx - 1]
    if (!precedingUser || precedingUser.role !== 'user') return
    removeMessage?.(message.id)
    removeMessage?.(precedingUser.id)
    sendMessage(precedingUser.content)
  }, [message.id, messages, removeMessage, sendMessage])

  const isError = !!message.metadata?.isError

  const actions: MessageAction[] = [
    {
      id: 'copy',
      icon: <Copy className="w-3 h-3" />,
      label: copied ? 'Copied!' : 'Copy',
      onClick: handleCopy,
      children: copied ? <span className="text-2xs">Copied</span> : undefined,
    },
    {
      id: 'regenerate',
      icon: <RefreshCw className="w-3 h-3" />,
      label: 'Regenerate',
      onClick: handleRegenerate,
      hidden: isError,
    },
    {
      id: 'retry',
      icon: <RefreshCw className="w-3 h-3" />,
      label: 'Retry',
      onClick: onRetry ?? (() => {}),
      hidden: !isError || !onRetry,
      danger: true,
    },
    {
      id: 'delete',
      icon: <Trash2 className="w-3 h-3" />,
      label: 'Delete',
      onClick: handleDelete,
      danger: true,
    },
  ]

  return { actions, copied }
}
