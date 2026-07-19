import { Copy, Pencil, RefreshCw, Trash2 } from 'lucide-react'
import { useCallback, useState } from 'react'
import { toast } from 'sonner'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage } from '@/types'
import type { ChatItemAvatar } from './chat-item'
import { ChatItem } from './chat-item'
import { ChatMetadata } from './chat-metadata'
import { FollowUpChips } from './follow-up-chips'
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
  const { removeMessage, sendMessage, messages } = useChatStore()

  // ── All hooks must be called before any conditional return ──
  const [editing, setEditing] = useState(false)
  const [editValue, setEditValue] = useState('')
  const [copied, setCopied] = useState(false)

  const startEdit = useCallback(() => {
    setEditValue(message.content)
    setEditing(true)
  }, [message.content])

  const saveEdit = useCallback(() => {
    if (!editValue.trim()) return
    removeMessage(message.id)
    sendMessage(editValue.trim())
    setEditing(false)
  }, [editValue, message.id, removeMessage, sendMessage])

  const handleDelete = useCallback(() => {
    removeMessage(message.id)
    toast('Message deleted', { description: 'This action cannot be undone.' })
  }, [message.id, removeMessage])

  const handleRegenerate = useCallback(() => {
    const idx = messages.findIndex((m) => m.id === message.id)
    if (idx < 0) return
    const precedingUser = [...messages.slice(0, idx)].reverse().find((m) => m.role === 'user')
    if (!precedingUser) return
    removeMessage(message.id)
    sendMessage(precedingUser.content)
  }, [message.id, messages, removeMessage, sendMessage])

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(message.content).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [message.content])

  const modelMark = message.model
    ? message.model.includes('/')
      ? message.model.split('/').slice(1).join('/')
      : message.model
    : null

  const avatar: ChatItemAvatar = isUser ? { name: 'You' } : { name: modelMark ?? 'Oxios' }

  // tool messages
  if (isTool) {
    return (
      <ToolCallCard
        call={{
          tool_name: message.toolName,
          input:
            typeof message.toolArgs === 'string'
              ? message.toolArgs
              : JSON.stringify(message.toolArgs ?? {}),
          output:
            typeof message.toolResult === 'string'
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
        actions={
          <div className="flex items-center gap-0.5">
            <button
              type="button"
              onClick={startEdit}
              title="Edit"
              className="inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
            >
              <Pencil className="w-3 h-3" />
            </button>
            <button
              type="button"
              onClick={handleDelete}
              title="Delete"
              className="inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:text-destructive hover:bg-muted transition-colors"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
        }
      >
        {editing ? (
          <div className="flex flex-col gap-2">
            <textarea
              value={editValue}
              onChange={(e) => setEditValue(e.target.value)}
              className="w-full min-w-[300px] rounded-lg border bg-background px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary/30 resize-none"
              rows={Math.min(editValue.split('\n').length, 10)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault()
                  saveEdit()
                }
                if (e.key === 'Escape') {
                  setEditing(false)
                }
              }}
            />
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={saveEdit}
                className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md bg-primary text-primary-foreground text-xs"
              >
                Save &amp; Resend
              </button>
              <button
                type="button"
                onClick={() => setEditing(false)}
                className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md border text-xs"
              >
                Cancel
              </button>
            </div>
          </div>
        ) : (
          <div className="inline-block max-w-[85%] rounded-lg bg-muted/50 px-3 py-2 text-sm">
            {message.content}
          </div>
        )}
      </ChatItem>
    )
  }

  // assistant messages
  const hasReasoning = !!(message.reasoning?.content || message.isReasoning)
  const hasSearch = !!(message.search?.citations?.length || message.search?.imageResults?.length)
  const hasContent = !!message.content

  const isError = message.metadata?.isError
  const msgError = isError
    ? { type: message.metadata?.errorKind ?? 'unknown', message: message.content }
    : null

  return (
    <ChatItem
      avatar={avatar}
      error={msgError}
      time={message.timestamp ? new Date(message.timestamp).getTime() : undefined}
      actions={
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={handleCopy}
            title={copied ? 'Copied!' : 'Copy'}
            className="inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          >
            {copied ? <span className="text-2xs">Copied</span> : <Copy className="w-3 h-3" />}
          </button>
          {!isError && (
            <button
              type="button"
              onClick={handleRegenerate}
              title="Regenerate"
              className="inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
            >
              <RefreshCw className="w-3 h-3" />
            </button>
          )}
          {isError && onRetry && (
            <button
              type="button"
              onClick={onRetry}
              title="Retry"
              className="inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:text-destructive hover:bg-muted transition-colors"
            >
              <RefreshCw className="w-3 h-3" />
            </button>
          )}
          <button
            type="button"
            onClick={handleDelete}
            title="Delete"
            className="inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:text-destructive hover:bg-muted transition-colors"
          >
            <Trash2 className="w-3 h-3" />
          </button>
        </div>
      }
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
        {hasContent && <MarkdownMessage>{message.content}</MarkdownMessage>}

        {/* Follow-up suggestion chips */}
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
