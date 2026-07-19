import { Copy, RefreshCw } from 'lucide-react'
import { useCallback, useState } from 'react'
import type { ChatMessage } from '@/types'
import { ChatItem } from './chat-item'
import type { ChatItemAvatar } from './chat-item'
import { ChatMetadata } from './chat-metadata'
import { KnowledgeSaveIndicator } from './knowledge-save-indicator'
import { MarkdownMessage } from './markdown-message'
import { SearchGrounding } from './search-grounding'
import { Thinking } from './thinking'
import { ToolCallCard } from './tool-call-card'

interface MessageBubbleProps {
  message: ChatMessage
  sessionId?: string
  assistantIndex?: number
  onRetry?: () => void
}

export function MessageBubble({ message, sessionId, assistantIndex, onRetry }: MessageBubbleProps) {
  const isUser = message.role === 'user'
  const isTool = message.role === 'tool'

  const modelMark = message.model
    ? message.model.includes('/')
      ? message.model.split('/').slice(1).join('/')
      : message.model
    : null

  const avatar: ChatItemAvatar = isUser
    ? { name: 'You' }
    : { name: modelMark ?? 'Oxios' }

  // tool messages
  if (isTool) {
    return (
      <ToolCallCard
        call={{
          tool_name: message.toolName,
          input: typeof message.toolArgs === 'string'
            ? message.toolArgs
            : JSON.stringify(message.toolArgs ?? {}),
          output: typeof message.toolResult === 'string'
            ? message.toolResult
            : JSON.stringify(message.toolResult ?? ''),
          duration_ms: message.toolDurationMs ?? 0,
        }}
        className="my-1"
      />
    )
  }

  // user messages
  if (isUser) {
    return (
      <ChatItem
        avatar={avatar}
        placement="right"
        time={message.timestamp ? new Date(message.timestamp).getTime() : undefined}
        showTitle={false}
      >
        <div className="inline-block max-w-[85%] rounded-lg bg-muted/50 px-3 py-2 text-sm">
          {message.content}
        </div>
      </ChatItem>
    )
  }

  // assistant messages
  const hasReasoning = !!(message.reasoning?.content || message.isReasoning)
  const hasSearch = !!(message.search?.citations?.length || message.search?.imageResults?.length)
  const hasContent = !!message.content

  const [copied, setCopied] = useState(false)
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(message.content).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [message.content])

  const msgError = message.metadata?.isError
    ? { type: message.metadata.errorKind ?? 'unknown', message: message.content }
    : null

  return (
    <ChatItem
      avatar={avatar}
      error={msgError}
      time={message.timestamp ? new Date(message.timestamp).getTime() : undefined}
      actions={
        <div className="flex items-center gap-1">
          <button type="button" onClick={handleCopy} className="inline-flex items-center gap-1 px-2 py-1 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors">
            {copied ? <span>Copied</span> : <><Copy className="w-3 h-3" />Copy</>}
          </button>
          {onRetry && (
            <button type="button" onClick={onRetry} className="inline-flex items-center gap-1 px-2 py-1 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors">
              <RefreshCw className="w-3 h-3" />Retry
            </button>
          )}
        </div>
      }
      messageExtra={
        <>
          {message.metadata && !message.metadata.isError && <ChatMetadata message={message} />}
          {sessionId != null && assistantIndex != null && (
            <KnowledgeSaveIndicator sessionId={sessionId} messageIndex={assistantIndex} />
          )}
        </>
      }
    >
      <div className="flex flex-col gap-2">
        {hasReasoning && (
          <Thinking content={message.reasoning?.content ?? ''} thinking={message.isReasoning ?? false} duration={message.reasoning?.duration} />
        )}
        {hasSearch && message.search && <SearchGrounding search={message.search} />}
        {hasContent && <MarkdownMessage>{message.content}</MarkdownMessage>}
      </div>
    </ChatItem>
  )
}
