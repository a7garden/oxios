// messages/MessageView — top-level dispatcher by message role.
//
// Replaces the monolithic message-bubble.tsx. Each role routes to a dedicated
// component that owns its own pipeline + actions.
//
// LobeHub analogue: Messages/index.tsx (role switch).

import { memo } from 'react'
import type { ChatMessage } from '@/types'
import { AssistantMessage } from './AssistantMessage'
import { ToolMessage } from './ToolMessage'
import { UserMessage } from './UserMessage'

export interface MessageViewProps {
  message: ChatMessage
  sessionId?: string
  assistantIndex?: number
  onRetry?: () => void
}

function MessageViewImpl({ message, sessionId, assistantIndex, onRetry }: MessageViewProps) {
  switch (message.role) {
    case 'user':
      return <UserMessage message={message} />
    case 'tool':
      return <ToolMessage message={message} />
    case 'assistant':
    case 'system': // system messages render like assistant prose
      return (
        <AssistantMessage
          message={message}
          sessionId={sessionId}
          assistantIndex={assistantIndex}
          onRetry={onRetry}
        />
      )
    default:
      return null
  }
}

export const MessageView = memo(MessageViewImpl)
