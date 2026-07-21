// messages/AssistantMessage — pipeline renderer for assistant role.
//
// LobeHub analogue: src/features/Conversation/Messages/Assistant/ +
//   Messages/components/MessageContent.tsx (the 6-stage pipeline).
//
// Pipeline order (Phase 2, 2026-07-21):
//   Reasoning → SearchGrounding → FileChunks → DisplayContent → ToolCalls → Images
//
// See docs/designs/2026-07-21-lobehub-chat-port-design.md §7 Phase 2.

import { memo } from 'react'
import type { ChatMessage } from '@/types'
import { ChatItem } from '@/components/chat/chat-item'
import type { ChatItemAvatar } from '@/components/chat/chat-item'
import { ChatMetadata } from '@/components/chat/chat-metadata'
import { ContentLoading } from '@/components/chat/content-loading'
import { FollowUpChips } from '@/components/chat/follow-up-chips'
import { KnowledgeSaveIndicator } from '@/components/chat/knowledge-save-indicator'
import { MarkdownMessage } from '@/components/chat/markdown-message'
import { SearchGrounding } from '@/components/chat/search-grounding'
import { Thinking } from '@/components/chat/thinking'
import { ToolCallList } from './components/ToolCallList'
import { useAssistantActions } from './useAssistantActions'

interface AssistantMessageProps {
  message: ChatMessage
  sessionId?: string
  assistantIndex?: number
  onRetry?: () => void
}

function modelDisplayName(model?: string): string | null {
  if (!model) return null
  return model.includes('/') ? model.split('/').slice(1).join('/') : model
}

function AssistantMessageImpl({
  message,
  sessionId,
  assistantIndex,
  onRetry,
}: AssistantMessageProps) {
  const { actions } = useAssistantActions({ message, onRetry })
  const avatar: ChatItemAvatar = { name: modelDisplayName(message.model) ?? 'Oxios' }

  const hasReasoning = !!(message.reasoning?.content || message.isReasoning)
  const hasSearch = !!(message.search?.citations?.length || message.search?.imageResults?.length)
  const hasContent = !!message.content
  const hasToolCalls = !!(message.toolCalls && message.toolCalls.length > 0)
  const hasChunks = !!(message.chunksList && message.chunksList.length > 0)
  const isError = !!message.metadata?.isError
  const msgError = isError
    ? { type: message.metadata?.errorKind ?? 'unknown', message: message.content }
    : null

  // Stream just started — no content yet, still generating. Show ContentLoading.
  const showLoading =
    !!message.generating && !hasContent && !hasReasoning && !hasToolCalls

  return (
    <ChatItem
      avatar={avatar}
      error={msgError}
      time={message.timestamp ? new Date(message.timestamp).getTime() : undefined}
      actions={actions}
      messageExtra={
        <>
          {message.metadata && !isError && <ChatMetadata message={message} />}
          {sessionId != null && assistantIndex != null && (
            <KnowledgeSaveIndicator sessionId={sessionId} messageIndex={assistantIndex} />
          )}
        </>
      }
    >
      <div className="flex flex-col gap-2">
        {hasReasoning && (
          <Thinking
            content={message.reasoning?.content ?? ''}
            thinking={message.isReasoning ?? false}
            duration={message.reasoning?.duration}
          />
        )}
        {hasSearch && message.search && <SearchGrounding search={message.search} />}
        {hasChunks && <FileChunksPlaceholder chunks={message.chunksList!} />}
        {showLoading && <ContentLoading id={`loading-${message.id}`} />}
        {hasContent && <MarkdownMessage>{message.content}</MarkdownMessage>}
        {hasToolCalls && <ToolCallList calls={message.toolCalls!} />}
        {hasContent && !isError && (
          <FollowUpChips
            sessionId={sessionId}
            messageId={message.id}
            content={message.content}
            onSelect={() => {}}
          />
        )}
      </div>
    </ChatItem>
  )
}

/** Phase 2 placeholder for RAG reference chunks. Phase 3 will port
 *  LobeHub FileChunks accordion with similarity scores + file icons. */
function FileChunksPlaceholder({
  chunks,
}: {
  chunks: NonNullable<ChatMessage['chunksList']>
}) {
  return (
    <details className="text-xs text-muted-foreground">
      <summary className="cursor-pointer hover:text-foreground transition-colors">
        {chunks.length} reference chunk{chunks.length === 1 ? '' : 's'}
      </summary>
      <ul className="mt-1 space-y-1 pl-3">
        {chunks.map((c) => (
          <li key={c.id} className="truncate">
            {c.filename ? `${c.filename}: ` : ''}
            {c.content.slice(0, 120)}
            {c.content.length > 120 ? '…' : ''}
          </li>
        ))}
      </ul>
    </details>
  )
}

export const AssistantMessage = memo(AssistantMessageImpl)
