import { useEffect, useState } from 'react'
import { ReactionsBar } from './components/reactions-bar'

function MessageReactionsRow({ messageId }: { messageId: string }) {
  const [version, setVersion] = useState(0)
  useEffect(() => {
    const onChange = () => setVersion((v) => v + 1)
    window.addEventListener('reactions-changed', onChange)
    return () => window.removeEventListener('reactions-changed', onChange)
  }, [])
  return <ReactionsBar messageId={messageId} version={version} />
}

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
import type { ChatItemAvatar } from '@/components/chat/chat-item'
import { ChatItem } from '@/components/chat/chat-item'
import { ChatMetadata } from '@/components/chat/chat-metadata'
import { FollowUpChips } from '@/components/chat/follow-up-chips'
import { KnowledgeSaveIndicator } from '@/components/chat/knowledge-save-indicator'
import { MarkdownMessage } from '@/components/chat/markdown-message'
import { SearchGrounding } from '@/components/chat/search-grounding'
import { Thinking } from '@/components/chat/thinking'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage } from '@/types'
import { ErrorCard } from './components/ErrorCard'
import { MessageActionBar } from './components/MessageActionBar'
import { BlockStream } from './components/BlockStream'
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
  const { sendMessage } = useChatStore()
  const avatar: ChatItemAvatar = { name: modelDisplayName(message.model) ?? 'Oxios' }

  const hasReasoning = !!(message.reasoning?.content || message.isReasoning)
  const hasSearch = !!(message.search?.citations?.length || message.search?.imageResults?.length)
  const hasContent = !!message.content
  const hasToolCalls = !!(message.toolCalls && message.toolCalls.length > 0)
  const hasChunks = !!(message.chunksList && message.chunksList.length > 0)
  const hasBlocks = !!(message.blocks && message.blocks.length > 0)
  const isError = !!message.metadata?.isError
  const chatError = isError
    ? {
        type: message.metadata?.errorKind ?? 'unknown',
        message: message.content,
        severity: 'error' as const,
      }
    : null

  return (
    <ChatItem
      avatar={avatar}
      error={chatError}
      time={message.timestamp ? new Date(message.timestamp).getTime() : undefined}
      durationMs={message.metadata?.duration_ms}
      actions={<MessageActionBar actions={actions} />}
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
        {hasBlocks ? (
          <BlockStream blocks={message.blocks!} messageId={message.id} />
        ) : (
          <>
            {hasReasoning && (
              <Thinking
                content={message.reasoning?.content ?? ''}
                thinking={message.isReasoning ?? false}
                duration={message.reasoning?.duration}
              />
            )}
            {hasContent && !isError && (
              <MarkdownMessage messageId={message.id} isStreaming={!!message.generating}>
                {message.content}
              </MarkdownMessage>
            )}
            {hasToolCalls && <ToolCallList calls={message.toolCalls!} />}
          </>
        )}
        {hasSearch && message.search && <SearchGrounding search={message.search} />}
        {hasChunks && <FileChunksPlaceholder chunks={message.chunksList!} />}
        {isError && chatError && <ErrorCard error={chatError} onRetry={onRetry} />}
        {hasContent && !isError && (
          <FollowUpChips
            content={message.content}
            generating={message.generating}
            onSelect={(s) => sendMessage(s)}
          />
        )}
        <MessageReactionsRow messageId={message.id} />
      </div>
    </ChatItem>
  )
}

/** Phase 2 placeholder for RAG reference chunks. Phase 3 will port
 *  LobeHub FileChunks accordion with similarity scores + file icons. */
function FileChunksPlaceholder({ chunks }: { chunks: NonNullable<ChatMessage['chunksList']> }) {
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
