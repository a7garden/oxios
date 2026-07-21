// messages/ToolMessage — standalone tool result message (role === 'tool').
//
// Renders using the legacy single-tool fields (toolName/toolArgs/toolResult)
// OR the new structured toolCalls[] field if present.

import { memo } from 'react'
import type { ChatMessage } from '@/types'
import type { ChatToolPayload } from '@/types/chat'
import { ToolCallList } from './components/ToolCallList'

interface ToolMessageProps {
  message: ChatMessage
}

function ToolMessageImpl({ message }: ToolMessageProps) {
  // Prefer structured toolCalls; fall back to legacy single-tool fields.
  const calls: ChatToolPayload[] =
    message.toolCalls && message.toolCalls.length > 0
      ? message.toolCalls
      : message.toolName
        ? [
            {
              id: message.id,
              identifier: 'kernel',
              apiName: message.toolName,
              arguments: message.toolArgs,
              result: message.toolResult,
              status: 'success',
              durationMs: message.toolDurationMs,
            },
          ]
        : []

  if (calls.length === 0) return null
  return (
    <div className="my-1">
      <ToolCallList calls={calls} defaultExpanded />
    </div>
  )
}

export const ToolMessage = memo(ToolMessageImpl)
